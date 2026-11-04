use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde_json::{Value, json};
use tokio::process::Command;

use super::{JobError, lower_priority};
use crate::analysis::{Analysis, SubtitleStream};

fn is_font(mimetype: &str) -> bool {
    let m = mimetype.to_ascii_lowercase();
    m.starts_with("font/")
        || m == "application/x-truetype-font"
        || m == "application/vnd.ms-opentype"
        || m.starts_with("application/x-font-")
}

/// Output file names for one stream, each with the codec options producing it.
fn outputs(s: &SubtitleStream) -> Vec<(String, &'static [&'static str])> {
    let name = |ext: &str| format!("{}.{ext}", s.index);
    match s.codec.as_str() {
        "ass" | "ssa" => vec![
            (name("ass"), &["-c", "copy"]),
            (name("vtt"), &["-c:s", "webvtt"]),
        ],
        "subrip" => vec![
            (name("srt"), &["-c", "copy"]),
            (name("vtt"), &["-c:s", "webvtt"]),
        ],
        "webvtt" => vec![(name("vtt"), &["-c", "copy"])],
        "mov_text" => vec![
            (name("srt"), &["-c:s", "srt"]),
            (name("vtt"), &["-c:s", "webvtt"]),
        ],
        "hdmv_pgs_subtitle" => vec![(name("sup"), &["-c", "copy"])],
        _ => vec![],
    }
}

fn ffmpeg(bin: &Path) -> Command {
    let mut cmd = Command::new(bin);
    cmd.args(["-nostdin", "-y", "-hide_banner", "-loglevel", "error"]);
    cmd
}

async fn run_quiet(mut cmd: Command) -> Result<(bool, String), std::io::Error> {
    lower_priority(&mut cmd);
    let out = cmd.stdin(Stdio::null()).kill_on_drop(true).output().await?;
    Ok((
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).trim().to_owned(),
    ))
}

fn font_file_name(name: &str) -> Option<String> {
    let base = name.replace('\\', "/");
    let base = base.rsplit('/').next()?;
    (!matches!(base, "" | "." | "..")).then(|| base.to_owned())
}

pub async fn run(
    ffmpeg_bin: &Path,
    input: &Path,
    a: &Analysis,
    out: &Path,
) -> Result<Value, JobError> {
    let mut tracks = Vec::new();
    for s in &a.subtitles {
        let outs = outputs(s);
        if outs.is_empty() {
            continue;
        }
        let mut cmd = ffmpeg(ffmpeg_bin);
        cmd.arg("-i").arg(input);
        for (file, opts) in &outs {
            cmd.arg("-map")
                .arg(format!("0:{}", s.index))
                .args(*opts)
                .arg(out.join(file));
        }
        let (ok, stderr) = run_quiet(cmd).await?;
        if !ok {
            return Err(JobError::Ffmpeg {
                index: s.index,
                stderr,
            });
        }
        tracks.push(json!({
            "index": s.index,
            "codec": s.codec,
            "files": outs.iter().map(|(f, _)| f).collect::<Vec<_>>(),
        }));
    }

    let mut fonts = Vec::new();
    for att in a.attachments.iter().filter(|x| is_font(&x.mimetype)) {
        let Some(file) = font_file_name(&att.filename) else {
            continue;
        };
        tokio::fs::create_dir_all(out.join("fonts")).await?;
        let dest: PathBuf = out.join("fonts").join(&file);
        let mut cmd = ffmpeg(ffmpeg_bin);
        cmd.arg(format!("-dump_attachment:{}", att.index))
            .arg(&dest)
            .arg("-i")
            .arg(input);
        // ffmpeg exits non-zero here for lack of an output, so only the file counts.
        run_quiet(cmd).await?;
        if tokio::fs::try_exists(&dest).await? {
            fonts.push(format!("fonts/{file}"));
        }
    }
    Ok(json!({ "tracks": tracks, "fonts": fonts }))
}

#[cfg(test)]
mod tests {
    use std::process::Command as StdCommand;

    use super::*;
    use crate::analysis::run_analyze;

    fn have(tool: &str) -> bool {
        StdCommand::new(tool)
            .arg("-version")
            .output()
            .is_ok_and(|o| o.status.success())
    }

    fn find_ttf() -> Option<PathBuf> {
        if let Some(p) = std::env::var_os("ENGINE_TEST_TTF") {
            return Some(p.into());
        }
        let mut dirs: Vec<PathBuf> = vec![
            "/usr/share/fonts".into(),
            "/run/current-system/sw/share/X11/fonts".into(),
            "/Library/Fonts".into(),
            "/System/Library/Fonts".into(),
            "C:/Windows/Fonts".into(),
        ];
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(&home).join(".nix-profile/share/fonts"));
            dirs.push(PathBuf::from(&home).join(".local/share/fonts"));
        }
        while let Some(dir) = dirs.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    dirs.push(p);
                } else if p.extension().is_some_and(|x| x == "ttf")
                    && std::fs::metadata(&p).is_ok_and(|m| m.len() < 2_000_000)
                {
                    return Some(p);
                }
            }
        }
        None
    }

    const ASS: &str = "[Script Info]\nScriptType: v4.00+\nPlayResX: 640\nPlayResY: 360\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:00.00,0:00:01.50,Default,,0,0,0,,Hello\n";

    #[test]
    fn font_mimetypes() {
        for m in [
            "font/ttf",
            "application/x-truetype-font",
            "application/vnd.ms-opentype",
            "application/x-font-otf",
        ] {
            assert!(is_font(m), "{m}");
        }
        assert!(!is_font("image/png"));
    }

    #[test]
    fn font_names_lose_directories() {
        assert_eq!(font_file_name("../../x.ttf").as_deref(), Some("x.ttf"));
        assert_eq!(font_file_name("a\\b.otf").as_deref(), Some("b.otf"));
        assert_eq!(font_file_name(".."), None);
    }

    #[tokio::test]
    async fn subtitles_real_ffmpeg() {
        if !have("ffmpeg") || !have("ffprobe") {
            return;
        }
        let Some(ttf) = find_ttf() else { return };
        let dir = std::env::temp_dir().join(format!("engine-subs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ass = dir.join("s.ass");
        std::fs::write(&ass, ASS).unwrap();
        let mkv = dir.join("in.mkv");
        let made = StdCommand::new("ffmpeg")
            .args(["-nostdin", "-y", "-v", "error", "-f", "lavfi", "-i"])
            .arg("testsrc2=size=160x90:rate=10:duration=2")
            .arg("-i")
            .arg(&ass)
            .args(["-map", "0", "-map", "1", "-c:v", "mpeg4", "-c:s", "ass"])
            .arg("-attach")
            .arg(&ttf)
            .args(["-metadata:s:t", "mimetype=application/x-truetype-font"])
            .arg(&mkv)
            .output()
            .unwrap();
        assert!(
            made.status.success(),
            "{}",
            String::from_utf8_lossy(&made.stderr)
        );

        let a = run_analyze(Path::new("ffprobe"), &mkv).await.unwrap();
        let out = dir.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let v = run(Path::new("ffmpeg"), &mkv, &a, &out).await.unwrap();

        let n = a.subtitles[0].index;
        let font = format!("fonts/{}", ttf.file_name().unwrap().to_string_lossy());
        assert_eq!(
            v,
            json!({
                "tracks": [{"index": n, "codec": "ass", "files": [format!("{n}.ass"), format!("{n}.vtt")]}],
                "fonts": [font],
            })
        );
        assert!(out.join(format!("{n}.ass")).is_file());
        assert!(out.join(format!("{n}.vtt")).is_file());
        assert!(out.join(&font).is_file());
        std::fs::remove_dir_all(dir).ok();
    }
}
