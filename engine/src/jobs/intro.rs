use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use serde::Serialize;
use tokio::process::Command;

use super::{JobError, lower_priority};
use crate::analysis::{Analysis, Chapter};
use crate::config::Config;

// Chromaprint hashes 4096 samples of 11025 Hz audio with a two thirds overlap.
const ITEM_SECS: f64 = 1365.0 / 11025.0;
const WINDOW_SECS: f64 = 600.0;
const MIN_SHARED_SECS: f64 = 15.0;
const MAX_BIT_ERRORS: u32 = 6;
const MAX_GAP_SECS: f64 = 3.5;
const SHIFTS_TRIED: usize = 16;
/// Values this common are silence or a drone: they vote for every shift and align nothing.
const MAX_POSITIONS_PER_VALUE: usize = 20;
/// Under this share of distinct values, a match is silence shared by both files, not a segment.
const MIN_DISTINCT_SHARE: f64 = 0.25;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Chapters,
    Fingerprint,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Markers {
    pub path: String,
    pub intro: Option<[f64; 2]>,
    pub credits: Option<[f64; 2]>,
    pub source: Option<Source>,
}

fn is_named(title: &str, names: &[&str]) -> bool {
    title
        .split(|c: char| !c.is_alphanumeric())
        .map(|word| word.trim_end_matches(|c: char| c.is_ascii_digit()))
        .any(|word| names.iter().any(|n| word.eq_ignore_ascii_case(n)))
}

fn span(c: &Chapter) -> Option<[f64; 2]> {
    (c.end > c.start).then_some([c.start, c.end])
}

/// Chapter markers: the first opening, and the last ending.
pub fn from_chapters(chapters: &[Chapter]) -> (Option<[f64; 2]>, Option<[f64; 2]>) {
    let opening = |c: &&Chapter| is_named(&c.title, &["op", "opening", "intro"]);
    let ending = |c: &&Chapter| !opening(c) && is_named(&c.title, &["ed", "ending", "credits"]);
    (
        chapters.iter().find(opening).and_then(span),
        chapters.iter().rev().find(ending).and_then(span),
    )
}

/// Longest run of `a` that matches `b` at one shift, as item ranges in each.
pub fn shared_segment(
    a: &[u32],
    b: &[u32],
) -> Option<(std::ops::Range<usize>, std::ops::Range<usize>)> {
    let mut positions: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, v) in a.iter().enumerate() {
        positions.entry(*v).or_default().push(i);
    }
    let mut votes: HashMap<isize, u32> = HashMap::new();
    for (j, v) in b.iter().enumerate() {
        let Some(list) = positions
            .get(v)
            .filter(|l| l.len() <= MAX_POSITIONS_PER_VALUE)
        else {
            continue;
        };
        for i in list {
            *votes.entry(*i as isize - j as isize).or_default() += 1;
        }
    }
    let mut shifts: Vec<(isize, u32)> = votes.into_iter().collect();
    shifts.sort_unstable_by_key(|(shift, count)| (std::cmp::Reverse(*count), *shift));

    let min_len = (MIN_SHARED_SECS / ITEM_SECS) as usize;
    let max_gap = (MAX_GAP_SECS / ITEM_SECS) as usize;
    let mut best: Option<(usize, usize, isize)> = None;
    for (shift, _) in shifts.into_iter().take(SHIFTS_TRIED) {
        let first = shift.max(0) as usize;
        let last = a.len().min((b.len() as isize + shift).max(0) as usize);
        let mut run: Option<(usize, usize)> = None;
        for i in first..last {
            let j = (i as isize - shift) as usize;
            if (a[i] ^ b[j]).count_ones() > MAX_BIT_ERRORS {
                continue;
            }
            run = match run {
                Some((start, end)) if i - end <= max_gap => Some((start, i)),
                _ => Some((i, i)),
            };
            if let Some((start, end)) = run {
                let len = end + 1 - start;
                if best.is_none_or(|(s, e, _)| len > e + 1 - s) && varied(&a[start..=end]) {
                    best = Some((start, end, shift));
                }
            }
        }
    }
    best.filter(|(s, e, _)| e + 1 - s >= min_len)
        .map(|(s, e, shift)| {
            (
                s..e + 1,
                (s as isize - shift) as usize..(e as isize - shift) as usize + 1,
            )
        })
}

fn varied(items: &[u32]) -> bool {
    let mut sorted = items.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    sorted.len() as f64 >= items.len() as f64 * MIN_DISTINCT_SHARE
}

/// For each fingerprint, the longest segment it shares with any other, in seconds.
pub fn longest_shared(prints: &[Option<Vec<u32>>]) -> Vec<Option<[f64; 2]>> {
    let mut best: Vec<Option<std::ops::Range<usize>>> = vec![None; prints.len()];
    for (i, a) in prints.iter().enumerate() {
        let Some(a) = a else { continue };
        for (j, b) in prints.iter().enumerate().skip(i + 1) {
            let Some(b) = b else { continue };
            let Some((ra, rb)) = shared_segment(a, b) else {
                continue;
            };
            for (at, range) in [(i, ra), (j, rb)] {
                if best[at].as_ref().is_none_or(|r| range.len() > r.len()) {
                    best[at] = Some(range);
                }
            }
        }
    }
    best.into_iter()
        .map(|r| r.map(|r| [r.start as f64 * ITEM_SECS, r.end as f64 * ITEM_SECS]))
        .collect()
}

async fn fingerprint(
    ffmpeg: &Path,
    input: &Path,
    start: f64,
    length: f64,
) -> Result<Vec<u32>, JobError> {
    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-nostdin", "-hide_banner", "-loglevel", "error", "-ss"])
        .arg(start.to_string())
        .arg("-t")
        .arg(length.to_string())
        .arg("-i")
        .arg(input)
        .args([
            "-map",
            "0:a:0",
            "-f",
            "chromaprint",
            "-fp_format",
            "raw",
            "-",
        ]);
    lower_priority(&mut cmd);
    let out = cmd.stdin(Stdio::null()).kill_on_drop(true).output().await?;
    if !out.status.success() {
        return Err(JobError::Ffmpeg {
            index: 0,
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        });
    }
    Ok(out
        .stdout
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect())
}

async fn window(
    cfg: &Config,
    inputs: &[Option<(std::path::PathBuf, f64)>],
    tail: bool,
) -> Vec<(f64, Option<Vec<u32>>)> {
    let mut out = Vec::with_capacity(inputs.len());
    for input in inputs {
        let Some((path, duration)) = input else {
            out.push((0.0, None));
            continue;
        };
        // Half the file at most, so a short episode's credits can't pass for its intro.
        let length = WINDOW_SECS.min(duration / 2.0);
        let start = if tail { duration - length } else { 0.0 };
        let print = match fingerprint(&cfg.ffmpeg(), path, start, length).await {
            Ok(print) => Some(print),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "fingerprint failed");
                None
            }
        };
        out.push((start, print));
    }
    out
}

fn shared_at(window: &[(f64, Option<Vec<u32>>)]) -> Vec<Option<[f64; 2]>> {
    let prints: Vec<Option<Vec<u32>>> = window.iter().map(|(_, p)| p.clone()).collect();
    longest_shared(&prints)
        .into_iter()
        .zip(window)
        .map(|(found, (start, _))| found.map(|[s, e]| [s + start, e + start]))
        .collect()
}

/// A season's markers, in the order of `paths`. A file that can't be read gets no markers and
/// doesn't cost the others theirs: Streamline sends the season again when an episode changes.
pub async fn run(cfg: &Config, paths: &[String], analyses: &[Option<Analysis>]) -> Vec<Markers> {
    let mut markers: Vec<Markers> = paths
        .iter()
        .zip(analyses)
        .map(|(path, analysis)| {
            let (intro, credits) = analysis
                .as_ref()
                .map_or((None, None), |a| from_chapters(&a.chapters));
            Markers {
                path: path.clone(),
                source: (intro.is_some() || credits.is_some()).then_some(Source::Chapters),
                intro,
                credits,
            }
        })
        .collect();
    if paths.len() < 2 {
        return markers;
    }
    let inputs: Vec<Option<(std::path::PathBuf, f64)>> = paths
        .iter()
        .zip(analyses)
        .map(|(path, a)| {
            let a = a.as_ref().filter(|a| !a.audio.is_empty())?;
            Some((cfg.path_map.rewrite(path), a.duration))
        })
        .collect();
    let heads = shared_at(&window(cfg, &inputs, false).await);
    let tails = shared_at(&window(cfg, &inputs, true).await);
    for ((m, head), tail) in markers.iter_mut().zip(heads).zip(tails) {
        if m.intro.is_none() && head.is_some() {
            m.intro = head;
            m.source.get_or_insert(Source::Fingerprint);
        }
        if m.credits.is_none() && tail.is_some() {
            m.credits = tail;
            m.source.get_or_insert(Source::Fingerprint);
        }
    }
    markers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter(start: f64, end: f64, title: &str) -> Chapter {
        Chapter {
            start,
            end,
            title: title.into(),
        }
    }

    /// Hash-like values, different for every seed and position.
    fn noise(seed: u32, len: usize) -> Vec<u32> {
        (0..len as u32)
            .map(|i| {
                (i.wrapping_add(seed.wrapping_mul(7_919)))
                    .wrapping_mul(2_654_435_761)
                    .rotate_left(13)
            })
            .collect()
    }

    fn secs(s: f64) -> usize {
        (s / ITEM_SECS) as usize
    }

    #[test]
    fn chapter_names_pick_the_opening_and_the_last_ending() {
        let list = [
            chapter(0.0, 60.0, "Cold open"),
            chapter(60.0, 150.0, "OP"),
            chapter(150.0, 1200.0, "Part A"),
            chapter(1200.0, 1290.0, "ED1"),
            chapter(1290.0, 1300.0, "Preview"),
        ];
        assert_eq!(
            from_chapters(&list),
            (Some([60.0, 150.0]), Some([1200.0, 1290.0]))
        );
        let words = [
            chapter(0.0, 90.0, "Opening Credits"),
            chapter(90.0, 95.0, "Chapter 2"),
            chapter(95.0, 99.0, "End Credits"),
        ];
        assert_eq!(
            from_chapters(&words),
            (Some([0.0, 90.0]), Some([95.0, 99.0]))
        );
        assert_eq!(
            from_chapters(&[chapter(0.0, 10.0, "Stopover"), chapter(10.0, 20.0, "Edge")]),
            (None, None)
        );
    }

    #[test]
    fn finds_a_segment_shared_at_different_positions() {
        let theme = noise(1, secs(90.0));
        let a = [noise(2, secs(60.0)), theme.clone(), noise(3, secs(200.0))].concat();
        let b = [noise(4, secs(5.0)), theme, noise(5, secs(300.0))].concat();
        let (ra, rb) = shared_segment(&a, &b).expect("shared");
        assert_eq!(ra, secs(60.0)..secs(60.0) + secs(90.0));
        assert_eq!(rb, secs(5.0)..secs(5.0) + secs(90.0));
    }

    #[test]
    fn tolerates_flipped_bits_and_short_gaps() {
        let theme = noise(1, secs(60.0));
        let mut other = theme.clone();
        for (i, v) in other.iter_mut().enumerate() {
            if i % 3 != 0 {
                *v ^= 0b101;
            }
            if (secs(20.0)..secs(22.0)).contains(&i) {
                *v = !*v;
            }
        }
        let (ra, _) = shared_segment(&theme, &other).expect("shared");
        assert_eq!(ra.len(), theme.len());
    }

    #[test]
    fn ignores_unrelated_audio_short_matches_and_shared_silence() {
        assert_eq!(
            shared_segment(&noise(1, secs(120.0)), &noise(2, secs(120.0))),
            None
        );
        let brief = noise(3, secs(10.0));
        let a = [noise(4, secs(50.0)), brief.clone(), noise(5, secs(50.0))].concat();
        let b = [noise(6, secs(70.0)), brief, noise(7, secs(30.0))].concat();
        assert_eq!(shared_segment(&a, &b), None);
        let silence = vec![0u32; secs(60.0)];
        assert_eq!(shared_segment(&silence, &silence), None);
    }

    #[test]
    fn each_file_keeps_its_longest_match_and_a_missing_file_gets_none() {
        let theme = noise(1, secs(40.0));
        let a = [noise(2, secs(30.0)), theme.clone(), noise(3, secs(100.0))].concat();
        let b = [theme.clone(), noise(4, secs(100.0))].concat();
        let found = longest_shared(&[Some(a), None, Some(b)]);
        let [start, end] = found[0].expect("a");
        assert!(
            (start - 30.0).abs() < 0.2 && (end - 70.0).abs() < 0.2,
            "{start} {end}"
        );
        assert_eq!(found[1], None);
        let [start, _] = found[2].expect("b");
        assert!(start.abs() < 0.2, "{start}");
    }
}
