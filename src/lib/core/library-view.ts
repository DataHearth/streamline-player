import { watchedTitle } from "#lib/core/browse.ts";
import type { Kind, Title, Watch } from "#lib/core/types.ts";

export type Flag = "unwatched" | "progress" | "uhd" | "hdr";
export type Sort = "added" | "title" | "year" | "rating" | "size";
export type Layout = "grid" | "list";
export type View = { flags: Flag[]; genre: string | null; sort: Sort; layout: Layout };

export const FLAGS: Flag[] = ["unwatched", "progress", "uhd", "hdr"];
export const SORTS: Sort[] = ["added", "title", "year", "rating", "size"];
export const DEFAULT_VIEW: View = { flags: [], genre: null, sort: "added", layout: "grid" };

const EXCLUDES: Partial<Record<Flag, Flag>> = { unwatched: "progress", progress: "unwatched" };

export function toggleFlag(flags: Flag[], f: Flag): Flag[] {
	if (flags.includes(f)) return flags.filter((x) => x !== f);
	return [...flags.filter((x) => x !== EXCLUDES[f]), f];
}

/** Percent watched of the unfinished play, 0 when there is none. */
export function progressOf(t: Title, kind: Kind, watch: Watch[]): number {
	const row = watch.find(
		(w) =>
			(kind === "movies" ? w.movie_id === t.id : w.series_id === t.id) &&
			!w.watched &&
			w.position_seconds > 0,
	);
	return row && row.duration_seconds > 0
		? Math.round((row.position_seconds / row.duration_seconds) * 100)
		: 0;
}

const matches: Record<Flag, (t: Title, kind: Kind, watch: Watch[]) => boolean> = {
	unwatched: (t, kind, watch) =>
		!watchedTitle(t, kind, watch) &&
		!watch.some((w) => isPlayOf(w, t, kind) && !w.watched && w.position_seconds > 0),
	progress: (t, kind, watch) =>
		watch.some((w) => isPlayOf(w, t, kind) && !w.watched && w.position_seconds > 0),
	uhd: (t) => t.file_summary?.resolution === "2160p",
	hdr: (t) => !!t.file_summary?.hdr,
};

const isPlayOf = (w: Watch, t: Title, kind: Kind) =>
	kind === "movies" ? w.movie_id === t.id : w.series_id === t.id;

const flagged = (titles: Title[], kind: Kind, watch: Watch[], flags: Flag[]) =>
	titles.filter((t) => flags.every((f) => matches[f](t, kind, watch)));

/** Every genre of the titles, counted among those that pass the flags; a genre with no title left is still listed, with 0. */
export function genres(titles: Title[], kind: Kind, watch: Watch[], flags: Flag[]) {
	const counts = new Map<string, number>();
	for (const t of titles) for (const g of t.genres) counts.set(g, counts.get(g) ?? 0);
	for (const t of flagged(titles, kind, watch, flags))
		for (const g of t.genres) counts.set(g, (counts.get(g) ?? 0) + 1);
	return [...counts]
		.map(([name, count]) => ({ name, count }))
		.sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
}

const sorters: Record<Sort, (a: Title, b: Title) => number> = {
	added: (a, b) => Date.parse(b.added_at) - Date.parse(a.added_at),
	title: (a, b) => a.title.localeCompare(b.title),
	year: (a, b) => b.year - a.year,
	rating: (a, b) => b.rating - a.rating,
	size: (a, b) => (b.file_summary?.size ?? 0) - (a.file_summary?.size ?? 0),
};

export function visible(titles: Title[], kind: Kind, watch: Watch[], v: View): Title[] {
	const list = flagged(titles, kind, watch, v.flags).filter(
		(t) => !v.genre || t.genres.includes(v.genre),
	);
	return list.sort((a, b) => sorters[v.sort](a, b) || a.title.localeCompare(b.title));
}

export function viewToQuery(v: View): string {
	const parts: string[] = [];
	if (v.flags.length) parts.push(`flags=${v.flags.join(",")}`);
	if (v.genre) parts.push(`genre=${encodeURIComponent(v.genre)}`);
	if (v.sort !== DEFAULT_VIEW.sort) parts.push(`sort=${v.sort}`);
	if (v.layout !== DEFAULT_VIEW.layout) parts.push(`layout=${v.layout}`);
	return parts.join("&");
}

export function viewFromQuery(q: URLSearchParams): View {
	const flags = (q.get("flags") ?? "")
		.split(",")
		.filter((f): f is Flag => (FLAGS as string[]).includes(f))
		.reduce<Flag[]>((acc, f) => (acc.includes(f) ? acc : toggleFlag(acc, f)), []);
	const sort = q.get("sort");
	return {
		flags,
		genre: q.get("genre") || null,
		sort: (SORTS as string[]).includes(sort ?? "") ? (sort as Sort) : DEFAULT_VIEW.sort,
		layout: q.get("layout") === "list" ? "list" : "grid",
	};
}
