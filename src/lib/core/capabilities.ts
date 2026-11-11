export type Caps = {
	video: { codec: string }[];
	hdr: { formats: string[] };
	audio: { codecs: string[]; passthrough: string[] };
	subtitles?: string[];
};
export type Chip = { group: "video" | "audio" | "subs"; name: string; ok: boolean };

/** What a player reports, as the chips Settings lists. Subtitles only when the player reports them. */
export function chips(c: Caps): Chip[] {
	const video = new Set(c.video.map((x) => x.codec));
	const audio = new Set([...c.audio.codecs, ...c.audio.passthrough]);
	const hdr = new Set(c.hdr.formats);
	const subs = c.subtitles ? new Set(c.subtitles) : null;
	return [
		{ group: "video", name: "H.264", ok: video.has("h264") },
		{ group: "video", name: "HEVC", ok: video.has("hevc") },
		{ group: "video", name: "AV1", ok: video.has("av1") },
		{ group: "video", name: "VP9", ok: video.has("vp9") },
		{ group: "video", name: "HDR10", ok: hdr.has("hdr10") },
		{ group: "video", name: "Dolby Vision", ok: hdr.has("dolby_vision") },
		{ group: "audio", name: "AAC", ok: audio.has("aac") },
		{ group: "audio", name: "AC3", ok: audio.has("ac3") },
		{ group: "audio", name: "EAC3", ok: audio.has("eac3") },
		{ group: "audio", name: "TrueHD", ok: audio.has("truehd") },
		{ group: "audio", name: "DTS", ok: audio.has("dts") },
		{ group: "audio", name: "FLAC", ok: audio.has("flac") },
		...(subs
			? [
					{ group: "subs" as const, name: "ASS", ok: subs.has("ass") },
					{ group: "subs" as const, name: "SRT", ok: subs.has("srt") },
					{ group: "subs" as const, name: "PGS", ok: subs.has("pgs") },
					{ group: "subs" as const, name: "VobSub", ok: subs.has("vobsub") },
				]
			: []),
	];
}

const VIDEO: Record<string, string> = {
	h264: 'video/mp4; codecs="avc1.640028"',
	hevc: 'video/mp4; codecs="hvc1.1.6.L120.90"',
	av1: 'video/mp4; codecs="av01.0.08M.08"',
	vp9: 'video/webm; codecs="vp09.00.40.08"',
};
const AUDIO: Record<string, string> = {
	aac: 'audio/mp4; codecs="mp4a.40.2"',
	ac3: 'audio/mp4; codecs="ac-3"',
	eac3: 'audio/mp4; codecs="ec-3"',
	dts: 'audio/mp4; codecs="dtsc"',
	flac: 'audio/mp4; codecs="flac"',
};

/** What the browser itself reports it can decode, asked the way the server asks the player: of the running webview. */
export function browserCaps(): Caps {
	const can = (type: string) =>
		typeof MediaSource !== "undefined" && MediaSource.isTypeSupported(type);
	const hdr = matchMedia("(dynamic-range: high)").matches;
	return {
		video: Object.keys(VIDEO)
			.filter((k) => can(VIDEO[k]!))
			.map((codec) => ({ codec })),
		hdr: {
			formats: [
				...(hdr ? ["hdr10"] : []),
				...(can('video/mp4; codecs="dvh1.05.06"') ? ["dolby_vision"] : []),
			],
		},
		audio: {
			codecs: Object.keys(AUDIO).filter((k) => can(AUDIO[k]!)),
			passthrough: [],
		},
	};
}
