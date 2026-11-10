// node dev/browse-check.ts
import { deepStrictEqual as eq } from "node:assert";
import {
	arrivalDay,
	continueWatching,
	isNew,
	metaLine,
	recentCount,
	recentlyAdded,
	topGenre,
	watchedTitle,
} from "../src/lib/core/browse.ts";
import type { Title, Watch } from "../src/lib/core/types.ts";

const title = (
	id: number,
	name: string,
	added: string,
	genres: string[],
	extra: Partial<Title> = {},
): Title => ({
	id,
	title: name,
	year: 2024,
	overview: "",
	genres,
	rating: 8.2,
	added_at: added,
	...extra,
});
const movies = [
	title(1, "Old", "2026-10-01T00:00:00.000Z", ["Science Fiction", "Drama"], {
		runtime: 100,
		certification: "PG-13",
	}),
	title(2, "Fresh", "2026-11-02T00:00:00.000Z", ["Science Fiction"], { runtime: 166 }),
];
const episode = (id: number, number: number, name: string) => ({
	id,
	number,
	title: name,
	overview: "",
	runtime: 50,
	has_file: true,
	media_file_id: `ep-${id}`,
});
const shows = [
	title(1, "Show", "2026-10-20T00:00:00.000Z", ["Drama"], {
		network: "Net",
		total_seasons: 2,
		first_aired: "2022–",
		seasons: [
			{ number: 1, name: "S1", episodes: [episode(11, 1, "One"), episode(12, 2, "Two")] },
			{ number: 2, name: "S2", episodes: [1, 2, 3, 4].map((n) => episode(20 + n, n, `Two ${n}`)) },
		],
	}),
];
const row = (media_id: string, w: Partial<Watch>): Watch => ({
	media_id,
	position_seconds: 600,
	duration_seconds: 3000,
	watched: false,
	...w,
});
const watch: Watch[] = [
	row("ep-24", { series_id: 1, season: 2, episode_id: 24 }),
	row("m2", { movie_id: 2, duration_seconds: 9960, position_seconds: 4980 }),
	row("m1", { movie_id: 1, watched: true, position_seconds: 6000, duration_seconds: 6000 }),
	row("ep-23", { series_id: 1, season: 2, episode_id: 23 }),
	row("ghost", { movie_id: 99 }),
];
const [old, fresh] = movies as [Title, Title];
const show = shows[0] as Title;
const now = new Date("2026-11-02T12:00:00Z");

const resume = continueWatching(movies, shows, watch);
eq(
	resume.map((r) => `${r.kind}:${r.id}`),
	["series:1", "movies:2"],
);
eq(resume[0]?.code, "S2 · E4");
eq(resume[0]?.sub, "Two 4");
eq(resume[0]?.episodeId, 24);
eq(resume[1]?.pct, 50);
eq(resume[1]?.left, 4980);
eq(continueWatching([], [], []), []);
eq(
	recentlyAdded(movies, 1).map((t) => t.id),
	[2],
);
eq(
	recentlyAdded(movies, 5).map((t) => t.id),
	[2, 1],
);
eq(recentCount(movies, now), 1);
eq(isNew(fresh, now), true);
eq(isNew(old, now), false);
eq(topGenre(movies), "Science Fiction");
eq(topGenre([]), null);
eq(topGenre([title(1, "a", "", ["B"]), title(2, "b", "", ["A"])]), "A");
eq(watchedTitle(old, "movies", watch), true);
eq(watchedTitle(fresh, "movies", watch), false);
eq(watchedTitle(show, "series", watch), false);
eq(arrivalDay("2026-11-02", now), "today");
eq(arrivalDay("2026-11-01", now), "yesterday");
eq(arrivalDay("2026-10-20", now), null);

const fmt = {
	runtime: (m: number) => `${m}m`,
	seasons: (n: number) => `${n} seasons`,
	rating: (n: number) => String(n),
};
eq(metaLine(old, "movies", fmt), ["2024", "100m", "PG-13", "★ 8.2"]);
eq(metaLine(show, "series", fmt), ["2022–", "2 seasons", "Net", "★ 8.2"]);
console.log("browse-check ok");
