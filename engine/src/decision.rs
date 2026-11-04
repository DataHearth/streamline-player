use serde::Serialize;

use crate::analysis::{Analysis, AudioStream, Hdr, VideoStream};
use crate::caps::Capabilities;
use crate::error::ApiError;

const MAX_TRANSCODE_BITRATE: u64 = 40_000_000;
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
}

impl EncoderSet {
    fn has(&self, codec: VideoCodec) -> bool {
        match codec {
            VideoCodec::H264 => self.h264,
            VideoCodec::Hevc => self.hevc,
            VideoCodec::Av1 => self.av1,
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
fn p7_conversion_possible(v: &VideoStream) -> bool {
    cfg!(unix) && !v.vfr && v.fps > 0.0
}

fn dv_rule(caps: &Capabilities, v: &VideoStream, profile: u8) -> DvOutcome {
    let av1 = v.codec == "av1";
    let dv_profiles = &caps.hdr.dv_profiles;
    if dv_profiles.contains(&profile) {
        let tag = if av1 { CopyTag::Dav1 } else { CopyTag::Dvh1 };
        return DvOutcome::Fits(tag, DvHandling::Keep);
    }
    if profile == 7 && dv_profiles.contains(&8) && p7_conversion_possible(v) {
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

fn video_fit(
    caps: &Capabilities,
    a: &Analysis,
    v: &VideoStream,
    p: &SessionParams,
) -> (Vec<Reason>, Option<(CopyTag, DvHandling)>) {
    let mut reasons = Vec::new();
    let mut tag = (plain_tag(&v.codec), DvHandling::Keep);

    match caps.video.iter().find(|c| c.codec == v.codec) {
        None => reasons.push(Reason::new(
            "codec_unsupported",
            format!("client does not decode {}", v.codec),
        )),
        Some(cap) => {
            let profile = v.profile.to_lowercase().replace(' ', "_");
            if !cap.profiles.is_empty() && !cap.profiles.contains(&profile) {
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
        Some(profile) => match dv_rule(caps, v, profile) {
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
            format!("{} bit/s is over {max}", a.bit_rate),
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
    let (codec, cap) = [VideoCodec::Av1, VideoCodec::Hevc, VideoCodec::H264]
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
        || cap.profiles.iter().any(|x| x == "main10" || x == "main_10")
        || (codec == VideoCodec::Av1 && cap.profiles.iter().any(|x| x == "main"));
    let hdr_out = if !source_hdr {
        HdrOut::Keep(Hdr::Sdr)
    } else if hdr_listed(caps, "hdr10") && codec != VideoCodec::H264 && depth_ok {
        HdrOut::Keep(Hdr::Hdr10)
    } else {
        HdrOut::Tonemap
    };

    let ladder = p.max_bitrate.map_or(u32::MAX, |b| ladder_height(b, codec));
    let height = even_down(v.height.min(ladder).min(cap.max_height));
    let sar = if v.sar.0 == 0 || v.sar.1 == 0 {
        1.0
    } else {
        f64::from(v.sar.0) / f64::from(v.sar.1)
    };
    let display_width = f64::from(v.width) * sar;
    let width = even_down((display_width * f64::from(height) / f64::from(v.height.max(1))) as u32);

    let bitrate = p
        .max_bitrate
        .unwrap_or(a.bit_rate)
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

    let (mut video_reasons, fit) = video_fit(caps, a, v, p);
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
