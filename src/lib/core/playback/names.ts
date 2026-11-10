import type { AudioTrack, Session, SubKind, SubTrack, Tracks } from "#lib/core/playback/port.ts";
import { m as i18n } from "#lib/paraglide/messages.js";
import { getLocale } from "#lib/paraglide/runtime.js";

export const CODEC_NAMES: Record<string, string> = {
	h264: "H.264",
	hevc: "HEVC",
	av1: "AV1",
	vp9: "VP9",
	aac: "AAC",
	ac3: "AC3",
	eac3: "EAC3",
	dts: "DTS",
	truehd: "TrueHD",
	flac: "FLAC",
	opus: "Opus",
	mp3: "MP3",
	pcm_s16le: "PCM",
	pcm_s24le: "PCM",
};
const MODE_LABEL: Record<Session["mode"], () => string> = {
	direct: i18n.player_mode_direct,
	remux: i18n.player_mode_remux,
	audio_transcode: i18n.player_mode_audio_transcode,
	transcode: i18n.player_mode_transcode,
};
const HDR_NAMES: Record<string, string> = { hdr10: "HDR10", hlg: "HLG" };

export const modeName = (mode: Session["mode"]) => MODE_LABEL[mode]();

export const digits = (n: number, d: number) =>
	new Intl.NumberFormat(getLocale(), {
		minimumFractionDigits: d,
		maximumFractionDigits: d,
		useGrouping: false,
	}).format(n);

export const clock = (s: number) => {
	const t = Math.max(0, Math.floor(s));
	const m = Math.floor(t / 60);
	const ss = String(t % 60).padStart(2, "0");
	return m >= 60 ? `${Math.floor(m / 60)}:${String(m % 60).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
};

export function languageLabel(code: string) {
	try {
		const name = new Intl.DisplayNames([getLocale()], { type: "language" }).of(code) ?? code;
		return name.charAt(0).toLocaleUpperCase(getLocale()) + name.slice(1);
	} catch {
		return code || i18n.player_unknown_language();
	}
}

export function subInfo(t: SubTrack): {
	kind: SubKind;
	label: string;
	fmt: string;
	burnOnly?: boolean;
} {
	if (t.codec === "ass" || t.codec === "ssa")
		return { kind: "styled", label: i18n.player_sub_styled(), fmt: "ASS" };
	if (t.codec === "hdmv_pgs_subtitle")
		return { kind: "image", label: i18n.player_sub_image({ format: "PGS" }), fmt: "PGS" };
	if (t.codec === "dvd_subtitle")
		return {
			kind: "image",
			label: i18n.player_sub_image({ format: "VobSub" }),
			fmt: "VobSub",
			burnOnly: true,
		};
	return {
		kind: "text",
		label: i18n.player_sub_text({ format: t.codec === "webvtt" ? "WebVTT" : "SRT" }),
		fmt: "SRT",
	};
}

export const subName = (t: SubTrack) =>
	t.forced
		? i18n.player_sub_forced({ language: languageLabel(t.language) })
		: languageLabel(t.language);

export const channelLayout = (n: number) => (n >= 3 ? `${n - 1}.1` : `${n}.0`);
export const audioName = (t: AudioTrack) =>
	`${CODEC_NAMES[t.codec] ?? t.codec} ${channelLayout(t.channels)}`;
export const audioOutName = (s: { audio: Session["audio"] }) =>
	`${CODEC_NAMES[s.audio.codec] ?? s.audio.codec} ${channelLayout(s.audio.channels)}`;

// Wide films are cropped (3840×2076, 1920×800): the width names the class, as on the box.
export const resName = (w: number, h: number) =>
	`${w >= 7000 || h >= 4000 ? 4320 : w >= 3400 || h >= 2000 ? 2160 : w >= 1800 || h >= 1000 ? 1080 : w >= 1200 || h >= 700 ? 720 : h}p`;
export const codecName = (codec: string, profile?: string) =>
	`${CODEC_NAMES[codec] ?? codec.toUpperCase()}${profile ? ` ${profile.replace(/\s/g, "")}` : ""}`;
export const videoName = (codec: string, w: number, h: number, profile?: string) =>
	`${codecName(codec, profile)} ${resName(w, h)}`;
export const rangeName = (hdr?: string, dv?: boolean) =>
	dv ? " Dolby Vision" : hdr && HDR_NAMES[hdr] ? ` ${HDR_NAMES[hdr]}` : "";
export const hdrName = (hdr: string) => HDR_NAMES[hdr];

export const originalName = (video: Tracks["video"]) =>
	video
		? `${resName(video.width, video.height)} ${codecName(video.codec, video.profile)}${rangeName(video.hdr, !!video.dv)}`
		: "";

export const fileMbit = (bitRate: number) =>
	i18n.player_mbit({ value: digits(bitRate / 1_000_000, 1) });
