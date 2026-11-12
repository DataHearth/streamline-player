import { episodesOf, runtime, stillPos } from "#lib/core/format.ts";
import { choiceFor } from "#lib/core/playback/choice.ts";
import type { Launch } from "#lib/core/playback/now-playing.svelte.ts";
import { isAnime } from "#lib/core/playback/rules.ts";
import type { Kind, Title, Watch } from "#lib/core/types.ts";
import { m as i18n } from "#lib/paraglide/messages.js";

type Chapter = { start: number; end: number; title: string };
export type Segment = { from: number; to: number; title: string };
export type PlayerKey =
	| "toggle"
	| "back10"
	| "forward10"
	| "volumeUp"
	| "volumeDown"
	| "mute"
	| "subtitles"
	| "stats"
	| "next"
	| "previous"
	| "fullscreen"
	| "closeMenu"
	| "exitFullscreen"
	| "exit";
export type SeekFlash = { side: "left" | "right"; amount: number; at: number };

const SEEK_STEP = 10;
const SEEK_RUN_MS = 900;
const MORE_LIKE = 6;
const KEYS: Record<string, PlayerKey> = {
	" ": "toggle",
	ArrowLeft: "back10",
	ArrowRight: "forward10",
	ArrowUp: "volumeUp",
	ArrowDown: "volumeDown",
	m: "mute",
	c: "subtitles",
	i: "stats",
	n: "next",
	p: "previous",
	f: "fullscreen",
};

/** One segment per chapter, each running up to the next chapter's start so no gap is left in the bar. */
export function segments(duration: number, chapters: Chapter[]): Segment[] {
	if (duration <= 0 || chapters.length === 0) return [{ from: 0, to: 1, title: "" }];
	return chapters.map((c, i) => ({
		from: i === 0 ? 0 : Math.min(1, c.start / duration),
		to: i + 1 < chapters.length ? Math.min(1, chapters[i + 1]!.start / duration) : 1,
		title: c.title,
	}));
}

export function hoverAt(ratio: number, duration: number, chapters: Chapter[]) {
	const time = ratio * duration;
	return { time, chapter: chapters.findLast((c) => time >= c.start)?.title ?? "" };
}

/** `M:SS`, or `H:MM:SS` once the whole runs an hour, so the two ends of the bar line up. */
export function clock(seconds: number, of: number): string {
	const t = Math.max(0, Math.floor(seconds));
	const ss = String(t % 60).padStart(2, "0");
	if (of < 3600) return `${Math.floor(t / 60)}:${ss}`;
	return `${Math.floor(t / 3600)}:${String(Math.floor(t / 60) % 60).padStart(2, "0")}:${ss}`;
}

export function endsAt(now: Date, remaining: number, speed: number, locale?: string): string {
	const at = new Date(now.getTime() + (remaining * 1000) / speed);
	return at.toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit", hourCycle: "h23" });
}

export function keyFor(
	e: {
		key: string;
		ctrlKey: boolean;
		metaKey: boolean;
		altKey: boolean;
		target: { tagName?: string } | null;
	},
	s: { ended: boolean; menuOpen: boolean; fullscreen: boolean },
): PlayerKey | null {
	if (e.ctrlKey || e.metaKey || e.altKey) return null;
	const tag = e.target?.tagName;
	if (tag === "SELECT" || tag === "INPUT" || tag === "TEXTAREA") return null;
	if (e.key === "Escape") {
		if (s.ended) return "exit";
		if (s.menuOpen) return "closeMenu";
		return s.fullscreen ? "exitFullscreen" : "exit";
	}
	if (s.ended) return null;
	return KEYS[e.key.length === 1 ? e.key.toLowerCase() : e.key] ?? null;
}

/** The amounts add up (10, 20, …) while the same side is pressed again within 900 ms. */
export function seekFlash(prev: SeekFlash | null, side: "left" | "right", now: number): SeekFlash {
	const run = prev !== null && prev.side === side && now - prev.at < SEEK_RUN_MS;
	return { side, amount: (run ? prev.amount : 0) + SEEK_STEP, at: now };
}

type Source = {
	title(kind: Kind, id: number): Promise<Title>;
	watchState(): Promise<Watch[]>;
	artwork(kind: Kind, id: number, name: "poster" | "backdrop"): string;
	everything(): Promise<(Title & { kind: Kind })[]>;
};

async function moreLike(src: Source, kind: Kind, t: Title) {
	const g = t.genres[0] ?? "";
	const pool = (await src.everything()).filter((x) => !(x.kind === kind && x.id === t.id));
	return [
		...pool.filter((x) => x.genres[0] === g),
		...pool.filter((x) => x.genres[0] !== g && x.genres.includes(g)),
		...pool.filter((x) => !x.genres.includes(g)),
	]
		.slice(0, MORE_LIKE)
		.map((x) => ({
			title: x.title,
			poster: src.artwork(x.kind, x.id, "poster"),
			href: `/detail/${x.kind}/${x.id}`,
		}));
}

/** What a screen needs to start a title: the session's item and the pictures and words around it. */
export async function loadLaunch(
	src: Source,
	kind: Kind,
	id: number,
	ep: number | null,
	resume: boolean,
): Promise<Launch> {
	const [t, watch] = await Promise.all([src.title(kind, id), src.watchState()]);
	const poster = src.artwork(kind, id, "poster");
	const still = src.artwork(kind, id, "backdrop");
	if (kind === "movies") {
		const mediaId = t.media_files?.[0]?.id ?? "";
		return {
			key: mediaId,
			item: {
				mediaId,
				titleName: t.title,
				anime: isAnime(t.genres),
				choice: choiceFor({ kind: "movies", id: t.id }),
				start: resume ? (watch.find((w) => w.movie_id === id)?.position_seconds ?? 0) : 0,
			},
			view: {
				kind,
				id,
				heading: t.title,
				eyebrow: `${t.year} · ${t.genres[0] ?? ""}`,
				sub: `${t.year} · ${runtime(t.runtime ?? 0)}`,
				still,
				poster,
				framePos: "50% 40%",
				synopsis: t.overview,
				more: await moreLike(src, kind, t),
			},
		};
	}
	const eps = episodesOf(t);
	const at = eps.findIndex((e) => e.id === ep);
	const e = eps[at];
	if (!e) throw new Error(`no episode ${ep}`);
	const following = eps[at + 1]?.has_file ? eps[at + 1] : undefined;
	const before = eps[at - 1]?.has_file ? eps[at - 1] : undefined;
	return {
		key: e.media_file_id,
		item: {
			mediaId: e.media_file_id,
			titleName: t.title,
			anime: isAnime(t.genres),
			choice: choiceFor({ kind: "series", id, season: e.season, episode: e.number }),
			start: resume
				? (watch.find((w) => w.media_id === e.media_file_id)?.position_seconds ?? 0)
				: 0,
		},
		view: {
			kind,
			id,
			season: e.season,
			episode: e.number,
			heading: e.title,
			eyebrow: `${t.title} · S${e.season} E${e.number}`,
			sub: `S${e.season} · E${e.number} — ${e.title}`,
			still,
			poster,
			framePos: stillPos(e.number, e.season),
			synopsis: e.overview || t.overview,
			next: following && {
				code: `S${following.season} · E${following.number}`,
				title: following.title,
				overview: following.overview,
				still,
				stillPos: stillPos(following.number, following.season),
				label: i18n.player_play_episode({ season: following.season, episode: following.number }),
				href: `/player/series/${id}?ep=${following.id}`,
			},
			prev: before && { href: `/player/series/${id}?ep=${before.id}` },
			more: following ? [] : await moreLike(src, kind, t),
		},
	};
}
