// node --import ./dev/controller-check/register.mjs dev/controller-check/check.mjs
import { strictEqual as eq } from "node:assert";

const { Playback } = await import("../../src/lib/core/playback/playback.svelte.ts");
const { prefs } = await import("../../src/lib/core/prefs.svelte.ts");

const flush = async () => {
	for (let i = 0; i < 30; i++) await new Promise((r) => setImmediate(r));
};
const session = {
	session_id: "s1",
	mode: "direct",
	url: "http://e/x",
	bandwidth: 8e6,
	video: { action: "copy", codec: "h264", width: 1920, height: 1080, hdr: "sdr" },
	audio: { action: "copy", codec: "aac", channels: 2 },
	video_reasons: [],
	audio_reasons: [],
	hw: "",
};
const tracks = (markers) => ({
	preparing: false,
	container: "mkv",
	video: null,
	chapters: [],
	markers,
	audio: [{ index: 0, codec: "aac", channels: 2, language: "eng", default: true }],
	subtitles: [],
	fonts: [],
});

// Back while media_tracks is in flight: when it lands, the unmounted player must not auto-play or auto-skip.
async function backDuringTracks(item, markers) {
	let release;
	const pending = new Promise((r) => {
		release = r;
	});
	const log = [];
	const port = {
		playerName: "mpv",
		command: async (cmd) => {
			log.push(cmd);
			return cmd === "stats" ? {} : null;
		},
		on: async () => () => {},
		capabilities: async () => ({}),
		decisions: async () => ({ bit_rate: 8e6, decisions: [] }),
		network: async () => "home",
		serverHost: async () => "host",
		startSession: async () => session,
		patchSession: async () => session,
		sessionInfo: async () => ({ speed: null }),
		stopSession: async () => {},
		heartbeat: async () => {},
		mediaTracks: () => pending,
		subtitleText: async () => "",
	};
	const pb = new Playback(port, { mediaId: "m", titleName: "T", anime: false, ...item });
	const end = pb.begin();
	await flush();
	await pb.exit();
	end();
	const before = log.length;
	release(tracks(markers));
	await flush();
	return log.slice(before);
}

let ended = 0;
await backDuringTracks({ start: 150, onEnd: () => ended++ }, { intro: null, credits: [100, 1400] });
eq(ended, 0, "autoplay fired after the viewer left");

prefs.autoSkip = true;
eq(
	(await backDuringTracks({ start: 5 }, { intro: [0, 90], credits: null })).includes("seek"),
	false,
	"intro auto-skip seeked a stopped player",
);
process.exit(0);
