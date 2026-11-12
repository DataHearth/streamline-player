import { type PlayItem, Playback, type PlaybackUi } from "#lib/core/playback/playback.svelte.ts";
import type { PlaybackPort } from "#lib/core/playback/port.ts";
import type { Kind } from "#lib/core/types.ts";

export type PlayView = {
	kind: Kind;
	id: number;
	season?: number;
	episode?: number;
	heading: string;
	eyebrow: string;
	still: string;
	synopsis: string;
	next?: {
		code: string;
		title: string;
		overview: string;
		still: string;
		stillPos: string;
		label: string;
		href: string;
	};
	more: { title: string; poster: string; href: string }[];
};
export type Launch = { key: string; item: PlayItem; view: PlayView };

const STORAGE_KEY = "player-volume";
const DEFAULT_VOLUME = { volume: 0.8, muted: false };

function remembered() {
	try {
		const v = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null");
		if (typeof v?.volume === "number" && typeof v?.muted === "boolean")
			return { volume: Math.min(1, Math.max(0, v.volume)), muted: v.muted };
	} catch {
		// Unreadable storage falls back to the design's default.
	}
	return { ...DEFAULT_VOLUME };
}

function remember(volume: number, muted: boolean) {
	try {
		localStorage.setItem(STORAGE_KEY, JSON.stringify({ volume, muted }));
	} catch {
		// Private windows refuse storage; the volume then lasts until the page closes.
	}
}

export const nowPlaying = $state<{
	launch: Launch | null;
	pb: Playback | null;
	volume: number;
	muted: boolean;
	mini: boolean;
}>({ launch: null, pb: null, ...remembered(), mini: false });

let port: PlaybackPort | null = null;
let end: (() => void) | null = null;
let offVolume: Promise<() => void> | null = null;
let generation = 0;

const sendVolume = () =>
	port?.command("volume", { volume: nowPlaying.volume, muted: nowPlaying.muted }).catch(() => {});

async function release() {
	const pb = nowPlaying.pb;
	const teardown = end;
	const off = offVolume;
	nowPlaying.pb = null;
	nowPlaying.launch = null;
	nowPlaying.mini = false;
	end = null;
	offVolume = null;
	off?.then((f) => f());
	if (!pb) return;
	await pb.exit();
	teardown?.();
}

export function play(p: PlaybackPort, launch: Launch, ui: PlaybackUi): Playback {
	if (nowPlaying.pb && nowPlaying.launch?.key === launch.key) return nowPlaying.pb;
	const id = ++generation;
	const leaving = release();
	port = p;
	const pb = new Playback(p, launch.item, ui);
	nowPlaying.pb = pb;
	nowPlaying.launch = launch;
	// The old session must be gone on the server before the new one asks for a slot.
	leaving.then(() => {
		if (id !== generation) return;
		offVolume = p
			.on<{ volume: number; muted: boolean }>("volume", (e) => {
				nowPlaying.volume = e.volume;
				nowPlaying.muted = e.muted;
				remember(e.volume, e.muted);
			})
			.catch(() => () => {});
		sendVolume();
		end = pb.begin();
	});
	return pb;
}

export function stop(): void {
	generation++;
	port = null;
	release();
}

export function setVolume(v: number): void {
	nowPlaying.volume = Math.min(1, Math.max(0, v));
	remember(nowPlaying.volume, nowPlaying.muted);
	sendVolume();
}

export function toggleMute(): void {
	nowPlaying.muted = !nowPlaying.muted;
	remember(nowPlaying.volume, nowPlaying.muted);
	sendVolume();
}

export const toMini = () => {
	nowPlaying.mini = true;
};
export const toFull = () => {
	nowPlaying.mini = false;
};
