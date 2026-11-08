import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { decodeHere, prefs } from "./prefs.svelte";

export type Kind = "movies" | "series";
export type FileSummary = { resolution?: string; codec?: string; hdr?: string; audio?: string; channels?: string };
export type Episode = { id: number; number: number; title: string; overview: string; runtime: number; has_file: boolean; media_file_id: string };
export type Season = { number: number; name: string; episodes: Episode[] };
export type Title = {
	id: number;
	title: string;
	year: number;
	overview: string;
	genres: string[];
	rating: number;
	added_at: string;
	file_summary?: FileSummary;
	runtime?: number;
	/** Age rating as printed on the box: "PG-13", "R", "12". */
	certification?: string;
	media_files?: { id: string }[];
	network?: string;
	total_seasons?: number;
	first_aired?: string;
	seasons?: Season[];
};
export type Watch = {
	media_id: string;
	movie_id?: number;
	series_id?: number;
	season?: number;
	episode_id?: number;
	position_seconds: number;
	duration_seconds: number;
	watched: boolean;
};

export type Reason = { code: string; text: string };
export type Mode = "direct" | "remux" | "audio_transcode" | "transcode";
export type VideoOut = { action: string; codec: string; width: number; height: number; hdr: string; dv_profile?: number };
export type AudioOut = { action: string; codec: string; channels: number };
export type Decision = { mode: Mode; video: VideoOut; audio: AudioOut; video_reasons: Reason[]; audio_reasons: Reason[]; needs_slot: boolean; bandwidth: number; error?: undefined } | { error: { code: string; message: string } };
export type DecisionOption = { audio_index?: number | null; burn_in_subtitle?: number | null; max_bitrate?: number | null };

let capabilities: { decode: string; value: Promise<unknown> } | null = null;
// ponytail: mpv is the only desktop backend so far (Linux); macOS and Windows will need their own name.
export const PLAYER_NAME = /Android/.test(navigator.userAgent) ? "ExoPlayer" : "mpv";
/** What this device plays, as the server needs it; it changes with the pass-through settings. */
export function playerCapabilities() {
	const decode = decodeHere();
	if (capabilities?.decode !== decode.join()) capabilities = { decode: decode.join(), value: invoke("plugin:player|capabilities", { decode }) };
	return capabilities.value;
}

/** What the engine would do for each option, without starting anything. */
export const decisions = async (mediaId: string, options: DecisionOption[]) =>
	invoke<{ bit_rate: number; decisions: Decision[] }>("decisions", { mediaId, capabilities: await playerCapabilities(), options });

/** Where Streamline places this device right now; anything but a clear "home" is away. */
export const network = () =>
	invoke<{ network?: string }>("playback_status").then((st) => (st.network === "home" ? "home" : "away") as "home" | "away").catch(() => "away" as const);

const listOf = <T>(page: { items: T[] }) => page.items;

export const library = (kind: Kind) => invoke<{ items: Title[] }>("library", { kind }).then(listOf);
export const title = (kind: Kind, id: number) => invoke<Title>("title", { kind, id });
export const watchState = () => invoke<{ items: Watch[] }>("watch_state").then(listOf);
export const setWatched = (mediaId: string, watched: boolean) => invoke("set_watched", { mediaId, watched });

// `convertFileSrc` would percent-encode the slashes of a whole path.
const ARTWORK = convertFileSrc("", "artwork");
export const artwork = (kind: Kind, id: number, name: "poster" | "backdrop") => `${ARTWORK}${kind}/${id}/${name}.jpg`;

export const runtime = (min: number) => {
	const h = Math.floor(min / 60);
	const m = min % 60;
	return h ? `${h}h ${String(m).padStart(2, "0")}m` : `${m}m`;
};
export const left = (sec: number) => {
	const m = Math.max(1, Math.round(sec / 60));
	return m >= 60 ? `${Math.floor(m / 60)}h ${m % 60}m left` : `${m}m left`;
};
export const inProgress = (w?: Watch) => !!w && !w.watched && w.position_seconds > 0;
export const pct = (w?: Watch) => (w && w.duration_seconds > 0 ? Math.round((w.position_seconds / w.duration_seconds) * 100) : 0);

export function badges(q?: FileSummary) {
	if (!q) return [];
	if (prefs.techDetails) return [q.resolution, q.codec, q.hdr, q.audio && `${q.audio} ${q.channels ?? ""}`.trim()].filter((b): b is string => !!b);
	const res = q.resolution === "4320p" ? "8K" : q.resolution === "2160p" ? "4K" : q.resolution === "1080p" ? "HD" : q.resolution;
	const hdr = q.hdr === "Dolby Vision" ? "Dolby Vision" : q.hdr ? "HDR" : null;
	return [res, hdr, q.audio && /Atmos/.test(q.audio) ? "Dolby Atmos" : q.channels].filter((b): b is string => !!b);
}

export const meta = (kind: Kind, t: Title) =>
	kind === "series"
		? `${t.first_aired ? `${t.first_aired.slice(0, 4)}\u2013` : t.year} · ${t.total_seasons ?? t.seasons?.length ?? 0} season${(t.total_seasons ?? t.seasons?.length) === 1 ? "" : "s"} · ★ ${t.rating}`
		: [t.year, runtime(t.runtime ?? 0), t.certification, `★ ${t.rating}`].filter(Boolean).join(" · ");

/** Episodes in play order, each with its season number. */
export const episodesOf = (t: Title) => (t.seasons ?? []).flatMap((s) => s.episodes.map((e) => ({ ...e, season: s.number })));

const STILL_POS = ["50% 45%", "22% 40%", "78% 55%", "35% 30%", "64% 70%", "12% 60%", "88% 35%"];
export const stillPos = (episode: number, season: number) => STILL_POS[(episode + season) % STILL_POS.length] ?? "50% 50%";

/** Movies and shows together; ids are only unique within a kind. */
export const everything = async () => {
	const [movies, shows] = await Promise.all([library("movies"), library("series")]);
	return [...movies.map((t) => ({ ...t, kind: "movies" as Kind })), ...shows.map((t) => ({ ...t, kind: "series" as Kind }))];
};

const BLANK = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg'/%3E";
/** WebKit draws a broken-image glyph over the card; a blank image keeps the card's frame and background. */
export const hideBroken = (e: Event) => ((e.currentTarget as HTMLImageElement).src = BLANK);

/** Titles without a backdrop fall back to their poster; without either, the image hides. */
export const orPoster = (poster: string, position?: string) => (e: Event) => {
	const img = e.currentTarget as HTMLImageElement;
	if (img.src === poster) return hideBroken(e);
	img.src = poster;
	img.style.objectPosition = position ?? "";
};
