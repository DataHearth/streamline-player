use std::fmt::Write;

use crate::analysis::{Analysis, Hdr, VideoStream};
use crate::args::TRANSCODE_SEGMENT_SECS;
use crate::decision::{AudioAction, CopyTag, DvHandling, HdrOut, Plan, VideoAction, VideoCodec};

fn round_micros(t: f64) -> f64 {
    (t * 1e6).round() / 1e6
}

pub fn boundaries(plan: &Plan, a: &Analysis) -> Vec<f64> {
    match plan.video {
        VideoAction::Copy { .. } if !a.keyframes.is_empty() => {
            let first = a.keyframes[0];
            let mut out: Vec<f64> = a
                .keyframes
                .iter()
                .map(|k| round_micros(k - first))
                .collect();
            out[0] = 0.0;
            out
        }
        VideoAction::Copy { .. } => vec![0.0],
        VideoAction::Transcode { .. } => {
            let mut out = vec![0.0];
            let mut n = 1.0;
            while n * TRANSCODE_SEGMENT_SECS < a.duration {
                out.push(n * TRANSCODE_SEGMENT_SECS);
                n += 1.0;
            }
            out
        }
    }
}

pub fn segment_for(bounds: &[f64], t: f64) -> u32 {
    bounds.partition_point(|b| *b <= t).saturating_sub(1) as u32
}

pub fn media_playlist(bounds: &[f64], duration: f64) -> String {
    let durations: Vec<f64> = bounds
        .iter()
        .enumerate()
        .map(|(i, start)| bounds.get(i + 1).copied().unwrap_or(duration) - start)
        .collect();
    let target = durations.iter().copied().fold(0.0, f64::max).ceil() as u64;

    let mut out = String::from("#EXTM3U\n#EXT-X-VERSION:7\n");
    let _ = writeln!(out, "#EXT-X-TARGETDURATION:{target}");
    out.push_str(
        "#EXT-X-PLAYLIST-TYPE:VOD\n#EXT-X-INDEPENDENT-SEGMENTS\n#EXT-X-MAP:URI=\"init.mp4\"\n",
    );
    for (i, d) in durations.iter().enumerate() {
        let _ = writeln!(out, "#EXTINF:{d:.6},\nseg_{i}.m4s");
    }
    out.push_str("#EXT-X-ENDLIST\n");
    out
}

fn source_video<'a>(plan: &Plan, a: &'a Analysis) -> Option<&'a VideoStream> {
    a.video
        .iter()
        .find(|v| v.index == plan.video_index)
        .or_else(|| a.video.first())
}

fn is_ten_bit(v: &VideoStream) -> bool {
    v.bit_depth >= 10 || v.profile.contains("10")
}

fn h264_profile_byte(profile: &str) -> &'static str {
    let p = profile.to_lowercase();
    if p.contains("baseline") {
        "42"
    } else if p == "main" {
        "4D"
    } else {
        "64"
    }
}

fn av1_level_idx(height: u32) -> &'static str {
    if height <= 1080 { "08" } else { "13" }
}

fn av1_attr(height: u32, ten_bit: bool) -> String {
    format!(
        "av01.0.{}M.{}",
        av1_level_idx(height),
        if ten_bit { "10" } else { "08" }
    )
}

fn hevc_attr(ten_bit: bool, level: u32) -> String {
    format!("hvc1.{}.L{level}.B0", if ten_bit { "2.4" } else { "1.6" })
}

fn hevc_default_level(height: u32) -> u32 {
    if height <= 1080 { 120 } else { 150 }
}

fn audio_attr(codec: &str) -> Option<&'static str> {
    Some(match codec {
        "aac" => "mp4a.40.2",
        "mp3" => "mp4a.40.34",
        "ac3" => "ac-3",
        "eac3" => "ec-3",
        "flac" => "fLaC",
        "opus" => "Opus",
        _ => return None,
    })
}

fn video_attr(plan: &Plan, a: &Analysis) -> (Option<String>, Option<String>) {
    match &plan.video {
        VideoAction::Transcode {
            codec,
            height,
            hdr_out,
            ..
        } => {
            let ten_bit = matches!(hdr_out, HdrOut::Keep(h) if *h != Hdr::Sdr);
            let attr = match codec {
                VideoCodec::H264 => "avc1.640029".to_string(),
                VideoCodec::Hevc => hevc_attr(ten_bit, hevc_default_level(*height)),
                VideoCodec::Av1 => av1_attr(*height, ten_bit),
            };
            (Some(attr), None)
        }
        VideoAction::Copy { tag, dv } => {
            let Some(v) = source_video(plan, a) else {
                return (None, None);
            };
            let dovi = v.dv.as_ref();
            match tag {
                CopyTag::Dvh1 | CopyTag::Dav1 => {
                    let (profile, level) = dovi.map_or((8, 0), |d| (d.profile, d.level));
                    let profile = if *dv == DvHandling::P7ToP81 {
                        8
                    } else {
                        profile
                    };
                    let family = if *tag == CopyTag::Dav1 {
                        "dav1"
                    } else {
                        "dvh1"
                    };
                    (Some(format!("{family}.{profile:02}.{level:02}")), None)
                }
                CopyTag::Avc1 => {
                    let level = v.level.unwrap_or(41);
                    (
                        Some(format!(
                            "avc1.{}00{level:02X}",
                            h264_profile_byte(&v.profile)
                        )),
                        None,
                    )
                }
                CopyTag::Av01 => (Some(av1_attr(v.height, is_ten_bit(v))), None),
                CopyTag::Hvc1 | CopyTag::Other => {
                    let level = v.level.unwrap_or_else(|| hevc_default_level(v.height));
                    let base = hevc_attr(is_ten_bit(v), level);
                    let supplemental = match (dv, dovi) {
                        (DvHandling::Keep, Some(d)) if d.bl_compat_id == 1 => {
                            Some(format!("dvh1.08.{:02}/db1p", d.level))
                        }
                        _ => None,
                    };
                    (Some(base), supplemental)
                }
            }
        }
    }
}

fn audio_codec_attr(plan: &Plan, a: &Analysis) -> Option<&'static str> {
    match &plan.audio {
        AudioAction::Transcode { codec, .. } => audio_attr(codec),
        AudioAction::Copy => {
            let stream = match plan.audio_index {
                Some(i) => a.audio.iter().find(|s| s.index == i),
                None => None,
            };
            audio_attr(&stream?.codec)
        }
    }
}

pub fn codecs_attr(plan: &Plan, a: &Analysis) -> (String, Option<String>) {
    let (video, supplemental) = video_attr(plan, a);
    let codecs: Vec<String> = video
        .into_iter()
        .chain(audio_codec_attr(plan, a).map(str::to_string))
        .collect();
    (codecs.join(","), supplemental)
}

fn video_range(plan: &Plan, a: &Analysis) -> &'static str {
    let hdr = match &plan.video {
        VideoAction::Transcode { hdr_out, .. } => match hdr_out {
            HdrOut::Keep(h) => *h,
            HdrOut::Tonemap => Hdr::Sdr,
        },
        VideoAction::Copy { dv, .. } => {
            source_video(plan, a).map_or(Hdr::Sdr, |v| match (&v.dv, *dv, v.hdr) {
                (Some(_), DvHandling::Keep | DvHandling::P7ToP81, Hdr::Sdr) => Hdr::Hdr10,
                _ => v.hdr,
            })
        }
    };
    match hdr {
        Hdr::Sdr => "SDR",
        Hdr::Hdr10 => "PQ",
        Hdr::Hlg => "HLG",
    }
}

fn resolution(plan: &Plan, a: &Analysis) -> (u32, u32) {
    match &plan.video {
        VideoAction::Transcode { width, height, .. } => (*width, *height),
        VideoAction::Copy { .. } => source_video(plan, a).map_or((0, 0), |v| (v.width, v.height)),
    }
}

pub fn master_playlist(plan: &Plan, a: &Analysis, bandwidth: u64) -> String {
    let (codecs, supplemental) = codecs_attr(plan, a);
    let (w, h) = resolution(plan, a);
    let mut line = format!(
        "#EXT-X-STREAM-INF:BANDWIDTH={bandwidth},CODECS=\"{codecs}\",RESOLUTION={w}x{h},VIDEO-RANGE={}",
        video_range(plan, a)
    );
    if let Some(s) = supplemental {
        let _ = write!(line, ",SUPPLEMENTAL-CODECS=\"{s}\"");
    }
    format!("#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-INDEPENDENT-SEGMENTS\n{line}\nmedia.m3u8\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{AudioStream, Dovi};

    fn plan(video: VideoAction) -> Plan {
        Plan {
            mode: crate::decision::Mode::Remux,
            video_index: 0,
            video,
            audio_index: Some(1),
            audio: AudioAction::Copy,
            video_reasons: vec![],
            audio_reasons: vec![],
            burn_in_subtitle: None,
        }
    }

    fn copy(tag: CopyTag, dv: DvHandling) -> Plan {
        plan(VideoAction::Copy { tag, dv })
    }

    fn transcode(codec: VideoCodec, height: u32, hdr_out: HdrOut) -> Plan {
        plan(VideoAction::Transcode {
            codec,
            width: height * 16 / 9,
            height,
            bitrate: 8_000_000,
            hdr_out,
            deinterlace: false,
        })
    }

    fn analysis(v: VideoStream, audio: &str) -> Analysis {
        Analysis {
            video: vec![v],
            audio: vec![AudioStream {
                index: 1,
                codec: audio.into(),
                ..Default::default()
            }],
            duration: 20.5,
            ..Default::default()
        }
    }

    fn stream(codec: &str, profile: &str, level: u32, w: u32, h: u32, depth: u8) -> VideoStream {
        VideoStream {
            index: 0,
            codec: codec.into(),
            profile: profile.into(),
            level: Some(level),
            width: w,
            height: h,
            bit_depth: depth,
            ..Default::default()
        }
    }

    fn assert_close(got: &[f64], want: &[f64]) {
        assert_eq!(got.len(), want.len(), "{got:?}");
        for (g, w) in got.iter().zip(want) {
            assert!((g - w).abs() < 1e-6, "{got:?} != {want:?}");
        }
    }

    #[test]
    fn copy_boundaries_are_keyframes() {
        let a = Analysis {
            keyframes: vec![0.042, 2.044, 3.003, 9.0],
            ..Default::default()
        };
        let b = boundaries(&copy(CopyTag::Hvc1, DvHandling::Keep), &a);
        assert_close(&b, &[0.0, 2.002, 2.961, 8.958]);
    }

    #[test]
    fn transcode_boundaries_every_6s() {
        let a = Analysis {
            duration: 20.5,
            ..Default::default()
        };
        let b = boundaries(
            &transcode(VideoCodec::H264, 1080, HdrOut::Keep(Hdr::Sdr)),
            &a,
        );
        assert_eq!(b, vec![0.0, 6.0, 12.0, 18.0]);
    }

    #[test]
    fn segment_for_edges() {
        let b = [0.0, 2.0, 5.0];
        assert_eq!(segment_for(&b, 2.0), 1);
        assert_eq!(segment_for(&b, 1.99), 0);
        assert_eq!(segment_for(&b, -1.0), 0);
        assert_eq!(segment_for(&b, 99.0), 2);
    }

    #[test]
    fn media_playlist_golden() {
        let got = media_playlist(&[0.0, 6.0, 12.0, 18.0], 20.5);
        let want = "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-TARGETDURATION:6\n#EXT-X-PLAYLIST-TYPE:VOD\n\
#EXT-X-INDEPENDENT-SEGMENTS\n#EXT-X-MAP:URI=\"init.mp4\"\n\
#EXTINF:6.000000,\nseg_0.m4s\n#EXTINF:6.000000,\nseg_1.m4s\n\
#EXTINF:6.000000,\nseg_2.m4s\n#EXTINF:2.500000,\nseg_3.m4s\n#EXT-X-ENDLIST\n";
        assert_eq!(got, want);
    }

    fn codecs(p: &Plan, a: &Analysis) -> (String, Option<String>) {
        codecs_attr(p, a)
    }

    #[test]
    fn codecs_golden() {
        let h264 = analysis(stream("h264", "High", 41, 1920, 1080, 8), "aac");
        assert_eq!(
            codecs(&copy(CopyTag::Avc1, DvHandling::Keep), &h264),
            ("avc1.640029,mp4a.40.2".into(), None)
        );
        let main = analysis(stream("h264", "Main", 40, 1280, 720, 8), "ac3");
        assert_eq!(
            codecs(&copy(CopyTag::Avc1, DvHandling::Keep), &main).0,
            "avc1.4D0028,ac-3"
        );
        let base = analysis(stream("h264", "Baseline", 30, 640, 480, 8), "mp3");
        assert_eq!(
            codecs(&copy(CopyTag::Avc1, DvHandling::Keep), &base).0,
            "avc1.42001E,mp4a.40.34"
        );

        let hevc = analysis(stream("hevc", "Main", 153, 3840, 2160, 8), "eac3");
        assert_eq!(
            codecs(&copy(CopyTag::Hvc1, DvHandling::Keep), &hevc).0,
            "hvc1.1.6.L153.B0,ec-3"
        );
        let hevc10 = analysis(stream("hevc", "Main 10", 153, 3840, 2160, 10), "flac");
        assert_eq!(
            codecs(&copy(CopyTag::Hvc1, DvHandling::Keep), &hevc10).0,
            "hvc1.2.4.L153.B0,fLaC"
        );

        let mut dv = stream("hevc", "Main 10", 153, 3840, 2160, 10);
        dv.dv = Some(Dovi {
            profile: 8,
            level: 6,
            bl_compat_id: 1,
        });
        let dv = analysis(dv, "opus");
        assert_eq!(
            codecs(&copy(CopyTag::Dvh1, DvHandling::Keep), &dv),
            ("dvh1.08.06,Opus".into(), None)
        );
        assert_eq!(
            codecs(&copy(CopyTag::Hvc1, DvHandling::Keep), &dv),
            (
                "hvc1.2.4.L153.B0,Opus".into(),
                Some("dvh1.08.06/db1p".into())
            )
        );
        assert_eq!(
            codecs(&copy(CopyTag::Hvc1, DvHandling::Strip), &dv),
            ("hvc1.2.4.L153.B0,Opus".into(), None)
        );

        let mut p7 = stream("hevc", "Main 10", 153, 3840, 2160, 10);
        p7.dv = Some(Dovi {
            profile: 7,
            level: 6,
            bl_compat_id: 6,
        });
        assert_eq!(
            codecs(
                &copy(CopyTag::Dvh1, DvHandling::P7ToP81),
                &analysis(p7, "aac")
            )
            .0,
            "dvh1.08.06,mp4a.40.2"
        );

        let mut av1dv = stream("av1", "Main", 13, 3840, 2160, 10);
        av1dv.dv = Some(Dovi {
            profile: 10,
            level: 9,
            bl_compat_id: 1,
        });
        assert_eq!(
            codecs(
                &copy(CopyTag::Dav1, DvHandling::Keep),
                &analysis(av1dv, "aac")
            )
            .0,
            "dav1.10.09,mp4a.40.2"
        );

        let av1 = analysis(stream("av1", "Main", 8, 1920, 1080, 8), "aac");
        assert_eq!(
            codecs(&copy(CopyTag::Av01, DvHandling::Keep), &av1).0,
            "av01.0.08M.08,mp4a.40.2"
        );
        let av1_4k = analysis(stream("av1", "Main", 13, 3840, 2160, 10), "aac");
        assert_eq!(
            codecs(&copy(CopyTag::Av01, DvHandling::Keep), &av1_4k).0,
            "av01.0.13M.10,mp4a.40.2"
        );

        let any = analysis(stream("h264", "High", 41, 1920, 1080, 8), "aac");
        assert_eq!(
            codecs(&transcode(VideoCodec::H264, 1080, HdrOut::Tonemap), &any).0,
            "avc1.640029,mp4a.40.2"
        );
        assert_eq!(
            codecs(
                &transcode(VideoCodec::Hevc, 1080, HdrOut::Keep(Hdr::Sdr)),
                &any
            )
            .0,
            "hvc1.1.6.L120.B0,mp4a.40.2"
        );
        assert_eq!(
            codecs(
                &transcode(VideoCodec::Hevc, 2160, HdrOut::Keep(Hdr::Hdr10)),
                &any
            )
            .0,
            "hvc1.2.4.L150.B0,mp4a.40.2"
        );
        assert_eq!(
            codecs(
                &transcode(VideoCodec::Av1, 1080, HdrOut::Keep(Hdr::Sdr)),
                &any
            )
            .0,
            "av01.0.08M.08,mp4a.40.2"
        );
        assert_eq!(
            codecs(
                &transcode(VideoCodec::Av1, 2160, HdrOut::Keep(Hdr::Hlg)),
                &any
            )
            .0,
            "av01.0.13M.10,mp4a.40.2"
        );
    }

    #[test]
    fn master_hdr10_range() {
        let mut hdr10 = stream("hevc", "Main 10", 153, 3840, 2160, 10);
        hdr10.hdr = Hdr::Hdr10;
        let a = analysis(hdr10, "aac");
        let m = master_playlist(&copy(CopyTag::Hvc1, DvHandling::Keep), &a, 8_000_000);
        assert!(m.contains(
            "#EXT-X-STREAM-INF:BANDWIDTH=8000000,CODECS=\"hvc1.2.4.L153.B0,mp4a.40.2\",RESOLUTION=3840x2160,VIDEO-RANGE=PQ\nmedia.m3u8"
        ));
        let keep = transcode(VideoCodec::Hevc, 2160, HdrOut::Keep(Hdr::Hdr10));
        assert!(master_playlist(&keep, &a, 1).contains("VIDEO-RANGE=PQ"));
        let tone = transcode(VideoCodec::H264, 1080, HdrOut::Tonemap);
        assert!(master_playlist(&tone, &a, 1).contains("VIDEO-RANGE=SDR"));

        let mut hlg = stream("hevc", "Main 10", 153, 1920, 1080, 10);
        hlg.hdr = Hdr::Hlg;
        let m = master_playlist(
            &copy(CopyTag::Hvc1, DvHandling::Keep),
            &analysis(hlg, "aac"),
            1,
        );
        assert!(m.contains("VIDEO-RANGE=HLG"));
    }

    #[test]
    fn master_supplemental_codecs() {
        let mut dv = stream("hevc", "Main 10", 153, 3840, 2160, 10);
        dv.hdr = Hdr::Hdr10;
        dv.dv = Some(Dovi {
            profile: 8,
            level: 6,
            bl_compat_id: 1,
        });
        let m = master_playlist(
            &copy(CopyTag::Hvc1, DvHandling::Keep),
            &analysis(dv, "aac"),
            1,
        );
        assert!(m.contains("VIDEO-RANGE=PQ,SUPPLEMENTAL-CODECS=\"dvh1.08.06/db1p\""));
    }
}
