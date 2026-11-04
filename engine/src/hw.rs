use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::config::{Config, HwAccel};
use crate::decision::{EncoderSet, VideoCodec};

const PROBE_TIMEOUT: Duration = Duration::from_secs(20);
const VAAPI_DEFAULT_DEVICE: &str = "/dev/dri/renderD128";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Vaapi,
    Nvenc,
    Qsv,
    Amf,
    Videotoolbox,
    Cpu,
}

#[derive(Debug, Clone)]
pub struct Hw {
    pub backend: Backend,
    pub device: String,
    pub encoders: EncoderSet,
    pub libplacebo: bool,
    pub untested: bool,
    pub hw_decode: bool,
    pub cpu_encoders: EncoderSet,
}

impl Hw {
    pub fn new(backend: Backend, device: String, encoders: EncoderSet, libplacebo: bool) -> Self {
        Self {
            backend,
            device,
            encoders,
            libplacebo,
            untested: !matches!(backend, Backend::Vaapi | Backend::Cpu),
            hw_decode: true,
            cpu_encoders: if backend == Backend::Cpu {
                encoders
            } else {
                EncoderSet::SOFTWARE
            },
        }
    }
}

impl Hw {
    pub fn decoders(&self) -> Vec<&'static str> {
        match self.backend.hwaccel() {
            Some(name) if self.hw_decode => vec![name],
            _ => Vec::new(),
        }
    }
}

fn or_default<'a>(device: &'a str, default: &'a str) -> &'a str {
    if device.is_empty() { default } else { device }
}

fn owned(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| (*a).to_owned()).collect()
}

impl Backend {
    pub fn decode_args(&self, device: &str) -> Vec<String> {
        match self {
            Self::Vaapi => owned(&[
                "-hwaccel",
                "vaapi",
                "-hwaccel_device",
                or_default(device, VAAPI_DEFAULT_DEVICE),
                "-hwaccel_output_format",
                "vaapi",
            ]),
            Self::Nvenc => owned(&[
                "-hwaccel",
                "cuda",
                "-hwaccel_device",
                or_default(device, "0"),
                "-hwaccel_output_format",
                "cuda",
            ]),
            Self::Qsv => owned(&[
                "-hwaccel",
                "qsv",
                "-qsv_device",
                or_default(device, "0"),
                "-hwaccel_output_format",
                "qsv",
            ]),
            Self::Amf => owned(&[
                "-hwaccel",
                "d3d11va",
                "-hwaccel_device",
                or_default(device, "0"),
                "-hwaccel_output_format",
                "d3d11",
            ]),
            Self::Videotoolbox => owned(&["-hwaccel", "videotoolbox"]),
            Self::Cpu => Vec::new(),
        }
    }

    pub fn hwaccel(&self) -> Option<&'static str> {
        match self {
            Self::Vaapi => Some("vaapi"),
            Self::Nvenc => Some("cuda"),
            Self::Qsv => Some("qsv"),
            Self::Amf => Some("d3d11va"),
            Self::Videotoolbox => Some("videotoolbox"),
            Self::Cpu => None,
        }
    }

    pub fn encoder(&self, c: VideoCodec) -> &'static str {
        use VideoCodec::{Av1, H264, Hevc};
        match (self, c) {
            (Self::Vaapi, H264) => "h264_vaapi",
            (Self::Vaapi, Hevc) => "hevc_vaapi",
            (Self::Vaapi, Av1) => "av1_vaapi",
            (Self::Nvenc, H264) => "h264_nvenc",
            (Self::Nvenc, Hevc) => "hevc_nvenc",
            (Self::Nvenc, Av1) => "av1_nvenc",
            (Self::Qsv, H264) => "h264_qsv",
            (Self::Qsv, Hevc) => "hevc_qsv",
            (Self::Qsv, Av1) => "av1_qsv",
            (Self::Amf, H264) => "h264_amf",
            (Self::Amf, Hevc) => "hevc_amf",
            (Self::Amf, Av1) => "av1_amf",
            (Self::Videotoolbox, H264) => "h264_videotoolbox",
            (Self::Videotoolbox, Hevc) => "hevc_videotoolbox",
            (Self::Videotoolbox, Av1) => "",
            (Self::Cpu, H264) => "libx264",
            (Self::Cpu, Hevc) => "libx265",
            (Self::Cpu, Av1) => "libsvtav1",
        }
    }

    pub fn vulkan_derive(&self) -> Option<&'static str> {
        match self {
            Self::Vaapi => Some("vaapi"),
            Self::Nvenc => Some("cuda"),
            Self::Qsv => Some("qsv"),
            Self::Amf => Some("d3d11va"),
            Self::Videotoolbox | Self::Cpu => None,
        }
    }

    pub fn scale_filter(&self, w: u32, h: u32) -> String {
        match self {
            Self::Vaapi => format!("scale_vaapi=w={w}:h={h}"),
            Self::Nvenc => format!("scale_cuda={w}:{h}"),
            Self::Qsv => format!("scale_qsv=w={w}:h={h}"),
            Self::Amf | Self::Videotoolbox | Self::Cpu => format!("scale={w}:{h}"),
        }
    }

    pub(crate) fn init_device_args(&self, device: &str) -> Vec<String> {
        match self {
            Self::Vaapi => owned(&[
                "-init_hw_device",
                &format!("vaapi=hw:{}", or_default(device, VAAPI_DEFAULT_DEVICE)),
                "-filter_hw_device",
                "hw",
            ]),
            Self::Nvenc => owned(&[
                "-init_hw_device",
                &format!("cuda=hw:{}", or_default(device, "0")),
                "-filter_hw_device",
                "hw",
            ]),
            Self::Qsv => owned(&[
                "-init_hw_device",
                &format!("qsv=hw:{}", or_default(device, "0")),
                "-filter_hw_device",
                "hw",
            ]),
            Self::Amf | Self::Videotoolbox | Self::Cpu => Vec::new(),
        }
    }

    fn upload_filter(&self) -> Option<&'static str> {
        match self {
            Self::Vaapi | Self::Nvenc => Some("format=nv12,hwupload"),
            Self::Qsv => Some("format=nv12,hwupload=extra_hw_frames=64"),
            Self::Amf | Self::Videotoolbox | Self::Cpu => None,
        }
    }
}

pub fn candidates(os: &str) -> Vec<Backend> {
    match os {
        "linux" => vec![Backend::Nvenc, Backend::Vaapi, Backend::Cpu],
        "windows" => vec![Backend::Nvenc, Backend::Qsv, Backend::Amf, Backend::Cpu],
        "macos" => vec![Backend::Videotoolbox, Backend::Cpu],
        _ => vec![Backend::Cpu],
    }
}

async fn ffmpeg_succeeds(ffmpeg: &Path, args: &[String]) -> bool {
    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-hide_banner", "-v", "error"])
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    match tokio::time::timeout(PROBE_TIMEOUT, cmd.status()).await {
        Ok(Ok(status)) => status.success(),
        _ => false,
    }
}

async fn encoder_works(ffmpeg: &Path, backend: Backend, device: &str, codec: VideoCodec) -> bool {
    let encoder = backend.encoder(codec);
    if encoder.is_empty() {
        return false;
    }
    let mut args = backend.init_device_args(device);
    args.extend(owned(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=256x144:rate=25:duration=0.4",
    ]));
    if let Some(upload) = backend.upload_filter() {
        args.extend(owned(&["-vf", upload]));
    }
    args.extend(owned(&["-c:v", encoder, "-f", "null", "-"]));
    ffmpeg_succeeds(ffmpeg, &args).await
}

async fn probe_backend(ffmpeg: &Path, backend: Backend, device: &str) -> EncoderSet {
    let (h264, hevc, av1) = tokio::join!(
        encoder_works(ffmpeg, backend, device, VideoCodec::H264),
        encoder_works(ffmpeg, backend, device, VideoCodec::Hevc),
        encoder_works(ffmpeg, backend, device, VideoCodec::Av1),
    );
    EncoderSet { h264, hevc, av1 }
}

async fn libplacebo_works(ffmpeg: &Path) -> bool {
    let args = owned(&[
        "-init_hw_device",
        "vulkan",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=256x144:duration=0.2",
        "-vf",
        "hwupload,libplacebo=w=128:h=72,hwdownload,format=yuv420p",
        "-f",
        "null",
        "-",
    ]);
    ffmpeg_succeeds(ffmpeg, &args).await
}

fn named_backend(accel: HwAccel) -> Option<Backend> {
    match accel {
        HwAccel::Auto => None,
        HwAccel::Vaapi => Some(Backend::Vaapi),
        HwAccel::Nvenc => Some(Backend::Nvenc),
        HwAccel::Qsv => Some(Backend::Qsv),
        HwAccel::Amf => Some(Backend::Amf),
        HwAccel::Videotoolbox => Some(Backend::Videotoolbox),
        HwAccel::None => Some(Backend::Cpu),
    }
}

pub async fn probe(cfg: &Config) -> Hw {
    let ffmpeg = cfg.ffmpeg();
    let order = match named_backend(cfg.hw_accel) {
        Some(Backend::Cpu) => vec![Backend::Cpu],
        Some(b) => vec![b],
        None => candidates(std::env::consts::OS),
    };
    let libplacebo = libplacebo_works(&ffmpeg).await;
    for backend in order {
        let encoders = probe_backend(&ffmpeg, backend, &cfg.hw_device).await;
        if encoders.h264 || encoders.hevc || encoders.av1 {
            let mut hw = Hw::new(backend, cfg.hw_device.clone(), encoders, libplacebo);
            if backend != Backend::Cpu {
                hw.cpu_encoders = probe_backend(&ffmpeg, Backend::Cpu, "").await;
            }
            return hw;
        }
    }
    Hw::new(
        Backend::Cpu,
        String::new(),
        EncoderSet::default(),
        libplacebo,
    )
}

pub async fn ffmpeg_version(ffmpeg: &Path) -> Option<String> {
    let out = Command::new(ffmpeg)
        .arg("-version")
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().next()?;
    version_token(line)
}

fn version_token(banner: &str) -> Option<String> {
    let rest = banner.strip_prefix("ffmpeg version ")?;
    rest.split_whitespace().next().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_the_token_only() {
        assert_eq!(
            super::version_token(
                "ffmpeg version 9.0.2 Copyright (c) 2000-2026 the FFmpeg developers"
            ),
            Some("9.0.2".into())
        );
        assert_eq!(
            super::version_token("ffmpeg version n7.1-3-gabc Copyright (c) x"),
            Some("n7.1-3-gabc".into())
        );
        assert_eq!(super::version_token("not ffmpeg"), None);
    }

    #[test]
    fn decoders_follow_the_gpu_backend() {
        let sw = EncoderSet::SOFTWARE;
        let mut hw = Hw::new(Backend::Vaapi, String::new(), sw, false);
        assert_eq!(hw.decoders(), vec!["vaapi"]);
        hw.hw_decode = false;
        assert!(hw.decoders().is_empty());
        assert!(
            Hw::new(Backend::Cpu, String::new(), sw, false)
                .decoders()
                .is_empty()
        );
    }

    use super::*;
    use crate::config::{Config, HwAccel};

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn golden_decode_args() {
        assert_eq!(
            Backend::Vaapi.decode_args(""),
            s(&[
                "-hwaccel",
                "vaapi",
                "-hwaccel_device",
                "/dev/dri/renderD128",
                "-hwaccel_output_format",
                "vaapi"
            ])
        );
        assert_eq!(
            Backend::Vaapi.decode_args("/dev/dri/renderD129"),
            s(&[
                "-hwaccel",
                "vaapi",
                "-hwaccel_device",
                "/dev/dri/renderD129",
                "-hwaccel_output_format",
                "vaapi"
            ])
        );
        assert_eq!(
            Backend::Nvenc.decode_args(""),
            s(&[
                "-hwaccel",
                "cuda",
                "-hwaccel_device",
                "0",
                "-hwaccel_output_format",
                "cuda"
            ])
        );
        assert_eq!(
            Backend::Qsv.decode_args("1"),
            s(&[
                "-hwaccel",
                "qsv",
                "-qsv_device",
                "1",
                "-hwaccel_output_format",
                "qsv"
            ])
        );
        assert_eq!(
            Backend::Amf.decode_args(""),
            s(&[
                "-hwaccel",
                "d3d11va",
                "-hwaccel_device",
                "0",
                "-hwaccel_output_format",
                "d3d11"
            ])
        );
        assert_eq!(
            Backend::Videotoolbox.decode_args("x"),
            s(&["-hwaccel", "videotoolbox"])
        );
        assert!(Backend::Cpu.decode_args("").is_empty());
    }

    #[test]
    fn golden_encoders() {
        use VideoCodec::*;
        let rows = [
            (Backend::Vaapi, ["h264_vaapi", "hevc_vaapi", "av1_vaapi"]),
            (Backend::Nvenc, ["h264_nvenc", "hevc_nvenc", "av1_nvenc"]),
            (Backend::Qsv, ["h264_qsv", "hevc_qsv", "av1_qsv"]),
            (Backend::Amf, ["h264_amf", "hevc_amf", "av1_amf"]),
            (
                Backend::Videotoolbox,
                ["h264_videotoolbox", "hevc_videotoolbox", ""],
            ),
            (Backend::Cpu, ["libx264", "libx265", "libsvtav1"]),
        ];
        for (b, [h264, hevc, av1]) in rows {
            assert_eq!(b.encoder(H264), h264);
            assert_eq!(b.encoder(Hevc), hevc);
            assert_eq!(b.encoder(Av1), av1);
        }
    }

    #[test]
    fn scale_and_derive() {
        assert_eq!(
            Backend::Vaapi.scale_filter(1280, 720),
            "scale_vaapi=w=1280:h=720"
        );
        assert_eq!(
            Backend::Nvenc.scale_filter(1280, 720),
            "scale_cuda=1280:720"
        );
        assert_eq!(
            Backend::Qsv.scale_filter(1280, 720),
            "scale_qsv=w=1280:h=720"
        );
        assert_eq!(Backend::Amf.scale_filter(1280, 720), "scale=1280:720");
        assert_eq!(Backend::Cpu.scale_filter(1280, 720), "scale=1280:720");
        assert_eq!(Backend::Vaapi.vulkan_derive(), Some("vaapi"));
        assert_eq!(Backend::Nvenc.vulkan_derive(), Some("cuda"));
        assert_eq!(Backend::Qsv.vulkan_derive(), Some("qsv"));
        assert_eq!(Backend::Amf.vulkan_derive(), Some("d3d11va"));
        assert_eq!(Backend::Videotoolbox.vulkan_derive(), None);
        assert_eq!(Backend::Cpu.vulkan_derive(), None);
    }

    #[test]
    fn candidates_per_os() {
        use Backend::*;
        assert_eq!(candidates("linux"), vec![Nvenc, Vaapi, Cpu]);
        assert_eq!(candidates("windows"), vec![Nvenc, Qsv, Amf, Cpu]);
        assert_eq!(candidates("macos"), vec![Videotoolbox, Cpu]);
    }

    #[test]
    fn untested_flags() {
        for b in [
            Backend::Nvenc,
            Backend::Qsv,
            Backend::Amf,
            Backend::Videotoolbox,
        ] {
            assert!(Hw::new(b, String::new(), EncoderSet::default(), false).untested);
        }
        for b in [Backend::Vaapi, Backend::Cpu] {
            assert!(!Hw::new(b, String::new(), EncoderSet::default(), false).untested);
        }
    }

    #[tokio::test]
    async fn probe_cpu_with_real_ffmpeg() {
        let cfg = Config {
            hw_accel: HwAccel::None,
            ..Config::default()
        };
        let ok = std::process::Command::new(cfg.ffmpeg())
            .arg("-version")
            .output()
            .is_ok();
        if !ok {
            return;
        }
        let hw = probe(&cfg).await;
        assert_eq!(hw.backend, Backend::Cpu);
        assert!(hw.encoders.h264 && hw.encoders.hevc);
    }
}
