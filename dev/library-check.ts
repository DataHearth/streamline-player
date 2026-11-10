// node dev/library-check.ts
import { deepStrictEqual as eq } from "node:assert";
import {
	DEFAULT_VIEW,
	genres,
	toggleFlag,
	type View,
	viewFromQuery,
	viewToQuery,
	visible,
} from "../src/lib/core/library-view.ts";
import type { Title, Watch } from "../src/lib/core/types.ts";

const movie = (
	id: number,
	name: string,
	added: string,
	year: number,
	rating: number,
	genre: string[],
	summary: Title["file_summary"],
): Title => ({
	id,
	title: name,
	year,
	overview: "",
	genres: genre,
	rating,
	added_at: added,
	file_summary: summary,
});
const movies = [
	movie(1, "The Matrix", "2026-09-01T00:00:00Z", 1999, 8.7, ["Science Fiction", "Action"], {
		resolution: "1080p",
		size: 2_000,
	}),
	movie(2, "Arrival", "2026-10-01T00:00:00Z", 2016, 7.9, ["Science Fiction", "Drama"], {
		resolution: "1080p",
		size: 1_000,
	}),
	movie(3, "Dune", "2026-11-01T00:00:00Z", 2021, 8.0, ["Science Fiction"], {
		resolution: "2160p",
		hdr: "HDR10",
		size: 9_000,
	}),
];
const watch: Watch[] = [
	{ media_id: "m1", movie_id: 1, position_seconds: 0, duration_seconds: 100, watched: true },
];
const v: View = { flags: ["uhd", "hdr"], genre: "Drama", sort: "title", layout: "list" };

eq(toggleFlag(["unwatched"], "progress"), ["progress"]);
eq(toggleFlag(["uhd"], "uhd"), []);
eq(
	visible(movies, "movies", watch, { ...DEFAULT_VIEW, flags: ["unwatched"] }).map((t) => t.id),
	[3, 2],
);
eq(
	visible(movies, "movies", watch, { ...DEFAULT_VIEW, flags: ["uhd", "hdr"] }).map((t) => t.id),
	[3],
);
eq(
	visible(movies, "movies", watch, { ...DEFAULT_VIEW, sort: "title" }).map((t) => t.title),
	["Arrival", "Dune", "The Matrix"],
);
eq(
	visible(movies, "movies", watch, { ...DEFAULT_VIEW, sort: "size" }).map((t) => t.id),
	[3, 1, 2],
);
eq(
	visible(movies, "movies", watch, { ...DEFAULT_VIEW, genre: "Drama" }).map((t) => t.id),
	[2],
);
eq(genres(movies, "movies", watch, [])[0], { name: "Science Fiction", count: 3 });
eq(
	genres(movies, "movies", watch, ["uhd"]).find((g) => g.name === "Drama"),
	{
		name: "Drama",
		count: 0,
	},
);
eq(viewToQuery(DEFAULT_VIEW), "");
eq(viewToQuery(v), "flags=uhd,hdr&genre=Drama&sort=title&layout=list");
eq(viewFromQuery(new URLSearchParams(viewToQuery(v))), v);
eq(
	viewFromQuery(new URLSearchParams(viewToQuery({ ...DEFAULT_VIEW, genre: "Sci Fi & Co" }))).genre,
	"Sci Fi & Co",
);
eq(viewFromQuery(new URLSearchParams("sort=nope&flags=x,uhd")), {
	...DEFAULT_VIEW,
	flags: ["uhd"],
});
console.log("library-check ok");
