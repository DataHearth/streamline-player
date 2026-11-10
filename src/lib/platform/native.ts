import type { PlaybackPort, Session, Tracks } from "#lib/core/playback/port.ts";
import { decodeHere } from "#lib/core/prefs.svelte.ts";
import type {
	AuthMethods,
	AuthState,
	Decision,
	DecisionOption,
	Device,
	DeviceInfo,
	HomeFeed,
	Kind,
	Me,
	Pairing,
	Person,
	Title,
	Watch,
} from "#lib/core/types.ts";
import { addPluginListener, convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

const webOnly = () => Promise.reject(new Error("not on this target"));

export const init = () => Promise.resolve();
export const authMethods = (): Promise<AuthMethods> => webOnly();
export const startSso = (_provider: string, _next: string): Promise<void> => webOnly();
export const finishSso = (_query: URLSearchParams): Promise<string> => webOnly();
export const signInEmail = (_email: string, _password: string): Promise<void> => webOnly();

export const windowControls = {
	minimize: () => getCurrentWindow().minimize(),
	toggleMaximize: () => getCurrentWindow().toggleMaximize(),
	close: () => getCurrentWindow().close(),
};

let capabilities: { decode: string; value: Promise<unknown> } | null = null;
// ponytail: mpv is the only desktop backend so far (Linux); macOS and Windows will need their own name.
export const PLAYER_NAME: string = /Android/.test(navigator.userAgent) ? "ExoPlayer" : "mpv";
/** What this device plays, as the server needs it; it changes with the pass-through settings. */
export function playerCapabilities() {
	const decode = decodeHere();
	if (capabilities?.decode !== decode.join())
		capabilities = {
			decode: decode.join(),
			value: invoke("plugin:player|capabilities", { decode }),
		};
	return capabilities.value;
}

/** What the engine would do for each option, without starting anything. */
export const decisions = async (mediaId: string, options: DecisionOption[]) =>
	invoke<{ bit_rate: number; decisions: Decision[] }>("decisions", {
		mediaId,
		capabilities: await playerCapabilities(),
		options,
	});

/** Where Streamline places this device right now; anything but a clear "home" is away. */
export const network = () =>
	playbackStatus()
		.then((st) => (st.network === "home" ? "home" : "away") as "home" | "away")
		.catch(() => "away" as const);

const listOf = <T>(page: { items: T[] }) => page.items;

export const library = (kind: Kind) => invoke<{ items: Title[] }>("library", { kind }).then(listOf);
export const title = (kind: Kind, id: number) => invoke<Title>("title", { kind, id });
export const person = (id: string) => invoke<Person>("person", { id });
export const watchState = () => invoke<{ items: Watch[] }>("watch_state").then(listOf);
export const setWatched = (mediaId: string, watched: boolean) =>
	invoke("set_watched", { mediaId, watched });

// `convertFileSrc` would percent-encode the slashes of a whole path.
const ARTWORK = convertFileSrc("", "artwork");
export const artwork = (kind: Kind, id: number, name: "poster" | "backdrop") =>
	`${ARTWORK}${kind}/${id}/${name}.jpg`;
export const myPicture = (version: number) => `${ARTWORK}users/me/photo.jpg?v=${version}`;
export const headshot = (person: string) => `${ARTWORK}people/${person}/photo.jpg`;

/** Movies and shows together; ids are only unique within a kind. */
export const everything = async () => {
	const [movies, shows] = await Promise.all([library("movies"), library("series")]);
	return [
		...movies.map((t) => ({ ...t, kind: "movies" as Kind })),
		...shows.map((t) => ({ ...t, kind: "series" as Kind })),
	];
};

export const authState = () => invoke<AuthState>("auth_state");
export const connect = (url: string) => invoke("connect", { url });
export const pairStart = () => invoke<Pairing>("pair_start");
export const pairPoll = () => invoke<string>("pair_poll");
export const signOut = () => invoke("sign_out");
export const me = () => invoke<Me>("me");
export const home = () => invoke<HomeFeed>("home");
export const updateMe = (patch: { name?: string; email?: string }) =>
	invoke<Me>("update_me", patch);
export const setPicture = async (file: Blob) =>
	invoke<void>("set_picture", new Uint8Array(await file.arrayBuffer()), {
		headers: { "x-picture-type": file.type },
	});
export const removePicture = () => invoke<void>("remove_picture");
export const revokeDevice = (id: string) => invoke<void>("revoke_device", { id });
export const devices = () => invoke<{ items: Device[] }>("devices").then(listOf);
export const playbackStatus = () => invoke<{ network?: string }>("playback_status");
export const serverHost = () => invoke<string>("server_host");
export const deviceInfo = () => invoke<DeviceInfo>("plugin:player|deviceInfo");

/** Rust drops a revoked key on the first 401 and says so with this event. */
export const onSignedOut = (cb: () => void): Promise<() => void> => listen("signed-out", cb);

/** What the TV's microphone heard, in `language`; empty text when nothing was recognized. */
export const voice = (language: string) =>
	invoke<{ text: string }>("plugin:player|voice", { language });

export const onPlayer = <T = undefined>(event: string, cb: (e: T) => void) =>
	addPluginListener<T>("player", event, cb).then((h) => () => h.unregister());

export const playbackPort: PlaybackPort = {
	playerName: PLAYER_NAME,
	command: (cmd, args) => invoke(`plugin:player|${cmd}`, args),
	on: onPlayer,
	capabilities: playerCapabilities,
	decisions,
	network,
	serverHost,
	startSession: (mediaId, capabilities, position, tracks) =>
		invoke<Session>("start_session", { mediaId, capabilities, position, tracks }),
	patchSession: (id, changes) => invoke<Session>("patch_session", { id, changes }),
	sessionInfo: (id) => invoke<{ speed: number | null }>("session_info", { id }),
	stopSession: (id) => invoke("stop_session", { id }),
	heartbeat: (id, position, paused) => invoke("heartbeat", { id, position, paused }),
	mediaTracks: (mediaId) => invoke<Tracks>("media_tracks", { mediaId }),
	subtitleText: (url) => invoke<string>("subtitle_text", { url }),
};
