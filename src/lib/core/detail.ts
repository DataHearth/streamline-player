import type { Tracks } from "#lib/core/playback/port.ts";
import type { Episode, HomeFeed, Kind, Title, Watch } from "#lib/core/types.ts";

const DAY_MS = 86_400_000;
const NEW_DAYS = 7;

export type Primary = {
	label: "play" | "resume";
	kind: Kind;
	id: number;
	season?: number;
	episode?: number;
	episodeId?: number;
	mediaId: string;
	pct: number;
} | null;
export type EpisodeState = "wanted" | "watched" | "progress" | "unwatched";
export type Tagged = Title & { kind: Kind };

const unfinished = (w: Watch | undefined): w is Watch =>
	!!w && !w.watched && w.position_seconds > 0;
const pctOf = (w: Watch) =>
	w.duration_seconds > 0 ? Math.round((w.position_seconds / w.duration_seconds) * 100) : 0;
const episodes = (t: Title) =>
	(t.seasons ?? []).flatMap((s) => s.episodes.map((e) => ({ ...e, season: s.number })));

/** A movie resumes where it stopped; a show resumes its most recent unfinished episode, else its first unwatched, else S1 E1. */
export function primaryAction(t: Title, kind: Kind, watch: Watch[]): Primary {
	if (kind === "movies") {
		const mediaId = t.media_files?.[0]?.id;
		if (!mediaId) return null;
		const w = watch.find((x) => x.media_id === mediaId);
		return {
			label: unfinished(w) ? "resume" : "play",
			kind,
			id: t.id,
			mediaId,
			pct: unfinished(w) ? pctOf(w) : 0,
		};
	}
	const files = episodes(t).filter((e) => e.has_file);
	const going = watch.find(
		(w) => unfinished(w) && files.some((e) => e.media_file_id === w.media_id),
	);
	const goingEp = going && files.find((e) => e.media_file_id === going.media_id);
	const next =
		goingEp ??
		files.find((e) => !watch.some((w) => w.media_id === e.media_file_id && w.watched)) ??
		files[0];
	if (!next) return null;
	return {
		label: goingEp ? "resume" : "play",
		kind,
		id: t.id,
		season: next.season,
		episode: next.number,
		episodeId: next.id,
		mediaId: next.media_file_id,
		pct: going && goingEp ? pctOf(going) : 0,
	};
}

export function episodeState(e: Episode, watch: Watch[]): EpisodeState {
	if (!e.has_file) return "wanted";
	const w = watch.find((x) => x.media_id === e.media_file_id);
	if (w?.watched) return "watched";
	return unfinished(w) ? "progress" : "unwatched";
}

/** "season:number" of the episodes whose files arrived in the last week, from the home feed's arrivals. */
export function newEpisodes(feed: HomeFeed | null, id: number, now: Date): Set<string> {
	const out = new Set<string>();
	const since = new Date(now.getTime() - NEW_DAYS * DAY_MS).toISOString().slice(0, 10);
	for (const day of feed?.arrivals ?? [])
		for (const a of day.items)
			if (a.kind === "series" && a.id === id && a.at.slice(0, 10) >= since)
				for (const n of a.episodes ?? []) out.add(`${a.season}:${n}`);
	return out;
}

export function defaultSeason(t: Title, watch: Watch[], asked: number | null): number {
	const seasons = t.seasons ?? [];
	if (asked !== null && seasons.some((s) => s.number === asked)) return asked;
	return primaryAction(t, "series", watch)?.season ?? seasons.at(-1)?.number ?? 1;
}

/** Same first genre first, then the titles that merely include it. */
export function moreLikeThis(t: Title, kind: Kind, all: Tagged[], limit: number): Tagged[] {
	const first = t.genres[0];
	if (first === undefined) return [];
	const pool = all.filter((x) => !(x.kind === kind && x.id === t.id));
	return [
		...pool.filter((x) => x.genres[0] === first),
		...pool.filter((x) => x.genres[0] !== first && x.genres.includes(first)),
	].slice(0, limit);
}

export type TechKey = "video" | "audio" | "subs" | "file" | "bitrate" | "added";
/** What the server does with one stream: nothing, or something the screen words. */
export type TechMode =
	| { kind: "asis" }
	| { kind: "converted"; to: string }
	| { kind: "burn"; formats: string[] }
	| { kind: "shown" };
export type TechRow = { key: TechKey; value: string; mode?: TechMode; count?: number };
export type TechContext = {
	video?: { copy: boolean; out: string };
	audio?: { copy: boolean; out: string };
	subs: { format: string; burn: boolean }[];
	bitrate?: string;
	size?: string;
	added: string;
};

/** The design's Technical details rows in its order; a show has only Video, Audio and Added. */
export function techRows(t: Title, kind: Kind, tracks: Tracks | null, ctx: TechContext): TechRow[] {
	const q = t.file_summary;
	const video = [q?.resolution, q?.codec, q?.hdr].filter(Boolean).join(" · ");
	const audio = [q?.audio, q?.channels].filter(Boolean).join(" ");
	const rows: TechRow[] = [];
	if (video)
		rows.push({
			key: "video",
			value: video,
			mode:
				ctx.video && (ctx.video.copy ? { kind: "asis" } : { kind: "converted", to: ctx.video.out }),
		});
	if (audio)
		rows.push({
			key: "audio",
			value: audio,
			mode:
				ctx.audio && (ctx.audio.copy ? { kind: "asis" } : { kind: "converted", to: ctx.audio.out }),
		});
	if (kind === "movies") {
		if (ctx.subs.length) {
			const burn = ctx.subs.filter((s) => s.burn).map((s) => s.format);
			rows.push({
				key: "subs",
				value: [...new Set(ctx.subs.map((s) => s.format))].join(", "),
				count: ctx.subs.length,
				mode: burn.length ? { kind: "burn", formats: [...new Set(burn)] } : { kind: "shown" },
			});
		}
		const file = [tracks?.container, ctx.size].filter(Boolean).join(" · ");
		if (file) rows.push({ key: "file", value: file });
		if (ctx.bitrate) rows.push({ key: "bitrate", value: ctx.bitrate });
	}
	rows.push({ key: "added", value: ctx.added });
	return rows;
}
