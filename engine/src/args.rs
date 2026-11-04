use std::path::Path;

use crate::analysis::{Analysis, Hdr};
use crate::decision::{AudioAction, CopyTag, DvHandling, HdrOut, Plan, VideoAction, VideoCodec};
use crate::hw::{Backend, Hw};

pub const TRANSCODE_SEGMENT_SECS: f64 = 6.0;

const CPU_TONEMAP: &str = "format=p010le,zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,tonemap=tonemap=hable:desat=0,zscale=t=bt709:m=bt709:r=tv,format=nv12";

pub struct RunSpec<'a> {
    pub input: &'a Path,
    pub plan: &'a Plan,
    pub analysis: &'a Analysis,
    pub hw: &'a Hw,
    pub start_index: u32,
    pub start_time: f64,
    pub out_dir: &'a Path,
}

fn owned(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| (*a).to_owned()).collect()
}

fn has_upload(b: Backend) -> bool {
    matches!(b, Backend::Vaapi | Backend::Nvenc | Backend::Qsv)
}

fn upload(b: Backend, fmt: &str) -> String {
    match b {
        Backend::Qsv => format!("format={fmt},hwupload=extra_hw_frames=64"),
        _ => format!("format={fmt},hwupload"),
    }
}

fn drop_output_format(decode: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(decode.len());
    let mut it = decode.into_iter();
    while let Some(arg) = it.next() {
        if arg == "-hwaccel_output_format" {
            it.next();
        } else {
            out.push(arg);
        }
    }
    out
}

struct Pipeline {
    init: Vec<String>,
    decode: Vec<String>,
    filter: Option<String>,
    burn_in: bool,
}

fn keeps_hdr(out: HdrOut) -> bool {
    matches!(out, HdrOut::Keep(Hdr::Hdr10 | Hdr::Hlg))
}

fn libplacebo(w: u32, h: u32, out: HdrOut, derive: &str) -> String {
    let (fmt, space, prim, trc) = match out {
        HdrOut::Keep(Hdr::Hdr10) => ("p010", "bt2020nc", "bt2020", "smpte2084"),
        HdrOut::Keep(Hdr::Hlg) => ("p010", "bt2020nc", "bt2020", "arib-std-b67"),
        _ => ("nv12", "bt709", "bt709", "bt709"),
    };
    format!(
        "hwmap=derive_device=vulkan,libplacebo=w={w}:h={h}:format={fmt}:colorspace={space}:color_primaries={prim}:color_trc={trc}:tonemapping=bt.2390:apply_dolbyvision=1,hwmap=derive_device={derive}:reverse=1"
    )
}

fn pipeline(plan: &Plan, a: &Analysis, hw: &Hw) -> Pipeline {
    let VideoAction::Transcode {
        width,
        height,
        hdr_out,
        deinterlace,
        ..
    } = plan.video
    else {
        return Pipeline {
            init: Vec::new(),
            decode: Vec::new(),
            filter: None,
            burn_in: false,
        };
    };
    let b = hw.backend;
    let burn = plan.burn_in_subtitle;
    let software_stage = deinterlace || burn.is_some();
    let tonemap = hdr_out == HdrOut::Tonemap;
    let derive = b.vulkan_derive();
    let placebo = hw.libplacebo
        && derive.is_some()
        && (tonemap || keeps_hdr(hdr_out))
        && (!software_stage || has_upload(b));

    let mut decode = if burn.is_some() || b == Backend::Cpu {
        Vec::new()
    } else {
        b.decode_args(&hw.device)
    };
    if b == Backend::Amf && !placebo {
        decode = drop_output_format(decode);
    }
    let init = if burn.is_some() && has_upload(b) {
        b.init_device_args(&hw.device)
    } else {
        Vec::new()
    };

    let mut on_gpu = decode.iter().any(|d| d == "-hwaccel_output_format");
    let source_fmt = a
        .video
        .iter()
        .find(|v| v.index == plan.video_index)
        .map_or("nv12", |v| if v.bit_depth > 8 { "p010le" } else { "nv12" });
    let out_fmt = if keeps_hdr(hdr_out) { "p010le" } else { "nv12" };

    let mut head = Vec::new();
    if deinterlace {
        if on_gpu {
            head.push(format!("hwdownload,format={source_fmt}"));
            on_gpu = false;
        }
        head.push("bwdif=mode=send_frame".to_owned());
    }

    let mut rest = Vec::new();
    if placebo {
        if !on_gpu {
            rest.push(upload(b, source_fmt));
        }
        rest.push(libplacebo(
            width,
            height,
            hdr_out,
            derive.unwrap_or_default(),
        ));
        on_gpu = true;
    } else if tonemap {
        if on_gpu {
            rest.push("hwdownload".to_owned());
        }
        rest.push(CPU_TONEMAP.to_owned());
        rest.push(format!("scale={width}:{height}"));
        on_gpu = false;
    } else if on_gpu {
        rest.push(b.scale_filter(width, height));
    } else {
        rest.push(format!("scale={width}:{height}"));
    }
    if !on_gpu && has_upload(b) {
        rest.push(upload(b, out_fmt));
    }

    let filter = match burn {
        Some(sub) => {
            let v = plan.video_index;
            let source = if head.is_empty() {
                format!("[0:{v}]")
            } else {
                format!("[0:{v}]{}[d];[d]", head.join(","))
            };
            Some(format!("{source}[0:{sub}]overlay,{}[v]", rest.join(",")))
        }
        None => {
            head.extend(rest);
            Some(head.join(",")).filter(|f| !f.is_empty())
        }
    };
    Pipeline {
        init,
        decode,
        filter,
        burn_in: burn.is_some(),
    }
}

pub fn video_filter(plan: &Plan, a: &Analysis, hw: &Hw) -> Option<String> {
    pipeline(plan, a, hw).filter
}

pub fn downmix_filter(layout: &str) -> Option<String> {
    let (back_l, back_r) = match layout {
        "5.1" | "5.1(back)" => ("0.707*BL", "0.707*BR"),
        "5.1(side)" => ("0.707*SL", "0.707*SR"),
        "7.1" => ("0.707*SL+0.707*BL", "0.707*SR+0.707*BR"),
        _ => return None,
    };
    Some(format!(
        "pan=stereo|FL<1.0*FC+0.707*FL+{back_l}+0.5*LFE|FR<1.0*FC+0.707*FR+{back_r}+0.5*LFE"
    ))
}

fn push(args: &mut Vec<String>, items: &[&str]) {
    args.extend(items.iter().map(|i| (*i).to_owned()));
}

fn video_copy_args(args: &mut Vec<String>, tag: CopyTag, dv: DvHandling) {
    push(args, &["-c:v", "copy"]);
    match tag {
        CopyTag::Hvc1 => push(args, &["-tag:v", "hvc1"]),
        CopyTag::Dvh1 => push(args, &["-tag:v", "dvh1"]),
        CopyTag::Avc1 => push(args, &["-tag:v", "avc1"]),
        CopyTag::Av01 => push(args, &["-tag:v", "av01"]),
        CopyTag::Dav1 => push(args, &["-tag:v", "dav1"]),
        CopyTag::Other => {}
    }
    match dv {
        DvHandling::Keep => {}
        DvHandling::Strip => push(args, &["-bsf:v", "dovi_rpu=strip=1"]),
        DvHandling::P7ToP81 => push(
            args,
            &[
                "-bsf:v",
                "filter_units=remove_types=63,dovi_rpu",
                "-strict",
                "unofficial",
            ],
        ),
    }
}

fn video_transcode_args(args: &mut Vec<String>, plan: &Plan, s: &RunSpec, p: &Pipeline) {
    let VideoAction::Transcode {
        codec,
        bitrate,
        hdr_out,
        ..
    } = plan.video
    else {
        return;
    };
    let b = s.hw.backend;
    args.extend(["-c:v".to_owned(), b.encoder(codec).to_owned()]);
    args.extend([
        "-b:v".to_owned(),
        bitrate.to_string(),
        "-maxrate".to_owned(),
        bitrate.to_string(),
        "-bufsize".to_owned(),
        (bitrate * 2).to_string(),
        "-g".to_owned(),
        "100000".to_owned(),
        "-force_key_frames".to_owned(),
        format!("expr:gte(t,n_forced*{})", TRANSCODE_SEGMENT_SECS),
    ]);
    match b.encoder(codec) {
        "libx264" => push(args, &["-x264-params", "scenecut=0"]),
        "libx265" => push(args, &["-x265-params", "scenecut=0"]),
        "libsvtav1" => push(args, &["-svtav1-params", "scd=0"]),
        _ => {}
    }
    if let Some(filter) = &p.filter {
        if p.burn_in {
            args.extend(["-filter_complex".to_owned(), filter.clone()]);
        } else {
            args.extend(["-vf".to_owned(), filter.clone()]);
        }
    }
    let keep = keeps_hdr(hdr_out);
    if keep {
        if codec == VideoCodec::Hevc {
            push(args, &["-profile:v", "main10"]);
        }
        let trc = if hdr_out == HdrOut::Keep(Hdr::Hlg) {
            "arib-std-b67"
        } else {
            "smpte2084"
        };
        push(
            args,
            &[
                "-color_primaries",
                "bt2020",
                "-color_trc",
                trc,
                "-colorspace",
                "bt2020nc",
            ],
        );
    } else {
        push(
            args,
            &[
                "-color_primaries",
                "bt709",
                "-color_trc",
                "bt709",
                "-colorspace",
                "bt709",
            ],
        );
    }
    if matches!(b, Backend::Cpu | Backend::Videotoolbox) {
        let fmt = match (b, keep) {
            (Backend::Cpu, true) => "yuv420p10le",
            (Backend::Cpu, false) => "yuv420p",
            (_, true) => "p010le",
            (_, false) => "nv12",
        };
        push(args, &["-pix_fmt", fmt]);
    }
}

fn audio_args(args: &mut Vec<String>, s: &RunSpec) {
    match &s.plan.audio {
        AudioAction::Copy => push(args, &["-c:a", "copy"]),
        AudioAction::Transcode {
            codec,
            channels,
            bitrate,
        } => {
            args.extend([
                "-c:a".to_owned(),
                (*codec).to_owned(),
                "-b:a".to_owned(),
                bitrate.to_string(),
                "-ac".to_owned(),
                channels.to_string(),
            ]);
            let source = s
                .analysis
                .audio
                .iter()
                .find(|a| Some(a.index) == s.plan.audio_index);
            if let Some(src) = source
                && *channels == 2
                && src.channels > 2
                && let Some(filter) = downmix_filter(&src.layout)
            {
                args.extend(["-af".to_owned(), filter]);
            }
        }
    }
}

pub fn live_args(s: &RunSpec) -> Vec<String> {
    let plan = s.plan;
    let pipe = pipeline(plan, s.analysis, s.hw);
    let mut args = owned(&[
        "-hide_banner",
        "-nostdin",
        "-v",
        "error",
        "-progress",
        "pipe:1",
        "-stats_period",
        "2",
    ]);
    args.extend(pipe.init.iter().cloned());
    args.extend(pipe.decode.iter().cloned());
    args.extend([
        "-ss".to_owned(),
        s.start_time.to_string(),
        "-copyts".to_owned(),
        "-start_at_zero".to_owned(),
        "-i".to_owned(),
        s.input.display().to_string(),
    ]);

    if pipe.burn_in {
        push(&mut args, &["-map", "[v]"]);
    } else {
        args.extend(["-map".to_owned(), format!("0:{}", plan.video_index)]);
    }
    if let Some(audio) = plan.audio_index {
        args.extend(["-map".to_owned(), format!("0:{audio}")]);
    }
    push(
        &mut args,
        &["-map_metadata", "-1", "-map_chapters", "-1", "-sn", "-dn"],
    );

    match plan.video {
        VideoAction::Copy { tag, dv } => video_copy_args(&mut args, tag, dv),
        VideoAction::Transcode { .. } => video_transcode_args(&mut args, plan, s, &pipe),
    }
    audio_args(&mut args, s);

    let n = s.start_index;
    let dir = s.out_dir.display();
    // ponytail: hls_time 0.001 cuts at every keyframe, one segment per GOP. On
    // 1-second-GOP Blu-ray remuxes that is many small requests; the upgrade is an
    // explicit segmenter that groups GOPs.
    args.extend(owned(&[
        "-f",
        "hls",
        "-hls_segment_type",
        "fmp4",
        "-hls_time",
        "0.001",
        "-hls_playlist_type",
        "event",
        "-hls_flags",
        "independent_segments+temp_file",
        "-start_number",
        &n.to_string(),
        "-hls_fmp4_init_filename",
        &format!("init_{n}.mp4"),
        "-hls_segment_filename",
        &format!("{dir}/seg_%d.m4s"),
        &format!("{dir}/ffmpeg_{n}.m3u8"),
    ]));
    args
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::analysis::{AudioStream, VideoStream};
    use crate::decision::{EncoderSet, Mode};

    fn analysis(interlaced: bool) -> Analysis {
        Analysis {
            version: 1,
            video: vec![VideoStream {
                index: 0,
                codec: "hevc".into(),
                bit_depth: 10,
                pix_fmt: "yuv420p10le".into(),
                width: 3840,
                height: 2160,
                sar: (1, 1),
                field_order: if interlaced { "tt" } else { "progressive" }.into(),
                ..Default::default()
            }],
            audio: vec![AudioStream {
                index: 1,
                codec: "dts".into(),
                channels: 6,
                layout: "5.1(side)".into(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn plan(video: VideoAction, audio: AudioAction) -> Plan {
        Plan {
            mode: Mode::Transcode,
            video_index: 0,
            video,
            audio_index: Some(1),
            audio,
            video_reasons: Vec::new(),
            audio_reasons: Vec::new(),
            burn_in_subtitle: None,
        }
    }

    fn transcode(codec: VideoCodec, hdr_out: HdrOut, deinterlace: bool) -> VideoAction {
        VideoAction::Transcode {
            codec,
            width: 1920,
            height: 1080,
            bitrate: 8_000_000,
            hdr_out,
            deinterlace,
        }
    }

    fn copy(tag: CopyTag, dv: DvHandling) -> VideoAction {
        VideoAction::Copy { tag, dv }
    }

    fn hw(backend: Backend, libplacebo: bool) -> Hw {
        Hw::new(backend, String::new(), EncoderSet::default(), libplacebo)
    }

    fn run(plan: &Plan, hw: &Hw, start_index: u32, start_time: f64) -> Vec<String> {
        let a = analysis(false);
        live_args(&RunSpec {
            input: Path::new("/media/in.mkv"),
            plan,
            analysis: &a,
            hw,
            start_index,
            start_time,
            out_dir: &PathBuf::from("/tmp/s"),
        })
    }

    fn contains_seq(args: &[String], seq: &[&str]) -> bool {
        args.windows(seq.len())
            .any(|w| w.iter().zip(seq).all(|(a, b)| a == b))
    }

    fn value_of<'a>(args: &'a [String], flag: &str) -> &'a str {
        let i = args.iter().position(|a| a == flag).unwrap();
        &args[i + 1]
    }

    const PLACEBO_SDR: &str = "hwmap=derive_device=vulkan,libplacebo=w=1920:h=1080:format=nv12:colorspace=bt709:color_primaries=bt709:color_trc=bt709:tonemapping=bt.2390:apply_dolbyvision=1,hwmap=derive_device=vaapi:reverse=1";

    #[test]
    fn remux_copy() {
        let p = plan(copy(CopyTag::Hvc1, DvHandling::Keep), AudioAction::Copy);
        let args = run(&p, &hw(Backend::Cpu, false), 0, 0.0);
        assert_eq!(
            &args[..8],
            [
                "-hide_banner",
                "-nostdin",
                "-v",
                "error",
                "-progress",
                "pipe:1",
                "-stats_period",
                "2"
            ]
        );
        assert!(contains_seq(
            &args,
            &[
                "-ss",
                "0",
                "-copyts",
                "-start_at_zero",
                "-i",
                "/media/in.mkv"
            ]
        ));
        assert!(contains_seq(&args, &["-map", "0:0", "-map", "0:1"]));
        assert!(contains_seq(
            &args,
            &["-map_metadata", "-1", "-map_chapters", "-1", "-sn", "-dn"]
        ));
        assert!(contains_seq(
            &args,
            &["-c:v", "copy", "-tag:v", "hvc1", "-c:a", "copy"]
        ));
        assert!(contains_seq(&args, &["-hls_time", "0.001"]));
        assert!(contains_seq(&args, &["-start_number", "0"]));
        assert!(!args.contains(&"-hwaccel".to_owned()));
    }

    #[test]
    fn remux_restart_at_segment_12() {
        let p = plan(copy(CopyTag::Hvc1, DvHandling::Keep), AudioAction::Copy);
        let args = run(&p, &hw(Backend::Cpu, false), 12, 72.5);
        assert!(contains_seq(&args, &["-ss", "72.5", "-copyts"]));
        assert!(contains_seq(&args, &["-start_number", "12"]));
        assert!(contains_seq(
            &args,
            &["-hls_fmp4_init_filename", "init_12.mp4"]
        ));
        assert_eq!(args.last().unwrap(), "/tmp/s/ffmpeg_12.m3u8");
        assert!(contains_seq(
            &args,
            &["-hls_segment_filename", "/tmp/s/seg_%d.m4s"]
        ));
    }

    #[test]
    fn no_audio_map_without_audio() {
        let mut p = plan(copy(CopyTag::Avc1, DvHandling::Keep), AudioAction::Copy);
        p.audio_index = None;
        let args = run(&p, &hw(Backend::Cpu, false), 0, 0.0);
        assert!(!args.contains(&"0:1".to_owned()));
    }

    #[test]
    fn audio_only_eac3() {
        let p = plan(
            copy(CopyTag::Hvc1, DvHandling::Keep),
            AudioAction::Transcode {
                codec: "eac3",
                channels: 6,
                bitrate: 640_000,
            },
        );
        let args = run(&p, &hw(Backend::Cpu, false), 0, 0.0);
        assert!(contains_seq(&args, &["-c:v", "copy"]));
        assert!(contains_seq(
            &args,
            &["-c:a", "eac3", "-b:a", "640000", "-ac", "6"]
        ));
        assert!(!args.contains(&"-af".to_owned()));
    }

    #[test]
    fn downmix_51_side() {
        assert_eq!(
            downmix_filter("5.1(side)").unwrap(),
            "pan=stereo|FL<1.0*FC+0.707*FL+0.707*SL+0.5*LFE|FR<1.0*FC+0.707*FR+0.707*SR+0.5*LFE"
        );
        assert_eq!(
            downmix_filter("5.1").unwrap(),
            "pan=stereo|FL<1.0*FC+0.707*FL+0.707*BL+0.5*LFE|FR<1.0*FC+0.707*FR+0.707*BR+0.5*LFE"
        );
        assert_eq!(
            downmix_filter("7.1").unwrap(),
            "pan=stereo|FL<1.0*FC+0.707*FL+0.707*SL+0.707*BL+0.5*LFE|FR<1.0*FC+0.707*FR+0.707*SR+0.707*BR+0.5*LFE"
        );
        assert_eq!(downmix_filter("quad"), None);
        let p = plan(
            copy(CopyTag::Hvc1, DvHandling::Keep),
            AudioAction::Transcode {
                codec: "aac",
                channels: 2,
                bitrate: 192_000,
            },
        );
        let args = run(&p, &hw(Backend::Cpu, false), 0, 0.0);
        assert_eq!(value_of(&args, "-af"), downmix_filter("5.1(side)").unwrap());
        assert!(contains_seq(&args, &["-ac", "2"]));
    }

    #[test]
    fn vaapi_hevc_libplacebo_tonemap() {
        let p = plan(
            transcode(VideoCodec::Hevc, HdrOut::Tonemap, false),
            AudioAction::Copy,
        );
        let args = run(&p, &hw(Backend::Vaapi, true), 0, 0.0);
        assert!(contains_seq(
            &args,
            &[
                "-hwaccel",
                "vaapi",
                "-hwaccel_device",
                "/dev/dri/renderD128",
                "-hwaccel_output_format",
                "vaapi",
                "-ss"
            ]
        ));
        assert_eq!(value_of(&args, "-vf"), PLACEBO_SDR);
        assert!(contains_seq(
            &args,
            &["-c:v", "hevc_vaapi", "-b:v", "8000000"]
        ));
        assert!(contains_seq(
            &args,
            &["-maxrate", "8000000", "-bufsize", "16000000"]
        ));
        assert!(contains_seq(
            &args,
            &[
                "-g",
                "100000",
                "-force_key_frames",
                "expr:gte(t,n_forced*6)"
            ]
        ));
        assert!(contains_seq(&args, &["-colorspace", "bt709"]));
    }

    #[test]
    fn vaapi_hevc_keep_hdr10() {
        let p = plan(
            transcode(VideoCodec::Hevc, HdrOut::Keep(Hdr::Hdr10), false),
            AudioAction::Copy,
        );
        let args = run(&p, &hw(Backend::Vaapi, true), 0, 0.0);
        let vf = value_of(&args, "-vf");
        assert!(vf.contains(
            "format=p010:colorspace=bt2020nc:color_primaries=bt2020:color_trc=smpte2084"
        ));
        assert!(contains_seq(&args, &["-profile:v", "main10"]));
        assert!(contains_seq(&args, &["-color_trc", "smpte2084"]));
    }

    #[test]
    fn cpu_h264_tonemap_no_libplacebo() {
        let p = plan(
            transcode(VideoCodec::H264, HdrOut::Tonemap, false),
            AudioAction::Copy,
        );
        let args = run(&p, &hw(Backend::Cpu, true), 0, 0.0);
        assert!(!args.contains(&"-hwaccel".to_owned()));
        let vf = value_of(&args, "-vf");
        assert_eq!(
            vf,
            "format=p010le,zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,tonemap=tonemap=hable:desat=0,zscale=t=bt709:m=bt709:r=tv,format=nv12,scale=1920:1080"
        );
        assert!(contains_seq(&args, &["-c:v", "libx264"]));
        assert!(contains_seq(&args, &["-x264-params", "scenecut=0"]));
    }

    #[test]
    fn vaapi_tonemap_without_libplacebo_downloads_and_uploads() {
        let p = plan(
            transcode(VideoCodec::H264, HdrOut::Tonemap, false),
            AudioAction::Copy,
        );
        let args = run(&p, &hw(Backend::Vaapi, false), 0, 0.0);
        let vf = value_of(&args, "-vf");
        assert!(vf.starts_with("hwdownload,format=p010le,zscale="));
        assert!(vf.ends_with("scale=1920:1080,format=nv12,hwupload"));
    }

    #[test]
    fn nvenc_av1() {
        let p = plan(
            transcode(VideoCodec::Av1, HdrOut::Tonemap, false),
            AudioAction::Copy,
        );
        let args = run(&p, &hw(Backend::Nvenc, true), 0, 0.0);
        assert!(contains_seq(&args, &["-hwaccel", "cuda"]));
        assert!(value_of(&args, "-vf").contains("derive_device=cuda"));
        assert!(contains_seq(&args, &["-c:v", "av1_nvenc"]));
    }

    #[test]
    fn videotoolbox_hevc() {
        let p = plan(
            transcode(VideoCodec::Hevc, HdrOut::Tonemap, false),
            AudioAction::Copy,
        );
        let args = run(&p, &hw(Backend::Videotoolbox, true), 0, 0.0);
        assert!(contains_seq(&args, &["-hwaccel", "videotoolbox", "-ss"]));
        let vf = value_of(&args, "-vf");
        assert!(!vf.contains("hwmap"));
        assert!(!vf.contains("hwdownload"));
        assert!(vf.contains("zscale"));
        assert!(contains_seq(&args, &["-c:v", "hevc_videotoolbox"]));
    }

    #[test]
    fn amf_drops_output_format_without_gpu_filter() {
        let p = plan(
            transcode(VideoCodec::H264, HdrOut::Keep(Hdr::Sdr), false),
            AudioAction::Copy,
        );
        let args = run(&p, &hw(Backend::Amf, false), 0, 0.0);
        assert!(contains_seq(&args, &["-hwaccel", "d3d11va"]));
        assert!(!args.contains(&"-hwaccel_output_format".to_owned()));
        assert_eq!(value_of(&args, "-vf"), "scale=1920:1080");
    }

    #[test]
    fn sdr_transcode_scales_on_gpu() {
        let p = plan(
            transcode(VideoCodec::H264, HdrOut::Keep(Hdr::Sdr), false),
            AudioAction::Copy,
        );
        let args = run(&p, &hw(Backend::Vaapi, true), 0, 0.0);
        assert_eq!(value_of(&args, "-vf"), "scale_vaapi=w=1920:h=1080");
    }

    #[test]
    fn bwdif_on_interlaced() {
        let p = plan(
            transcode(VideoCodec::H264, HdrOut::Keep(Hdr::Sdr), true),
            AudioAction::Copy,
        );
        let cpu = run(&p, &hw(Backend::Cpu, false), 0, 0.0);
        assert!(value_of(&cpu, "-vf").starts_with("bwdif=mode=send_frame,"));
        let gpu = run(&p, &hw(Backend::Vaapi, false), 0, 0.0);
        assert!(
            value_of(&gpu, "-vf").starts_with("hwdownload,format=p010le,bwdif=mode=send_frame,")
        );
        assert!(value_of(&gpu, "-vf").ends_with("format=nv12,hwupload"));
    }

    #[test]
    fn burn_in_software_overlay() {
        let mut p = plan(
            transcode(VideoCodec::H264, HdrOut::Keep(Hdr::Sdr), false),
            AudioAction::Copy,
        );
        p.burn_in_subtitle = Some(3);
        let args = run(&p, &hw(Backend::Vaapi, true), 0, 0.0);
        assert!(!args.contains(&"-hwaccel".to_owned()));
        assert!(!args.contains(&"-vf".to_owned()));
        assert_eq!(
            value_of(&args, "-filter_complex"),
            "[0:0][0:3]overlay,scale=1920:1080,format=nv12,hwupload[v]"
        );
        assert!(contains_seq(&args, &["-map", "[v]", "-map", "0:1"]));
        assert!(contains_seq(
            &args,
            &[
                "-init_hw_device",
                "vaapi=hw:/dev/dri/renderD128",
                "-filter_hw_device",
                "hw"
            ]
        ));
    }

    #[test]
    fn dv_strip_and_p7_to_p81() {
        let strip = plan(copy(CopyTag::Hvc1, DvHandling::Strip), AudioAction::Copy);
        let args = run(&strip, &hw(Backend::Cpu, false), 0, 0.0);
        assert!(contains_seq(&args, &["-bsf:v", "dovi_rpu=strip=1"]));
        let p7 = plan(copy(CopyTag::Dvh1, DvHandling::P7ToP81), AudioAction::Copy);
        let args = run(&p7, &hw(Backend::Cpu, false), 0, 0.0);
        assert!(contains_seq(&args, &["-tag:v", "dvh1"]));
        assert!(contains_seq(
            &args,
            &[
                "-bsf:v",
                "filter_units=remove_types=63,dovi_rpu",
                "-strict",
                "unofficial"
            ]
        ));
    }
}
