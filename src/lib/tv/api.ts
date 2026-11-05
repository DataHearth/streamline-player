import { convertFileSrc, invoke } from "@tauri-apps/api/core";

export type Kind = "movies" | "series";
export type FileSummary = { resolution?: string; hdr?: string; audio?: string; channels?: string };
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
	const res = q.resolution === "2160p" ? "4K" : q.resolution === "1080p" ? "HD" : q.resolution;
	const hdr = q.hdr === "Dolby Vision" ? "Dolby Vision" : q.hdr ? "HDR" : null;
	return [res, hdr, q.audio && /Atmos/.test(q.audio) ? "Dolby Atmos" : q.channels].filter((b): b is string => !!b);
}

export const meta = (kind: Kind, t: Title) =>
	kind === "series"
		? `${t.first_aired ?? t.year} · ${t.total_seasons} season${t.total_seasons === 1 ? "" : "s"} · ★ ${t.rating}`
		: `${t.year} · ${runtime(t.runtime ?? 0)} · ★ ${t.rating}`;

/** Episodes in play order, each with its season number. */
export const episodesOf = (t: Title) => (t.seasons ?? []).flatMap((s) => s.episodes.map((e) => ({ ...e, season: s.number })));

const STILL_POS = ["50% 45%", "22% 40%", "78% 55%", "35% 30%", "64% 70%", "12% 60%", "88% 35%"];
export const stillPos = (n: number) => STILL_POS[n % STILL_POS.length] ?? "50% 50%";
