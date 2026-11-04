use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ANALYSIS_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    #[error("cannot run ffprobe: {0}")]
    Spawn(String),
    #[error("ffprobe cannot read the file: {0}")]
    Unreadable(String),
    #[error("ffprobe output is not valid json: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    pub version: u32,
    #[serde(default)]
    pub container: String,
    #[serde(default)]
    pub duration: f64,
    #[serde(default)]
    pub bit_rate: u64,
    #[serde(default)]
    pub video: Vec<VideoStream>,
    #[serde(default)]
    pub audio: Vec<AudioStream>,
    #[serde(default)]
    pub subtitles: Vec<SubtitleStream>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub chapters: Vec<Chapter>,
    #[serde(default)]
    pub keyframes: Vec<f64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Hdr {
    #[default]
    Sdr,
    Hdr10,
    Hlg,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Dovi {
    #[serde(default)]
    pub profile: u8,
    #[serde(default)]
    pub level: u8,
    #[serde(default)]
    pub bl_compat_id: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VideoStream {
    #[serde(default)]
    pub index: u32,
    #[serde(default)]
    pub codec: String,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub level: Option<u32>,
    #[serde(default)]
    pub bit_depth: u8,
    #[serde(default)]
    pub pix_fmt: String,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub sar: (u32, u32),
    #[serde(default)]
    pub fps: f64,
    #[serde(default)]
    pub vfr: bool,
    #[serde(default)]
    pub field_order: String,
    #[serde(default)]
    pub start_time: f64,
    #[serde(default)]
    pub hdr: Hdr,
    #[serde(default)]
    pub dv: Option<Dovi>,
    #[serde(default)]
    pub hdr10plus: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AudioStream {
    #[serde(default)]
    pub index: u32,
    #[serde(default)]
    pub codec: String,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub channels: u32,
    #[serde(default)]
    pub layout: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub forced: bool,
    #[serde(default)]
    pub start_time: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SubtitleStream {
    #[serde(default)]
    pub index: u32,
    #[serde(default)]
    pub codec: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub forced: bool,
    #[serde(default)]
    pub sdh: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Attachment {
    #[serde(default)]
    pub index: u32,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub mimetype: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Chapter {
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub end: f64,
    #[serde(default)]
    pub title: String,
}

pub fn is_current(a: &Analysis) -> bool {
    a.version >= ANALYSIS_VERSION
}

fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn int(v: &Value) -> Option<i64> {
    num(v).map(|f| f as i64)
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_owned()
}

fn rational(v: &Value, sep: char) -> Option<(u32, u32)> {
    let (a, b) = v.as_str()?.split_once(sep)?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

fn fps(v: &Value) -> Option<f64> {
    let (n, d) = rational(v, '/')?;
    (d != 0 && n != 0).then(|| f64::from(n) / f64::from(d))
}

fn flag(stream: &Value, name: &str) -> bool {
    stream["disposition"][name].as_i64() == Some(1)
}

fn tag(stream: &Value, name: &str) -> String {
    text(&stream["tags"][name])
}

fn bit_depth(stream: &Value) -> u8 {
    if let Some(d) = int(&stream["bits_per_raw_sample"]).filter(|d| *d > 0) {
        return d as u8;
    }
    let pix_fmt = stream["pix_fmt"].as_str().unwrap_or_default();
    for depth in [16u8, 14, 12, 10, 9] {
        if pix_fmt.contains(&depth.to_string()) {
            return depth;
        }
    }
    8
}

fn is_sdh(stream: &Value, title: &str) -> bool {
    flag(stream, "hearing_impaired")
        || title
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .any(|w| w == "sdh" || w == "cc")
}

fn container(format_name: &str) -> String {
    const NAMES: [(&str, &str); 6] = [
        ("matroska", "mkv"),
        ("mov,mp4", "mp4"),
        ("webm", "webm"),
        ("mpegts", "ts"),
        ("avi", "avi"),
        ("mpeg", "mpeg"),
    ];
    NAMES
        .iter()
        .find(|(probe, _)| format_name.contains(probe))
        .map_or_else(String::new, |(_, name)| (*name).to_owned())
}

fn side_data<'a>(stream: &'a Value, kind: &str) -> Option<&'a Value> {
    stream["side_data_list"]
        .as_array()?
        .iter()
        .find(|s| s["side_data_type"].as_str() == Some(kind))
}

fn video_stream(s: &Value, first_frame: &Value) -> VideoStream {
    let hdr = match s["color_transfer"].as_str() {
        Some("smpte2084") => Hdr::Hdr10,
        Some("arib-std-b67") => Hdr::Hlg,
        _ => Hdr::Sdr,
    };
    let avg = fps(&s["avg_frame_rate"]);
    let raw = fps(&s["r_frame_rate"]);
    let hdr10plus = first_frame["frames"][0]["side_data_list"]
        .as_array()
        .is_some_and(|l| {
            l.iter()
                .any(|d| text(&d["side_data_type"]).contains("SMPTE2094-40"))
        });
    VideoStream {
        index: int(&s["index"]).unwrap_or(0) as u32,
        codec: text(&s["codec_name"]),
        profile: text(&s["profile"]),
        level: int(&s["level"]).filter(|l| *l > 0).map(|l| l as u32),
        bit_depth: bit_depth(s),
        pix_fmt: text(&s["pix_fmt"]),
        width: int(&s["width"]).unwrap_or(0) as u32,
        height: int(&s["height"]).unwrap_or(0) as u32,
        sar: rational(&s["sample_aspect_ratio"], ':')
            .filter(|(n, d)| *n != 0 && *d != 0)
            .unwrap_or((1, 1)),
        fps: avg.or(raw).unwrap_or(0.0),
        vfr: matches!((avg, raw), (Some(a), Some(r)) if (a - r).abs() > 0.001),
        field_order: text(&s["field_order"]),
        start_time: num(&s["start_time"]).unwrap_or(0.0),
        hdr,
        dv: side_data(s, "DOVI configuration record").map(|d| Dovi {
            profile: int(&d["dv_profile"]).unwrap_or(0) as u8,
            level: int(&d["dv_level"]).unwrap_or(0) as u8,
            bl_compat_id: int(&d["dv_bl_signal_compatibility_id"]).unwrap_or(0) as u8,
        }),
        hdr10plus,
    }
}

fn keyframes(packets_csv: &str) -> Vec<f64> {
    let mut kf: Vec<f64> = packets_csv
        .lines()
        .filter_map(|line| {
            let mut cols = line.split(',');
            let pts = cols.next()?.trim().parse::<f64>().ok()?;
            cols.next()?.contains('K').then_some(pts)
        })
        .collect();
    kf.sort_by(f64::total_cmp);
    kf.dedup();
    kf
}

pub fn parse(
    probe: &Value,
    first_frame: &Value,
    packets_csv: &str,
) -> Result<Analysis, AnalysisError> {
    let streams = probe["streams"].as_array().map_or(&[][..], Vec::as_slice);
    let mut a = Analysis {
        version: ANALYSIS_VERSION,
        container: container(probe["format"]["format_name"].as_str().unwrap_or_default()),
        duration: num(&probe["format"]["duration"]).unwrap_or(0.0),
        bit_rate: num(&probe["format"]["bit_rate"]).unwrap_or(0.0) as u64,
        keyframes: keyframes(packets_csv),
        ..Analysis::default()
    };
    for s in streams {
        let index = int(&s["index"]).unwrap_or(0) as u32;
        let title = tag(s, "title");
        match s["codec_type"].as_str() {
            Some("video") if !flag(s, "attached_pic") => {
                a.video.push(video_stream(s, first_frame));
            }
            Some("audio") => a.audio.push(AudioStream {
                index,
                codec: text(&s["codec_name"]),
                profile: text(&s["profile"]),
                channels: int(&s["channels"]).unwrap_or(0) as u32,
                layout: text(&s["channel_layout"]),
                language: tag(s, "language"),
                default: flag(s, "default"),
                forced: flag(s, "forced"),
                start_time: num(&s["start_time"]).unwrap_or(0.0),
                title,
            }),
            Some("subtitle") => a.subtitles.push(SubtitleStream {
                index,
                codec: text(&s["codec_name"]),
                language: tag(s, "language"),
                default: flag(s, "default"),
                forced: flag(s, "forced"),
                sdh: is_sdh(s, &title),
                title,
            }),
            Some("attachment") => a.attachments.push(Attachment {
                index,
                filename: tag(s, "filename"),
                mimetype: tag(s, "mimetype"),
            }),
            _ => {}
        }
    }
    for c in probe["chapters"].as_array().into_iter().flatten() {
        a.chapters.push(Chapter {
            start: num(&c["start_time"]).unwrap_or(0.0),
            end: num(&c["end_time"]).unwrap_or(0.0),
            title: tag(c, "title"),
        });
    }
    Ok(a)
}

type Prepare<'a> = &'a (dyn Fn(&mut tokio::process::Command) + Sync);

async fn ffprobe(
    ffprobe: &Path,
    args: &[&str],
    path: &Path,
    prepare: Prepare<'_>,
) -> Result<String, AnalysisError> {
    let mut cmd = tokio::process::Command::new(ffprobe);
    cmd.args(["-v", "error"]).args(args).arg(path);
    prepare(&mut cmd);
    let out = cmd
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| AnalysisError::Spawn(format!("{}: {e}", ffprobe.display())))?;
    if !out.status.success() {
        return Err(AnalysisError::Unreadable(
            String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub async fn run_analyze(ffprobe_bin: &Path, path: &Path) -> Result<Analysis, AnalysisError> {
    run_analyze_with(ffprobe_bin, path, &|_| {}).await
}

/// `prepare` runs on each ffprobe command after its program and arguments are set and before stdio is configured.
pub async fn run_analyze_with(
    ffprobe_bin: &Path,
    path: &Path,
    prepare: Prepare<'_>,
) -> Result<Analysis, AnalysisError> {
    let probe = ffprobe(
        ffprobe_bin,
        &[
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            "-show_chapters",
        ],
        path,
        prepare,
    )
    .await?;
    let frame = ffprobe(
        ffprobe_bin,
        &[
            "-select_streams",
            "v:0",
            "-read_intervals",
            "%+#1",
            "-show_frames",
            "-show_entries",
            "frame=side_data_list",
            "-print_format",
            "json",
        ],
        path,
        prepare,
    )
    .await?;
    let packets = ffprobe(
        ffprobe_bin,
        &[
            "-select_streams",
            "v:0",
            "-show_entries",
            "packet=pts_time,flags",
            "-of",
            "csv=p=0",
        ],
        path,
        prepare,
    )
    .await?;
    parse(
        &serde_json::from_str(&probe)?,
        &serde_json::from_str(&frame)?,
        &packets,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn analysis_ignores_unknown_field() {
        let a: Analysis = serde_json::from_str(r#"{"version": 1, "foo": 1, "video": []}"#).unwrap();
        assert_eq!(a.version, 1);
        assert!(is_current(&a));
        assert!(!is_current(&Analysis::default()));
    }

    #[test]
    fn parse_h264() {
        let a = fixture("h264_aac_mkv");
        assert_eq!(a.container, "mkv");
        assert_eq!(a.video.len(), 1);
        let v = &a.video[0];
        assert_eq!(
            (v.codec.as_str(), v.bit_depth, v.pix_fmt.as_str()),
            ("h264", 8, "yuv420p")
        );
        assert_eq!(v.hdr, Hdr::Sdr);
        assert!(!a.keyframes.is_empty());
        assert!(a.keyframes.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(a.keyframes[0], v.start_time);
    }

    #[test]
    fn parse_hdr10_truehd_pgs_ass_fonts() {
        let a = fixture("hevc10_hdr10_truehd_pgs_ass_fonts");
        assert_eq!(a.video[0].hdr, Hdr::Hdr10);
        assert_eq!(a.video[0].bit_depth, 10);
        let truehd: Vec<_> = a.audio.iter().filter(|s| s.codec == "truehd").collect();
        assert_eq!(truehd.len(), 1);
        assert_eq!(truehd[0].channels, 8);
        assert!(a.subtitles.iter().any(|s| s.codec == "hdmv_pgs_subtitle"));
        assert!(a.subtitles.iter().any(|s| s.codec == "ass"));
        assert!(a.subtitles.iter().any(|s| s.forced));
        assert!(
            a.attachments
                .iter()
                .any(|f| f.filename.ends_with(".ttf") || f.filename.ends_with(".otf"))
        );
    }

    #[test]
    fn parse_dv_p81() {
        let dv = fixture("dv_p81").video[0].dv.clone().unwrap();
        assert_eq!((dv.profile, dv.bl_compat_id), (8, 1));
    }

    #[test]
    fn parse_dv_p7() {
        assert_eq!(fixture("dv_p7").video[0].dv.clone().unwrap().profile, 7);
    }

    #[test]
    fn parse_av1_dv_p10() {
        let v = &fixture("av1_dv_p10").video[0];
        assert_eq!(v.codec, "av1");
        let dv = v.dv.clone().unwrap();
        assert_eq!((dv.profile, dv.bl_compat_id), (10, 1));
    }

    #[test]
    fn parse_444() {
        assert!(fixture("hevc444").video[0].pix_fmt.contains("444"));
    }

    #[test]
    fn parse_mpeg2_interlaced() {
        let v = &fixture("mpeg2_interlaced").video[0];
        assert_ne!(v.field_order, "progressive");
        assert!(!v.field_order.is_empty());
    }

    #[test]
    fn sdh_from_title_or_disposition() {
        let s = serde_json::json!({"disposition": {"hearing_impaired": 0}});
        assert!(is_sdh(&s, "English [SDH]"));
        assert!(is_sdh(&s, "English CC"));
        assert!(!is_sdh(&s, "Accent"));
        assert!(is_sdh(
            &serde_json::json!({"disposition": {"hearing_impaired": 1}}),
            ""
        ));
    }
}
