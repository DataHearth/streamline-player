use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, bail};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HwAccel {
    Auto,
    Vaapi,
    Nvenc,
    Qsv,
    Amf,
    Videotoolbox,
    None,
}

#[derive(Debug, Clone, Deserialize)]
struct PathPair {
    from: String,
    to: PathBuf,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PathMap(Vec<PathPair>);

fn normalize(p: &str) -> String {
    p.replace('\\', "/")
}

fn components(p: &str) -> Vec<&str> {
    p.split('/').filter(|c| !c.is_empty()).collect()
}

impl PathMap {
    pub fn rewrite(&self, p: &str) -> PathBuf {
        let norm = normalize(p);
        let comps = components(&norm);
        let best = self
            .0
            .iter()
            .filter_map(|pair| {
                let from = normalize(&pair.from);
                let from_comps = components(&from);
                comps
                    .starts_with(&from_comps)
                    .then_some((from_comps.len(), pair))
            })
            .max_by_key(|(len, _)| *len);
        match best {
            Some((len, pair)) => {
                let mut out = pair.to.clone();
                out.extend(&comps[len..]);
                out
            }
            None => PathBuf::from(p),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub listen: SocketAddr,
    pub secret: Option<String>,
    pub secret_file: Option<PathBuf>,
    pub tmp_dir: PathBuf,
    pub jobs_dir: PathBuf,
    pub ffmpeg_dir: PathBuf,
    pub hw_accel: HwAccel,
    pub hw_device: String,
    pub max_transcodes: usize,
    pub max_jobs: usize,
    pub max_queued: usize,
    #[serde(with = "humantime_serde")]
    pub orphan_timeout: Duration,
    #[serde(with = "humantime_serde")]
    pub job_output_ttl: Duration,
    pub path_map: PathMap,
}

impl Default for Config {
    fn default() -> Self {
        let temp = std::env::temp_dir();
        Self {
            listen: SocketAddr::from(([0, 0, 0, 0], 8097)),
            secret: None,
            secret_file: None,
            tmp_dir: temp.join("streamline-engine").join("live"),
            jobs_dir: temp.join("streamline-engine").join("jobs"),
            ffmpeg_dir: PathBuf::new(),
            hw_accel: HwAccel::Auto,
            hw_device: String::new(),
            max_transcodes: 4,
            max_jobs: 1,
            max_queued: 64,
            orphan_timeout: Duration::from_secs(30 * 60),
            job_output_ttl: Duration::from_secs(168 * 3600),
            path_map: PathMap::default(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Config> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading config {}", path.display()))?;
        Self::parse(&text).with_context(|| format!("parsing config {}", path.display()))
    }

    fn parse(text: &str) -> anyhow::Result<Config> {
        let mut cfg: Config = serde_yaml_ng::from_str(text)?;
        let defaults = Config::default();
        if cfg.tmp_dir.as_os_str().is_empty() {
            cfg.tmp_dir = defaults.tmp_dir;
        }
        if cfg.jobs_dir.as_os_str().is_empty() {
            cfg.jobs_dir = defaults.jobs_dir;
        }
        match (&cfg.secret, &cfg.secret_file) {
            (Some(_), Some(_)) => bail!("set exactly one of `secret` and `secret_file`, not both"),
            (None, None) => bail!("set one of `secret` or `secret_file`"),
            _ => {}
        }
        Ok(cfg)
    }

    pub fn secret_bytes(&self) -> anyhow::Result<Vec<u8>> {
        match (&self.secret, &self.secret_file) {
            (Some(s), None) => Ok(s.as_bytes().to_vec()),
            (None, Some(f)) => {
                let text = std::fs::read_to_string(f)
                    .with_context(|| format!("reading secret_file {}", f.display()))?;
                Ok(text.trim_end_matches(['\n', '\r']).as_bytes().to_vec())
            }
            _ => bail!("set exactly one of `secret` and `secret_file`"),
        }
    }

    pub fn ffmpeg(&self) -> PathBuf {
        self.binary("ffmpeg")
    }

    pub fn ffprobe(&self) -> PathBuf {
        self.binary("ffprobe")
    }

    pub fn dovi_tool(&self) -> PathBuf {
        self.binary("dovi_tool")
    }

    fn binary(&self, name: &str) -> PathBuf {
        let file = if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.to_owned()
        };
        if self.ffmpeg_dir.as_os_str().is_empty() {
            PathBuf::from(file)
        } else {
            self.ffmpeg_dir.join(file)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_apply() {
        let c = Config::parse("secret: x").unwrap();
        assert_eq!(c.max_transcodes, 4);
        assert_eq!(c.max_jobs, 1);
        assert_eq!(c.max_queued, 64);
        assert_eq!(c.orphan_timeout, Duration::from_secs(30 * 60));
        assert_eq!(c.job_output_ttl, Duration::from_secs(168 * 3600));
        assert_eq!(c.hw_accel, HwAccel::Auto);
        assert_eq!(c.listen, "0.0.0.0:8097".parse().unwrap());
        assert!(c.tmp_dir.starts_with(std::env::temp_dir()));
        assert!(c.jobs_dir.starts_with(std::env::temp_dir()));
    }

    #[test]
    fn explicit_values_and_empty_dirs() {
        let c = Config::parse(
            "secret: x\ntmp_dir: \"\"\nhw_accel: vaapi\norphan_timeout: 5m\nmax_jobs: 2",
        )
        .unwrap();
        assert_eq!(c.hw_accel, HwAccel::Vaapi);
        assert_eq!(c.orphan_timeout, Duration::from_secs(300));
        assert_eq!(c.max_jobs, 2);
        assert!(c.tmp_dir.starts_with(std::env::temp_dir()));
    }

    #[test]
    fn path_map_longest_prefix() {
        let c = Config::parse(
            "secret: x\npath_map:\n  - { from: /data, to: /mnt }\n  - { from: /data/media, to: /srv }\n  - { from: 'C:/media', to: /m }",
        )
        .unwrap();
        let m = &c.path_map;
        assert_eq!(m.rewrite("/data/media/a.mkv"), PathBuf::from("/srv/a.mkv"));
        assert_eq!(m.rewrite("/data/x.mkv"), PathBuf::from("/mnt/x.mkv"));
        assert_eq!(m.rewrite("/database/x"), PathBuf::from("/database/x"));
        assert_eq!(m.rewrite("C:\\media\\a.mkv"), PathBuf::from("/m/a.mkv"));
    }

    #[test]
    fn secret_exclusive() {
        assert!(Config::parse("secret: x\nsecret_file: /tmp/s").is_err());
        assert!(Config::parse("max_jobs: 1").is_err());
    }

    #[test]
    fn secret_file_trims_trailing_newline() {
        let dir = std::env::temp_dir().join(format!("engine-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("secret");
        std::fs::write(&f, "hunter2\n").unwrap();
        let c = Config {
            secret_file: Some(f),
            ..Config::default()
        };
        assert_eq!(c.secret_bytes().unwrap(), b"hunter2");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn binary_names() {
        let mut c = Config::default();
        let bare = if cfg!(windows) {
            "ffmpeg.exe"
        } else {
            "ffmpeg"
        };
        assert_eq!(c.ffmpeg(), PathBuf::from(bare));
        c.ffmpeg_dir = PathBuf::from("/opt/ff");
        assert_eq!(
            c.ffprobe(),
            Path::new("/opt/ff").join(if cfg!(windows) {
                "ffprobe.exe"
            } else {
                "ffprobe"
            })
        );
    }
}
