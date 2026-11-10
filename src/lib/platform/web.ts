import { resolve } from "$app/paths";
import { browserDeviceName, pkcePair, SignInError } from "#lib/core/auth.ts";
import type { PlaybackPort, Session, Tracks } from "#lib/core/playback/port.ts";
import type {
	AuthMethods,
	AuthState,
	Decision,
	DecisionOption,
	Device,
	DeviceInfo,
	Kind,
	Me,
	Pairing,
	Person,
	Title,
	Watch,
} from "#lib/core/types.ts";

const REFRESH = "auth:refresh";
const DEVICE = "auth:device";
const PENDING_SSO = "auth:sso";
const LOCK = "auth-refresh";
const NO_PLAYER = "no browser player before plan 4";

type Tokens = {
	access_token: string;
	refresh_token: string;
	device_id: string;
};

let streamline = location.origin;
let access: string | null = null;
const signedOutListeners = new Set<() => void>();

const store = {
	get: (key: string) => {
		try {
			return localStorage.getItem(key);
		} catch {
			return null;
		}
	},
	set: (key: string, value: string) => {
		try {
			localStorage.setItem(key, value);
		} catch {}
	},
	remove: (key: string) => {
		try {
			localStorage.removeItem(key);
		} catch {}
	},
};

const notOnWeb = () => Promise.reject(new Error("not on this target"));

export async function init() {
	try {
		const res = await fetch(`${resolve("/")}config.json`);
		const config = res.ok ? await res.json() : null;
		if (typeof config?.streamline === "string") {
			streamline = config.streamline.replace(/\/+$/, "");
			return;
		}
	} catch {}
	streamline = import.meta.env.DEV
		? ((import.meta.env.VITE_DEV_STREAMLINE as string | undefined) ?? location.origin)
		: location.origin;
}

const signedOut = () => {
	access = null;
	store.remove(REFRESH);
	store.remove(DEVICE);
	for (const cb of [...signedOutListeners]) cb();
};

window.addEventListener("storage", (e) => {
	if (e.key === REFRESH && e.newValue === null) {
		access = null;
		for (const cb of [...signedOutListeners]) cb();
	}
});

const inLock = <T>(job: () => Promise<T>): Promise<T> =>
	navigator.locks ? navigator.locks.request(LOCK, job) : job();

const url = (path: string) => `${streamline}/api/v1${path}`;

const fail = (text: string): never => {
	throw text;
};

async function tokenRequest(body: Record<string, string>): Promise<Response> {
	try {
		return await fetch(url("/auth/token"), {
			method: "POST",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify(body),
		});
	} catch {
		throw new SignInError("network");
	}
}

function keep(tokens: Tokens) {
	access = tokens.access_token;
	store.set(REFRESH, tokens.refresh_token);
	store.set(DEVICE, tokens.device_id);
}

/** `stale` is the access token the caller's request used: another call may have replaced it since. */
function refresh(stale: string | null): Promise<boolean> {
	return inLock(async () => {
		if (access !== stale) return true;
		const token = store.get(REFRESH);
		if (!token) return false;
		let res: Response;
		try {
			res = await tokenRequest({ grant_type: "refresh_token", refresh_token: token });
		} catch {
			return false;
		}
		if (res.ok) {
			keep(await res.json());
			return true;
		}
		if (res.status >= 400 && res.status < 500) signedOut();
		return false;
	});
}

async function call<T = unknown>(method: string, path: string, body?: unknown): Promise<T> {
	if (!access && store.get(REFRESH)) await refresh(null);
	const send = () => {
		const sent = access;
		return fetch(url(path), {
			method,
			headers: {
				...(sent ? { Authorization: `Bearer ${sent}` } : {}),
				...(body === undefined ? {} : { "Content-Type": "application/json" }),
			},
			body: body === undefined ? undefined : JSON.stringify(body),
		}).then((res) => ({ res, sent }));
	};
	let { res, sent } = await send().catch((e) => fail(String(e)));
	if (res.status === 401 && store.get(REFRESH)) {
		if (await refresh(sent)) ({ res, sent } = await send().catch((e) => fail(String(e))));
	}
	const text = await res.text();
	if (!res.ok) return fail(text);
	return (text ? JSON.parse(text) : null) as T;
}

const listOf = <T>(page: { items: T[] }) => page.items;

export const library = (kind: Kind) => call<{ items: Title[] }>("GET", `/${kind}`).then(listOf);
export const title = (kind: Kind, id: number) => call<Title>("GET", `/${kind}/${id}`);
export const person = (id: string) =>
	/^[A-Za-z0-9-]+$/.test(id)
		? call<Person>("GET", `/people/${id}`)
		: fail(JSON.stringify({ error: { code: "not_found" } }));
export const watchState = () => call<{ items: Watch[] }>("GET", "/watchstate").then(listOf);
export const setWatched = (mediaId: string, watched: boolean) =>
	call("PUT", `/watchstate/${mediaId}`, { watched });

export const artwork = (kind: Kind, id: number, name: "poster" | "backdrop") =>
	`${streamline}/api/v1/posters/${kind}/${id}/${name}.jpg`;
export const headshot = (person: string) =>
	`${streamline}/api/v1/posters/people/${person}/photo.jpg`;

export const everything = async () => {
	const [movies, shows] = await Promise.all([library("movies"), library("series")]);
	return [
		...movies.map((t) => ({ ...t, kind: "movies" as Kind })),
		...shows.map((t) => ({ ...t, kind: "series" as Kind })),
	];
};

export const authState = (): Promise<AuthState> =>
	Promise.resolve({
		paired: !!store.get(REFRESH),
		server: streamline,
		keystore: "browser storage",
		store_error: null,
	});
export const connect = (_url: string): Promise<unknown> => notOnWeb();
export const pairStart = (): Promise<Pairing> => notOnWeb();
export const pairPoll = (): Promise<string> => notOnWeb();
export const me = () => call<Me>("GET", "/me");
export const devices = () => call<{ items: Device[] }>("GET", "/devices").then(listOf);
export const playbackStatus = () => call<{ network?: string }>("GET", "/playback/status");
export const serverHost = () => Promise.resolve(streamline.replace(/^[a-z]+:\/\//, ""));

export async function signOut(): Promise<unknown> {
	if (store.get(REFRESH)) await call("DELETE", "/devices/current").catch(() => {});
	access = null;
	store.remove(REFRESH);
	store.remove(DEVICE);
	return undefined;
}

export const onSignedOut = (cb: () => void): Promise<() => void> => {
	signedOutListeners.add(cb);
	return Promise.resolve(() => void signedOutListeners.delete(cb));
};

export const deviceInfo = (): Promise<DeviceInfo> =>
	Promise.resolve({
		name: browserDeviceName(navigator.userAgent),
		model: "",
		platform: "Web browser",
		player: "Shaka",
		display: {
			name: "",
			width: screen.width,
			height: screen.height,
			refresh_hz: 0,
			hdr: [],
		},
	});

export const voice = (_language: string): Promise<{ text: string }> => notOnWeb();

export const PLAYER_NAME: string = "Shaka";
export const playerCapabilities = (): Promise<unknown> => Promise.reject(new Error(NO_PLAYER));

export const decisions = async (mediaId: string, options: DecisionOption[]) =>
	call<{ bit_rate: number; decisions: Decision[] }>("POST", "/playback/decisions", {
		media_id: mediaId,
		capabilities: await playerCapabilities(),
		options,
	});

export const network = () =>
	playbackStatus()
		.then((st) => (st.network === "home" ? "home" : "away") as "home" | "away")
		.catch(() => "away" as const);

export const onPlayer = <T = undefined>(
	_event: string,
	_cb: (e: T) => void,
): Promise<() => Promise<void>> => Promise.resolve(() => Promise.resolve());

export const playbackPort: PlaybackPort = {
	playerName: PLAYER_NAME,
	command: () => Promise.reject(new Error(NO_PLAYER)),
	on: () => Promise.resolve(() => {}),
	capabilities: playerCapabilities,
	decisions,
	network,
	serverHost,
	startSession: (mediaId, capabilities, position, tracks) =>
		call<Session>("POST", "/playback/sessions", {
			media_id: mediaId,
			capabilities,
			position,
			...tracks,
		}),
	patchSession: (id, changes) => call<Session>("PATCH", `/playback/sessions/${id}`, changes),
	sessionInfo: (id) => call<{ speed: number | null }>("GET", `/playback/sessions/${id}`),
	stopSession: (id) => call("DELETE", `/playback/sessions/${id}`),
	heartbeat: (id, position, paused) =>
		call("POST", `/playback/sessions/${id}/progress`, { position, paused }),
	mediaTracks: (mediaId) => call<Tracks>("GET", `/playback/media/${mediaId}/tracks`),
	subtitleText: (subtitleUrl) =>
		fetch(subtitleUrl).then((res) => (res.ok ? res.text() : fail(`${res.status}`))),
};

export const authMethods = (): Promise<AuthMethods> =>
	fetch(url("/auth/methods")).then((res) => res.json());

const callbackUrl = () => `${location.origin}${resolve("/")}signin/callback`;

export async function startSso(provider: string, next: string) {
	const { verifier, challenge } = await pkcePair();
	const state = crypto.randomUUID();
	sessionStorage.setItem(PENDING_SSO, JSON.stringify({ state, verifier, next }));
	const query = new URLSearchParams({
		provider,
		redirect_uri: callbackUrl(),
		state,
		code_challenge: challenge,
		code_challenge_method: "S256",
	});
	location.assign(url(`/auth/authorize?${query}`));
}

export async function finishSso(query: URLSearchParams): Promise<string> {
	const raw = sessionStorage.getItem(PENDING_SSO);
	sessionStorage.removeItem(PENDING_SSO);
	const pending = raw
		? (JSON.parse(raw) as { state: string; verifier: string; next: string })
		: null;
	if (!pending || query.get("state") !== pending.state) throw new SignInError("state");
	if (query.get("error") === "access_denied") throw new SignInError("denied");
	const code = query.get("code");
	if (!code) throw new SignInError("state");
	const res = await tokenRequest({
		grant_type: "authorization_code",
		code,
		code_verifier: pending.verifier,
		redirect_uri: callbackUrl(),
		device_name: browserDeviceName(navigator.userAgent),
	});
	if (!res.ok) throw new SignInError("invalid_grant");
	keep(await res.json());
	return pending.next;
}

export async function signInEmail(email: string, password: string) {
	const res = await tokenRequest({
		grant_type: "password",
		email,
		password,
		device_name: browserDeviceName(navigator.userAgent),
	});
	if (!res.ok) throw new SignInError("credentials");
	keep(await res.json());
}

export const windowControls = {
	minimize: () => Promise.resolve(),
	toggleMaximize: () => Promise.resolve(),
	close: () => Promise.resolve(),
};
