// node --import ./dev/controller-check/register.mjs dev/now-playing-check.ts
import { strictEqual as eq, deepStrictEqual as same } from "node:assert";

const { nowPlaying, play, stop, setVolume, toggleMute, toMini, toFull } = await import(
	"../src/lib/core/playback/now-playing.svelte.ts"
);

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
const tracks = {
	preparing: false,
	container: "mkv",
	video: null,
	chapters: [],
	markers: { intro: null, credits: null },
	audio: [],
	subtitles: [],
	fonts: [],
};

let log: string[] = [];
let commands: { cmd: string; args?: Record<string, unknown> }[] = [];
let sessions = 0;
const port = {
	playerName: "mpv",
	command: async (cmd: string, args?: Record<string, unknown>) => {
		commands.push({ cmd, args });
		log.push(cmd);
		return cmd === "stats" ? {} : null;
	},
	on: async () => () => {},
	capabilities: async () => ({}),
	decisions: async () => ({ bit_rate: 8e6, decisions: [] }),
	network: async () => "home" as const,
	serverHost: async () => "host",
	startSession: async () => {
		log.push("startSession");
		return { ...session, session_id: `s${++sessions}` };
	},
	patchSession: async () => session,
	sessionInfo: async () => ({ speed: null }),
	stopSession: async (id: string) => {
		log.push(`stopSession:${id}`);
	},
	heartbeat: async () => {},
	mediaTracks: async () => tracks,
	subtitleText: async () => "",
};
const view = {
	kind: "movies" as const,
	id: 1,
	heading: "H",
	eyebrow: "E",
	still: "",
	synopsis: "",
	more: [],
};
const launch = (key: string) => ({
	key,
	view,
	item: { mediaId: key, titleName: key, anime: false, start: 0 },
});
const count = (name: string) => log.filter((l) => l === name).length;
const reset = () => {
	log = [];
	commands = [];
};

play(port, launch("a"), {});
await flush();
play(port, launch("a"), {});
await flush();
eq(count("startSession"), 1, "same key started a second session");
eq(count("load"), 1, "same key loaded twice");

reset();
play(port, launch("b"), {});
await flush();
eq(log.includes("stopSession:s1"), true, "the first session was not stopped");
eq(log.indexOf("stopSession:s1") < log.indexOf("startSession"), true, "stop came after start");

reset();
setVolume(0.5);
same(commands.at(-1), { cmd: "volume", args: { volume: 0.5, muted: false } });
reset();
play(port, launch("c"), {});
await flush();
same(commands.find((c) => c.cmd === "volume")?.args, { volume: 0.5, muted: false });
eq(log.indexOf("volume") < log.indexOf("play"), true, "volume was not sent before play");
eq(JSON.parse(localStorage.getItem("player-volume") ?? "{}").volume, 0.5);

toggleMute();
eq(nowPlaying.muted, true);
toggleMute();
eq(nowPlaying.muted, false);
setVolume(1.4);
eq(nowPlaying.volume, 1);
setVolume(-1);
eq(nowPlaying.volume, 0);

reset();
toMini();
eq(nowPlaying.mini, true);
toFull();
eq(nowPlaying.mini, false);
eq(
	log.some((l) => l.startsWith("stopSession") || l === "startSession"),
	false,
	"mini/full touched the session",
);

reset();
stop();
await flush();
eq(count("stop") >= 1, true, "stop was not sent");
eq(log.filter((l) => l.startsWith("stopSession")).length, 1, "session not stopped once");
eq(nowPlaying.pb, null);
console.log("now-playing-check: ok");
process.exit(0);
