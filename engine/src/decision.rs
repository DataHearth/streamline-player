use serde::{Deserialize, Serialize};

use crate::analysis::{Analysis, AudioStream, Hdr, VideoStream};
use crate::caps::Capabilities;
use crate::error::ApiError;

const MAX_TRANSCODE_BITRATE: u64 = 40_000_000;
const MIN_BITRATE_AT_1080P: u64 = 4_000_000;
const MIN_H264_BITRATE_AT_1080P: u64 = 8_000_000;
const FMP4_AUDIO: [&str; 6] = ["aac", "mp3", "ac3", "eac3", "flac", "opus"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Direct,
    Remux,
    AudioTranscode,
    Transcode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reason {
    pub code: &'static str,
    pub text: String,
}

impl Reason {
    fn new(code: &'static str, text: impl Into<String>) -> Self {
        Self {
            code,
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct SessionParams {
    pub audio_index: Option<u32>,
    pub burn_in_subtitle: Option<u32>,
    pub max_bitrate: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    Hevc,
    Av1,
}

impl VideoCodec {
    fn name(self) -> &'static str {
        match self {
            Self::H264 => "h264",
            Self::Hevc => "hevc",
            Self::Av1 => "av1",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyTag {
    Hvc1,
    Dvh1,
    Avc1,
    Av01,
    Dav1,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DvHandling {
    Keep,
    Strip,
    P7ToP81,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdrOut {
    Keep(Hdr),
    Tonemap,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VideoAction {
    Copy {
        tag: CopyTag,
        dv: DvHandling,
    },
    Transcode {
        codec: VideoCodec,
        width: u32,
        height: u32,
        bitrate: u64,
        hdr_out: HdrOut,
        deinterlace: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioAction {
    Copy,
    Transcode {
        codec: &'static str,
        channels: u32,
        bitrate: u64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub mode: Mode,
    pub video_index: u32,
    pub video: VideoAction,
    pub audio_index: Option<u32>,
    pub audio: AudioAction,
    pub video_reasons: Vec<Reason>,
    pub audio_reasons: Vec<Reason>,
    pub burn_in_subtitle: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EncoderSet {
    pub h264: bool,
    pub hevc: bool,
    pub av1: bool,
    /// Rank HEVC above AV1 when the client decodes both.
    pub hevc_first: bool,
    /// dovi_tool ran at boot; without it profile 7 cannot become 8.1.
    pub dovi_tool: bool,
}

impl EncoderSet {
    pub const SOFTWARE: Self = Self {
        h264: true,
        hevc: true,
        av1: true,
        hevc_first: false,
        dovi_tool: false,
    };

    fn has(&self, codec: VideoCodec) -> bool {
        match codec {
            VideoCodec::H264 => self.h264,
            VideoCodec::Hevc => self.hevc,
            VideoCodec::Av1 => self.av1,
        }
    }

    /// Only `codec`, when this set has it; otherwise the set as it is.
    pub fn prefer_only(self, codec: VideoCodec) -> Self {
        if !self.has(codec) {
            return self;
        }
        Self {
            h264: codec == VideoCodec::H264,
            hevc: codec == VideoCodec::Hevc,
            av1: codec == VideoCodec::Av1,
            ..self
        }
    }
}

pub fn ladder_height(max_bitrate: u64, codec: VideoCodec) -> u32 {
    let h264 = codec == VideoCodec::H264;
    match max_bitrate {
        20_000_000.. => {
            if h264 {
                1080
            } else {
                2160
            }
        }
        8_000_000.. => 1080,
        4_000_000.. => {
            if h264 {
                720
            } else {
                1080
            }
        }
        2_000_000.. => {
            if h264 {
                480
            } else {
                720
            }
        }
        _ => {
            if h264 {
                360
            } else {
                480
            }
        }
    }
}

fn is_420(pix_fmt: &str) -> bool {
    pix_fmt.contains("420") || pix_fmt.starts_with("nv12") || pix_fmt.starts_with("p010")
}

fn hdr_listed(caps: &Capabilities, name: &str) -> bool {
    caps.hdr.formats.iter().any(|f| f == name)
}

fn base_hdr_fits(caps: &Capabilities, hdr: Hdr) -> bool {
    match hdr {
        Hdr::Sdr => true,
        Hdr::Hdr10 => hdr_listed(caps, "hdr10"),
        Hdr::Hlg => hdr_listed(caps, "hlg"),
    }
}

fn plain_tag(codec: &str) -> CopyTag {
    match codec {
        "hevc" => CopyTag::Hvc1,
        "h264" => CopyTag::Avc1,
        "av1" => CopyTag::Av01,
        _ => CopyTag::Other,
    }
}

enum DvOutcome {
    Fits(CopyTag, DvHandling),
    Unsupported,
}

/// The conversion pipes the video through dovi_tool, which cannot write to stdout on Windows, and the muxer gets frame timing from a constant frame rate.
fn p7_conversion_possible(v: &VideoStream, enc: &EncoderSet) -> bool {
    cfg!(unix) && enc.dovi_tool && !v.vfr && v.fps > 0.0
}

fn dv_rule(caps: &Capabilities, v: &VideoStream, profile: u8, enc: &EncoderSet) -> DvOutcome {
    let av1 = v.codec == "av1";
    let dv_profiles = &caps.hdr.dv_profiles;
    if dv_profiles.contains(&profile) {
        let tag = if av1 { CopyTag::Dav1 } else { CopyTag::Dvh1 };
        return DvOutcome::Fits(tag, DvHandling::Keep);
    }
    if profile == 7 && dv_profiles.contains(&8) && p7_conversion_possible(v, enc) {
        return DvOutcome::Fits(CopyTag::Dvh1, DvHandling::P7ToP81);
    }
    let compat = v.dv.as_ref().map_or(0, |d| d.bl_compat_id);
    let base = match compat {
        1 | 6 => Some(Hdr::Hdr10),
        4 => Some(Hdr::Hlg),
        2 => Some(Hdr::Sdr),
        _ => None,
    };
    match base {
        Some(h) if base_hdr_fits(caps, h) => {
            DvOutcome::Fits(plain_tag(&v.codec), DvHandling::Strip)
        }
        _ => DvOutcome::Unsupported,
    }
}

fn is_dv_stream(v: &VideoStream) -> Option<u8> {
    let d = v.dv.as_ref()?;
    let known = match v.codec.as_str() {
        "hevc" => matches!(d.profile, 5 | 7 | 8),
        "av1" => d.profile == 10,
        _ => false,
    };
    known.then_some(d.profile)
}

fn profile_key(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, ' ' | '_'))
        .flat_map(char::to_lowercase)
        .collect()
}

fn has_profile(profiles: &[String], wanted: &str) -> bool {
    let wanted = profile_key(wanted);
    profiles.iter().any(|p| profile_key(p) == wanted)
}

fn video_fit(
    caps: &Capabilities,
    a: &Analysis,
    v: &VideoStream,
    p: &SessionParams,
    enc: &EncoderSet,
) -> (Vec<Reason>, Option<(CopyTag, DvHandling)>) {
    let mut reasons = Vec::new();
    let mut tag = (plain_tag(&v.codec), DvHandling::Keep);

    match caps.video.iter().find(|c| c.codec == v.codec) {
        None => reasons.push(Reason::new(
            "codec_unsupported",
            format!("client does not decode {}", v.codec),
        )),
        Some(cap) => {
            if !cap.profiles.is_empty() && !has_profile(&cap.profiles, &v.profile) {
                reasons.push(Reason::new(
                    "profile_unsupported",
                    format!("profile {} is not supported", v.profile),
                ));
            }
            if let (Some(max), Some(level)) = (cap.max_level, v.level)
                && level > max
            {
                reasons.push(Reason::new(
                    "level_unsupported",
                    format!("level {level} is over {max}"),
                ));
            }
            if v.width > cap.max_width || v.height > cap.max_height {
                reasons.push(Reason::new(
                    "resolution_over",
                    format!(
                        "{}x{} is over {}x{}",
                        v.width, v.height, cap.max_width, cap.max_height
                    ),
                ));
            }
            if v.fps > cap.max_fps {
                reasons.push(Reason::new(
                    "fps_over",
                    format!("{} fps is over {}", v.fps, cap.max_fps),
                ));
            }
        }
    }

    if !is_420(&v.pix_fmt) {
        reasons.push(Reason::new(
            "chroma_unsupported",
            format!("{} is not 4:2:0", v.pix_fmt),
        ));
    }

    match is_dv_stream(v) {
        Some(profile) => match dv_rule(caps, v, profile, enc) {
            DvOutcome::Fits(t, dv) => tag = (t, dv),
            DvOutcome::Unsupported => reasons.push(Reason::new(
                "dv_profile_unsupported",
                format!("dolby vision profile {profile} cannot be played or reduced"),
            )),
        },
        None => {
            if !base_hdr_fits(caps, v.hdr) {
                reasons.push(Reason::new(
                    "hdr_unsupported",
                    format!("display cannot render {:?}", v.hdr),
                ));
            }
        }
    }

    if let Some(max) = p.max_bitrate
        && a.bit_rate > max
    {
        reasons.push(Reason::new(
            "bitrate_cap",
            format!("capped at {} Mbit/s", max as f64 / 1_000_000.0),
        ));
    }
    if p.burn_in_subtitle.is_some() {
        reasons.push(Reason::new("burn_in", "subtitle is burned into the video"));
    }

    let fit = reasons.is_empty().then_some(tag);
    (reasons, fit)
}

fn audio_direct_fits(caps: &Capabilities, s: &AudioStream) -> bool {
    caps.audio.codecs.contains(&s.codec) && s.channels <= caps.audio.max_channels
}

fn audio_copy_reasons(caps: &Capabilities, s: &AudioStream) -> Vec<Reason> {
    let mut reasons = Vec::new();
    if !caps.audio.codecs.contains(&s.codec) {
        reasons.push(Reason::new(
            "audio_codec_unsupported",
            format!("client does not decode {}", s.codec),
        ));
    }
    if s.channels > caps.audio.max_channels {
        reasons.push(Reason::new(
            "audio_channels_over",
            format!(
                "{} channels is over {}",
                s.channels, caps.audio.max_channels
            ),
        ));
    }
    if !FMP4_AUDIO.contains(&s.codec.as_str()) {
        reasons.push(Reason::new(
            "audio_not_fmp4",
            format!("{} cannot go into fMP4", s.codec),
        ));
    }
    reasons
}

fn audio_transcode(caps: &Capabilities, s: &AudioStream) -> AudioAction {
    let eac3 = caps.audio.codecs.iter().any(|c| c == "eac3") && s.channels > 2;
    if eac3 {
        AudioAction::Transcode {
            codec: "eac3",
            channels: s.channels.min(caps.audio.max_channels).min(6),
            bitrate: 640_000,
        }
    } else {
        AudioAction::Transcode {
            codec: "aac",
            channels: s.channels.min(caps.audio.max_channels).min(2),
            bitrate: 192_000,
        }
    }
}

fn pick_audio<'a>(a: &'a Analysis, p: &SessionParams) -> Result<Option<&'a AudioStream>, ApiError> {
    if let Some(i) = p.audio_index {
        return a
            .audio
            .iter()
            .find(|s| s.index == i)
            .map(Some)
            .ok_or_else(|| ApiError::invalid_request(format!("no audio stream {i}")));
    }
    Ok(a.audio.iter().find(|s| s.default).or(a.audio.first()))
}

fn transcode_video(
    caps: &Capabilities,
    a: &Analysis,
    v: &VideoStream,
    p: &SessionParams,
    enc: &EncoderSet,
) -> Result<VideoAction, ApiError> {
    let order = if enc.hevc_first {
        [VideoCodec::Hevc, VideoCodec::Av1, VideoCodec::H264]
    } else {
        [VideoCodec::Av1, VideoCodec::Hevc, VideoCodec::H264]
    };
    let (codec, cap) = order
        .into_iter()
        .filter(|c| enc.has(*c))
        .find_map(|c| {
            caps.video
                .iter()
                .find(|vc| vc.codec == c.name())
                .map(|vc| (c, vc))
        })
        .ok_or_else(|| {
            ApiError::transcode_unavailable("no encoder for a codec the client decodes")
        })?;

    let source_hdr = v.hdr != Hdr::Sdr || v.dv.is_some();
    let depth_ok = cap.profiles.is_empty()
        || has_profile(&cap.profiles, "main10")
        || (codec == VideoCodec::Av1 && has_profile(&cap.profiles, "main"));
    let hdr_out = if !source_hdr {
        HdrOut::Keep(Hdr::Sdr)
    } else if hdr_listed(caps, "hdr10") && codec != VideoCodec::H264 && depth_ok {
        HdrOut::Keep(Hdr::Hdr10)
    } else {
        HdrOut::Tonemap
    };

    let ladder = p.max_bitrate.map_or(u32::MAX, |b| ladder_height(b, codec));
    let sar = if v.sar.0 == 0 || v.sar.1 == 0 {
        1.0
    } else {
        f64::from(v.sar.0) / f64::from(v.sar.1)
    };
    let aspect = f64::from(v.width) * sar / f64::from(v.height.max(1));
    let mut height = even_down(v.height.min(ladder).min(cap.max_height));
    let mut width = even_down((aspect * f64::from(height)).round() as u32);
    if width > cap.max_width {
        width = even_down(cap.max_width);
        height = even_down((f64::from(width) / aspect).round() as u32);
    }

    // A probe can report no bitrate, and a target of 0 is not an encode.
    let source = match a.bit_rate {
        0 => MAX_TRANSCODE_BITRATE,
        known => known,
    };
    // Above 1 when an anamorphic source is squared up: those pixels carry no new detail.
    let pixels = (f64::from(width) * f64::from(height)
        / (f64::from(v.width) * f64::from(v.height)))
    .min(1.0);
    let min_at_1080p = if codec == VideoCodec::H264 {
        MIN_H264_BITRATE_AT_1080P
    } else {
        MIN_BITRATE_AT_1080P
    };
    // Scaling alone starves compressed sources: on the 7900 XT, a 4.8 Mbit/s 4K film at 1.2 Mbit/s
    // scored VMAF 58 on its worst frames, against 79 at this minimum.
    let floor =
        (min_at_1080p as f64 * f64::from(width) * f64::from(height) / (1920.0 * 1080.0)) as u64;
    let bitrate = ((source as f64 * pixels) as u64)
        .max(floor.min(source))
        .min(p.max_bitrate.unwrap_or(u64::MAX))
        .min(MAX_TRANSCODE_BITRATE);
    let deinterlace = !matches!(v.field_order.as_str(), "progressive" | "unknown" | "");

    Ok(VideoAction::Transcode {
        codec,
        width,
        height,
        bitrate,
        hdr_out,
        deinterlace,
    })
}

fn even_down(n: u32) -> u32 {
    n & !1
}

pub fn decide(
    caps: &Capabilities,
    a: &Analysis,
    p: &SessionParams,
    enc: &EncoderSet,
) -> Result<Plan, ApiError> {
    let v = a.video.first().ok_or_else(ApiError::no_video_stream)?;
    if let Some(i) = p.burn_in_subtitle
        && !a.subtitles.iter().any(|s| s.index == i)
    {
        return Err(ApiError::invalid_request(format!("no subtitle stream {i}")));
    }
    let audio = pick_audio(a, p)?;

    let (mut video_reasons, fit) = video_fit(caps, a, v, p, enc);
    let (mut audio_reasons, audio_copy) = match audio {
        Some(s) => {
            let r = audio_copy_reasons(caps, s);
            let ok = r.is_empty();
            (r, ok)
        }
        None => (Vec::new(), true),
    };

    let container_ok = caps.containers.contains(&a.container);
    let audio_direct = audio.is_none_or(|s| audio_direct_fits(caps, s));

    let mode = match fit {
        Some((_, dv)) if container_ok && dv == DvHandling::Keep && audio_direct => Mode::Direct,
        Some(_) if audio_copy => Mode::Remux,
        Some(_) => Mode::AudioTranscode,
        None => Mode::Transcode,
    };

    let video = match (mode, fit) {
        (Mode::Transcode, _) | (_, None) => transcode_video(caps, a, v, p, enc)?,
        (_, Some((tag, dv))) => VideoAction::Copy { tag, dv },
    };
    let audio_action = match (mode, audio) {
        (Mode::Direct, _) | (_, None) => AudioAction::Copy,
        (_, Some(_)) if audio_copy => AudioAction::Copy,
        (_, Some(s)) => audio_transcode(caps, s),
    };

    if mode == Mode::Direct {
        audio_reasons.clear();
    } else if !container_ok {
        video_reasons.insert(
            0,
            Reason::new(
                "container_unsupported",
                format!("client does not open {}", a.container),
            ),
        );
    }

    Ok(Plan {
        mode,
        video_index: v.index,
        video,
        audio_index: audio.map(|s| s.index),
        audio: audio_action,
        video_reasons,
        audio_reasons,
        burn_in_subtitle: p.burn_in_subtitle,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::analysis::{Dovi, parse};
    use crate::caps::{AudioCaps, HdrCaps, VideoCap};

    fn fixture(name: &str) -> Analysis {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ffprobe");
        let read = |ext: &str| std::fs::read_to_string(dir.join(format!("{name}{ext}"))).unwrap();
        parse(
            &serde_json::from_str(&read(".json")).unwrap(),
            &serde_json::from_str(&read(".frames.json")).unwrap(),
            &read(".packets.csv"),
        )
        .unwrap()
    }

    fn vcap(codec: &str, profiles: &[&str]) -> VideoCap {
        VideoCap {
            codec: codec.into(),
            profiles: profiles.iter().map(|s| s.to_string()).collect(),
            max_level: None,
            max_width: 3840,
            max_height: 2160,
            max_fps: 120.0,
        }
    }

    fn caps(
        video: Vec<VideoCap>,
        hdr: &[&str],
        audio: &[&str],
        containers: &[&str],
    ) -> Capabilities {
        Capabilities {
            video,
            hdr: HdrCaps {
                formats: hdr.iter().map(|s| s.to_string()).collect(),
                dv_profiles: vec![],
            },
            audio: AudioCaps {
                codecs: audio.iter().map(|s| s.to_string()).collect(),
                max_channels: 6,
                passthrough: vec![],
            },
            containers: containers.iter().map(|s| s.to_string()).collect(),
            subtitles: vec![],
            hls_keyframe_exact: false,
        }
    }

    const ALL: EncoderSet = EncoderSet {
        h264: true,
        hevc: true,
        av1: true,
        hevc_first: false,
        dovi_tool: true,
    };

    fn plan(c: &Capabilities, a: &Analysis, p: &SessionParams) -> Plan {
        decide(c, a, p, &ALL).unwrap()
    }

    fn codes(r: &[Reason]) -> Vec<&'static str> {
        r.iter().map(|r| r.code).collect()
    }

    fn hevc_caps(containers: &[&str], hdr: &[&str]) -> Capabilities {
        let mut c = caps(
            vec![vcap("hevc", &["main", "main_10"])],
            hdr,
            &["eac3", "truehd", "ac3"],
            containers,
        );
        c.audio.max_channels = 8;
        c
    }

    #[test]
    fn direct_when_everything_fits() {
        let a = fixture("h264_aac_mkv");
        let c = caps(vec![vcap("h264", &[])], &[], &["ac3"], &["mkv"]);
        let p = plan(&c, &a, &SessionParams::default());
        assert_eq!(p.mode, Mode::Direct);
        assert!(p.video_reasons.is_empty() && p.audio_reasons.is_empty());
    }

    #[test]
    fn remux_for_avplayer_mkv() {
        let a = fixture("h264_aac_mkv");
        let c = caps(vec![vcap("h264", &[])], &[], &["ac3"], &["mp4"]);
        let p = plan(&c, &a, &SessionParams::default());
        assert_eq!(p.mode, Mode::Remux);
        assert_eq!(codes(&p.video_reasons), ["container_unsupported"]);
    }

    #[test]
    fn truehd_forces_audio_transcode_in_hls() {
        let a = fixture("hevc10_hdr10_truehd_pgs_ass_fonts");
        let c = hevc_caps(&["mp4"], &["hdr10"]);
        let p = plan(&c, &a, &SessionParams::default());
        assert_eq!(p.mode, Mode::AudioTranscode);
        assert_eq!(
            p.audio,
            AudioAction::Transcode {
                codec: "eac3",
                channels: 6,
                bitrate: 640_000
            }
        );
        assert!(codes(&p.audio_reasons).contains(&"audio_not_fmp4"));
    }

    #[test]
    fn truehd_direct_when_client_decodes_it() {
        let a = fixture("hevc10_hdr10_truehd_pgs_ass_fonts");
        let c = hevc_caps(&["mkv"], &["hdr10"]);
        assert_eq!(plan(&c, &a, &SessionParams::default()).mode, Mode::Direct);
    }

    #[test]
    fn hevc444_transcodes() {
        let a = fixture("hevc444");
        let c = caps(
            vec![vcap("hevc", &[]), vcap("h264", &[])],
            &[],
            &["aac"],
            &["mkv"],
        );
        let p = plan(&c, &a, &SessionParams::default());
        assert_eq!(p.mode, Mode::Transcode);
        assert!(codes(&p.video_reasons).contains(&"chroma_unsupported"));
    }

    #[test]
    fn codec_choice_prefers_av1() {
        let a = fixture("hevc444");
        let c = caps(
            vec![vcap("hevc", &[]), vcap("av1", &[])],
            &[],
            &["aac"],
            &["mkv"],
        );
        let codec = |enc: &EncoderSet| match decide(&c, &a, &SessionParams::default(), enc)
            .unwrap()
            .video
        {
            VideoAction::Transcode { codec, .. } => codec,
            other => panic!("{other:?}"),
        };
        assert_eq!(codec(&ALL), VideoCodec::Av1);
        assert_eq!(codec(&EncoderSet { av1: false, ..ALL }), VideoCodec::Hevc);
        let hevc_first = EncoderSet {
            hevc_first: true,
            ..ALL
        };
        assert_eq!(codec(&hevc_first), VideoCodec::Hevc);
        assert_eq!(
            codec(&EncoderSet {
                hevc: false,
                ..hevc_first
            }),
            VideoCodec::Av1
        );
    }

    fn capped() -> SessionParams {
        SessionParams {
            max_bitrate: Some(4_000_000),
            ..Default::default()
        }
    }

    #[test]
    fn hdr_kept_on_hdr_display() {
        let a = fixture("hevc10_hdr10_truehd_pgs_ass_fonts");
        let c = hevc_caps(&["mkv"], &["hdr10"]);
        let p = plan(&c, &a, &capped());
        assert_eq!(p.mode, Mode::Transcode);
        assert!(codes(&p.video_reasons).contains(&"bitrate_cap"));
        match p.video {
            VideoAction::Transcode {
                hdr_out, height, ..
            } => {
                assert_eq!(hdr_out, HdrOut::Keep(Hdr::Hdr10));
                assert_eq!(height, a.video[0].height.min(1080));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn hdr_tonemapped_on_sdr_display() {
        let a = fixture("hevc10_hdr10_truehd_pgs_ass_fonts");
        let c = hevc_caps(&["mkv"], &[]);
        match plan(&c, &a, &capped()).video {
            VideoAction::Transcode { hdr_out, .. } => assert_eq!(hdr_out, HdrOut::Tonemap),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn sdr_source_is_not_tonemapped() {
        let mut a = fixture("hevc10_hdr10_truehd_pgs_ass_fonts");
        a.video[0].hdr = Hdr::Sdr;
        a.video[0].dv = None;
        let c = hevc_caps(&["mkv"], &[]);
        match plan(&c, &a, &capped()).video {
            VideoAction::Transcode { hdr_out, .. } => assert_eq!(hdr_out, HdrOut::Keep(Hdr::Sdr)),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn h264_ladder() {
        let a = fixture("hevc10_hdr10_truehd_pgs_ass_fonts");
        let c = caps(vec![vcap("h264", &[])], &[], &["ac3"], &["mkv"]);
        let p = SessionParams {
            max_bitrate: Some(2_000_000),
            ..Default::default()
        };
        match plan(&c, &a, &p).video {
            VideoAction::Transcode { height, .. } => assert_eq!(height, 480),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn ladder_table() {
        use VideoCodec::*;
        assert_eq!(ladder_height(20_000_000, Hevc), 2160);
        assert_eq!(ladder_height(20_000_000, H264), 1080);
        assert_eq!(ladder_height(8_000_000, H264), 1080);
        assert_eq!(ladder_height(4_000_000, H264), 720);
        assert_eq!(ladder_height(1_999_999, Av1), 480);
        assert_eq!(ladder_height(1_999_999, H264), 360);
    }

    fn target_bitrate(
        (width, height): (u32, u32),
        bit_rate: u64,
        max_bitrate: Option<u64>,
        codec: &str,
        (max_width, max_height): (u32, u32),
    ) -> u64 {
        let mut a = fixture("hevc444");
        a.bit_rate = bit_rate;
        a.video[0].width = width;
        a.video[0].height = height;
        let cap = VideoCap {
            max_width,
            max_height,
            ..vcap(codec, &[])
        };
        let c = caps(vec![cap], &[], &["aac"], &["mkv"]);
        let p = SessionParams {
            max_bitrate,
            ..Default::default()
        };
        match plan(&c, &a, &p).video {
            VideoAction::Transcode { bitrate, .. } => bitrate,
            other => panic!("{other:?}"),
        }
    }

    const UHD: (u32, u32) = (3840, 2160);
    const FHD: (u32, u32) = (1920, 1080);

    #[test]
    fn transcode_bitrate_follows_the_pixels() {
        assert_eq!(
            target_bitrate(UHD, 60_000_000, None, "hevc", FHD),
            15_000_000
        );
        assert_eq!(
            target_bitrate(UHD, 16_000_000, None, "hevc", FHD),
            4_000_000
        );
        assert_eq!(
            target_bitrate(UHD, 60_000_000, None, "hevc", UHD),
            40_000_000
        );
    }

    #[test]
    fn transcode_bitrate_never_exceeds_the_source_or_the_cap() {
        assert_eq!(
            target_bitrate(FHD, 5_000_000, Some(20_000_000), "hevc", UHD),
            5_000_000
        );
        assert_eq!(
            target_bitrate(UHD, 60_000_000, Some(8_000_000), "hevc", UHD),
            8_000_000
        );
    }

    #[test]
    fn h264_keeps_a_1080p_minimum_below_the_source_and_cap() {
        assert_eq!(
            target_bitrate(UHD, 16_000_000, None, "h264", FHD),
            8_000_000
        );
        assert_eq!(target_bitrate(UHD, 5_000_000, None, "h264", FHD), 5_000_000);
        assert_eq!(
            target_bitrate(UHD, 60_000_000, Some(4_000_000), "h264", UHD),
            4_000_000
        );
    }

    #[test]
    fn compressed_sources_keep_a_minimum_scaled_by_pixels() {
        assert_eq!(target_bitrate(UHD, 4_800_000, None, "hevc", FHD), 4_000_000);
        assert_eq!(target_bitrate(UHD, 3_000_000, None, "hevc", FHD), 3_000_000);
        assert_eq!(
            target_bitrate((3840, 1608), 4_638_000, None, "hevc", FHD),
            2_977_777
        );
        assert_eq!(
            target_bitrate((3840, 1600), 16_000_000, None, "h264", FHD),
            5_925_925
        );
        assert_eq!(
            target_bitrate(UHD, 4_800_000, Some(500_000), "hevc", FHD),
            500_000
        );
    }

    #[test]
    fn anamorphic_upscale_stays_at_the_source_bitrate() {
        let mut a = fixture("hevc444");
        a.bit_rate = 2_000_000;
        a.video[0].width = 720;
        a.video[0].height = 576;
        a.video[0].sar = (64, 45);
        let c = caps(vec![vcap("hevc", &[])], &[], &["aac"], &["mkv"]);
        match plan(&c, &a, &SessionParams::default()).video {
            VideoAction::Transcode { bitrate, width, .. } => {
                assert_eq!((bitrate, width), (2_000_000, 1024));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn unknown_source_bitrate_falls_back_to_the_limit_and_cap() {
        assert_eq!(target_bitrate(FHD, 0, None, "hevc", FHD), 40_000_000);
        assert_eq!(
            target_bitrate(FHD, 0, Some(20_000_000), "hevc", FHD),
            20_000_000
        );
    }

    #[test]
    fn dv_p81_to_non_dv_hdr10_client() {
        let a = fixture("dv_p81");
        let c = hevc_caps(&["mkv"], &["hdr10"]);
        let ac3 = SessionParams {
            audio_index: Some(2),
            ..Default::default()
        };
        let p = plan(&c, &a, &ac3);
        assert_eq!(p.mode, Mode::Remux);
        assert_eq!(
            p.video,
            VideoAction::Copy {
                tag: CopyTag::Hvc1,
                dv: DvHandling::Strip
            }
        );
    }

    #[test]
    fn dv_p7_to_p8_client() {
        let a = fixture("dv_p7");
        let mut c = hevc_caps(&["mkv"], &["hdr10"]);
        c.hdr.dv_profiles = vec![8];
        let p = plan(&c, &a, &SessionParams::default());
        assert_eq!(p.mode, Mode::Remux);
        assert!(matches!(
            p.video,
            VideoAction::Copy {
                dv: DvHandling::P7ToP81,
                ..
            }
        ));
    }

    #[test]
    fn wide_source_fits_max_width() {
        let mut a = fixture("hevc10_hdr10_truehd_pgs_ass_fonts");
        a.video[0].width = 3840;
        a.video[0].height = 1608;
        a.video[0].sar = (1, 1);
        let mut c = hevc_caps(&["mp4"], &[]);
        c.video[0].max_width = 1920;
        c.video[0].max_height = 1080;
        let p = plan(&c, &a, &SessionParams::default());
        let VideoAction::Transcode { width, height, .. } = p.video else {
            panic!("expected a transcode, got {:?}", p.video)
        };
        assert_eq!((width, height), (1920, 804));
    }

    #[test]
    fn dv_p7_without_dovi_tool_strips_to_base_layer() {
        let a = fixture("dv_p7");
        let mut c = hevc_caps(&["mkv"], &["hdr10"]);
        c.hdr.dv_profiles = vec![8];
        let enc = EncoderSet {
            dovi_tool: false,
            ..ALL
        };
        let p = decide(&c, &a, &SessionParams::default(), &enc).unwrap();
        assert!(matches!(
            p.video,
            VideoAction::Copy {
                tag: CopyTag::Hvc1,
                dv: DvHandling::Strip,
            }
        ));
    }

    #[test]
    fn dv_p7_variable_frame_rate_strips_to_base_layer() {
        let mut a = fixture("dv_p7");
        a.video[0].vfr = true;
        let mut c = hevc_caps(&["mkv"], &["hdr10"]);
        c.hdr.dv_profiles = vec![8];
        let p = plan(&c, &a, &SessionParams::default());
        assert!(matches!(
            p.video,
            VideoAction::Copy {
                dv: DvHandling::Strip,
                ..
            }
        ));
    }

    const SPEC_CAPS: &str = r#"{
      "video": [
        { "codec": "hevc", "profiles": ["main", "main10"], "max_level": 153,
          "max_width": 3840, "max_height": 2160, "max_fps": 60 }
      ],
      "hdr": { "formats": ["hdr10", "hdr10plus", "hlg", "dolby_vision"], "dv_profiles": [5, 8] },
      "audio": { "codecs": ["aac", "ac3", "eac3", "flac", "opus", "truehd"], "max_channels": 6, "passthrough": ["eac3"] },
      "containers": ["mkv", "mp4"],
      "subtitles": ["webvtt", "srt", "ass", "pgs"],
      "hls_keyframe_exact": false
    }"#;

    #[test]
    fn spec_example_caps_accept_hevc_main10() {
        let a = fixture("dv_p81");
        assert_eq!(a.video[0].profile, "Main 10");
        let c: Capabilities = serde_json::from_str(SPEC_CAPS).unwrap();
        let p = plan(&c, &a, &SessionParams::default());
        assert!(!codes(&p.video_reasons).contains(&"profile_unsupported"));
    }

    #[test]
    fn spec_example_caps_accept_av1_main() {
        let a = fixture("av1_dv_p10");
        assert_eq!(a.video[0].profile, "Main");
        let mut c: Capabilities = serde_json::from_str(SPEC_CAPS).unwrap();
        c.video[0].codec = "av1".into();
        let p = plan(&c, &a, &SessionParams::default());
        assert!(!codes(&p.video_reasons).contains(&"profile_unsupported"));
    }

    #[test]
    fn profile_spelling_does_not_matter() {
        let a = fixture("dv_p81");
        for spelling in ["main10", "main_10", "Main 10", "MAIN10"] {
            let c = caps(vec![vcap("hevc", &[spelling])], &[], &["ac3"], &["mkv"]);
            let p = plan(&c, &a, &SessionParams::default());
            assert!(
                !codes(&p.video_reasons).contains(&"profile_unsupported"),
                "{spelling}"
            );
        }
    }

    #[test]
    fn dv_p10_av1_to_non_dv_hdr10_client() {
        let a = fixture("av1_dv_p10");
        let mut c = caps(
            vec![vcap("av1", &["main"])],
            &["hdr10"],
            &["ac3", "eac3"],
            &["mkv"],
        );
        let p = plan(&c, &a, &SessionParams::default());
        assert_eq!(p.mode, Mode::Remux);
        assert_eq!(
            p.video,
            VideoAction::Copy {
                tag: CopyTag::Av01,
                dv: DvHandling::Strip
            }
        );
        c.hdr.dv_profiles = vec![10];
        assert_eq!(
            plan(&c, &a, &SessionParams::default()).video,
            VideoAction::Copy {
                tag: CopyTag::Dav1,
                dv: DvHandling::Keep
            }
        );
    }

    #[test]
    fn dv_p5_to_hdr10_client() {
        let mut a = fixture("dv_p81");
        a.video[0].dv = Some(Dovi {
            profile: 5,
            level: 6,
            bl_compat_id: 0,
        });
        let c = hevc_caps(&["mkv"], &["hdr10"]);
        let p = plan(&c, &a, &SessionParams::default());
        assert_eq!(p.mode, Mode::Transcode);
        assert!(codes(&p.video_reasons).contains(&"dv_profile_unsupported"));
    }

    #[test]
    fn burn_in_forces_transcode() {
        let a = fixture("hevc10_hdr10_truehd_pgs_ass_fonts");
        let pgs = a
            .subtitles
            .iter()
            .find(|s| s.codec.contains("pgs"))
            .unwrap()
            .index;
        let c = hevc_caps(&["mkv"], &["hdr10"]);
        let p = plan(
            &c,
            &a,
            &SessionParams {
                burn_in_subtitle: Some(pgs),
                ..Default::default()
            },
        );
        assert_eq!(p.mode, Mode::Transcode);
        assert!(codes(&p.video_reasons).contains(&"burn_in"));
        assert_eq!(p.burn_in_subtitle, Some(pgs));
    }

    #[test]
    fn no_video_is_422() {
        let mut a = fixture("h264_aac_mkv");
        a.video.clear();
        let c = caps(vec![vcap("h264", &[])], &[], &["ac3"], &["mkv"]);
        let e = decide(&c, &a, &SessionParams::default(), &ALL).unwrap_err();
        assert_eq!(e.code, "no_video_stream");
    }

    #[test]
    fn bad_audio_index_is_400() {
        let a = fixture("h264_aac_mkv");
        let c = caps(vec![vcap("h264", &[])], &[], &["ac3"], &["mkv"]);
        let p = SessionParams {
            audio_index: Some(99),
            ..Default::default()
        };
        assert_eq!(
            decide(&c, &a, &p, &ALL).unwrap_err().code,
            "invalid_request"
        );
        let p = SessionParams {
            burn_in_subtitle: Some(99),
            ..Default::default()
        };
        assert_eq!(
            decide(&c, &a, &p, &ALL).unwrap_err().code,
            "invalid_request"
        );
    }

    #[test]
    fn no_encoder_is_503() {
        let a = fixture("hevc444");
        let c = caps(vec![vcap("hevc", &[])], &[], &["ac3"], &["mkv"]);
        let e = decide(&c, &a, &SessionParams::default(), &EncoderSet::default()).unwrap_err();
        assert_eq!(e.code, "transcode_unavailable");
    }
}
