import type { Kind, Title, Watch } from "#lib/core/types.ts";

const DAY_MS = 86_400_000;
const NEW_DAYS = 7;
const MONTH_DAYS = 30;

export type Ref = { kind: Kind; id: number };
export type ResumeItem = Ref & {
	title: string;
	/** "S2 · E4" for an episode, the year for a movie. */
	code: string;
	/** The episode's title; empty for a movie. */
	sub: string;
	/** Seconds still to watch. */
	left: number;
	pct: number;
	season?: number;
	episode?: number;
	episodeId?: number;
	mediaId: string;
};
/** What the screen's language does to a number: the one thing a meta line cannot decide in `core`. */
export type MetaFormat = {
	runtime(minutes: number): string;
	seasons(count: number): string;
	rating(value: number): string;
};

const unfinished = (w: Watch) => !w.watched && w.position_seconds > 0;
const addedMs = (t: Title) => Date.parse(t.added_at);
const ageDays = (t: Title, now: Date) => (now.getTime() - addedMs(t)) / DAY_MS;
const day = (d: Date) => d.toISOString().slice(0, 10);

/** One entry per title, in the order of `watch` (newest first); a series whose episode is not loaded is skipped. */
export function continueWatching(movies: Title[], shows: Title[], watch: Watch[]): ResumeItem[] {
	const out: ResumeItem[] = [];
	const seen = new Set<string>();
	for (const w of watch) {
		if (!unfinished(w)) continue;
		const base = {
			left: Math.max(0, w.duration_seconds - w.position_seconds),
			pct: w.duration_seconds > 0 ? Math.round((w.position_seconds / w.duration_seconds) * 100) : 0,
			mediaId: w.media_id,
		};
		if (w.series_id !== undefined) {
			const key = `series:${w.series_id}`;
			const show = shows.find((t) => t.id === w.series_id);
			const season = show?.seasons?.find((s) => s.episodes.some((e) => e.id === w.episode_id));
			const ep = season?.episodes.find((e) => e.id === w.episode_id);
			if (seen.has(key) || !show || !season || !ep) continue;
			seen.add(key);
			out.push({
				...base,
				kind: "series",
				id: show.id,
				title: show.title,
				code: `S${season.number} · E${ep.number}`,
				sub: ep.title,
				season: season.number,
				episode: ep.number,
				episodeId: ep.id,
			});
		} else if (w.movie_id !== undefined) {
			const key = `movies:${w.movie_id}`;
			const movie = movies.find((t) => t.id === w.movie_id);
			if (seen.has(key) || !movie) continue;
			seen.add(key);
			out.push({
				...base,
				kind: "movies",
				id: movie.id,
				title: movie.title,
				code: String(movie.year),
				sub: "",
			});
		}
	}
	return out;
}

export const recentlyAdded = (titles: Title[], limit: number) =>
	[...titles].sort((a, b) => addedMs(b) - addedMs(a)).slice(0, limit);

/** How many of `titles` came in the last 30 days: the row's "N this month". */
export const recentCount = (titles: Title[], now: Date) =>
	titles.filter((t) => ageDays(t, now) <= MONTH_DAYS).length;

export const isNew = (t: Title, now: Date) => ageDays(t, now) <= NEW_DAYS;

export function topGenre(titles: Title[]): string | null {
	const counts = new Map<string, number>();
	for (const t of titles) for (const g of t.genres) counts.set(g, (counts.get(g) ?? 0) + 1);
	const ranked = [...counts].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
	return ranked[0]?.[0] ?? null;
}

/** A movie is watched when its row says so; a series when every episode with a file is, which needs its seasons loaded. */
export function watchedTitle(t: Title, kind: Kind, watch: Watch[]): boolean {
	if (kind === "movies") return watch.some((w) => w.movie_id === t.id && w.watched);
	const files = (t.seasons ?? []).flatMap((s) => s.episodes).filter((e) => e.has_file);
	return (
		files.length > 0 &&
		files.every((e) => watch.some((w) => w.media_id === e.media_file_id && w.watched))
	);
}

export function metaLine(t: Title, kind: Kind, f: MetaFormat): string[] {
	const stars = `★ ${f.rating(t.rating)}`;
	if (kind === "movies") {
		return [
			String(t.year),
			t.runtime ? f.runtime(t.runtime) : "",
			t.certification ?? "",
			stars,
		].filter(Boolean);
	}
	const count = t.total_seasons ?? t.seasons?.length ?? 0;
	return [
		t.first_aired ?? String(t.year),
		count ? f.seasons(count) : "",
		t.network ?? "",
		stars,
	].filter(Boolean);
}

export function arrivalDay(date: string, now: Date): "today" | "yesterday" | null {
	if (date === day(now)) return "today";
	if (date === day(new Date(now.getTime() - DAY_MS))) return "yesterday";
	return null;
}

/** The episode Play starts: the first one with a file that is not watched yet, else the first. Needs the seasons loaded. */
export function nextEpisode(t: Title, watch: Watch[]) {
	const files = (t.seasons ?? []).flatMap((s) =>
		s.episodes.filter((e) => e.has_file).map((e) => ({ ...e, season: s.number })),
	);
	return (
		files.find((e) => !watch.some((w) => w.media_id === e.media_file_id && w.watched)) ?? files[0]
	);
}

export const playUrl = (kind: Kind, id: number, episodeId?: number, resuming = false) => {
	const query = [episodeId ? `ep=${episodeId}` : "", resuming ? "resume=1" : ""].filter(Boolean);
	return `/player/${kind}/${id}${query.length ? `?${query.join("&")}` : ""}`;
};
