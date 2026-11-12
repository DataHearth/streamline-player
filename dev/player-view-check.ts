// node --import ./dev/controller-check/register.mjs dev/player-view-check.ts
import { strictEqual as eq, deepStrictEqual as same } from "node:assert";

const { clock, endsAt, hoverAt, keyFor, loadLaunch, seekFlash, segments } = await import(
	"../src/lib/core/player-view.ts"
);

const chapters = [
	{ start: 0, end: 10, title: "Intro" },
	{ start: 10, end: 100, title: "Act" },
];

same(segments(100, []), [{ from: 0, to: 1, title: "" }]);
const two = segments(100, chapters);
eq(two.length, 2);
eq(two[0].to, 0.1);
eq(two[1].from, 0.1);
eq(two[1].to, 1);
const gap = segments(100, [
	{ start: 0, end: 20, title: "A" },
	{ start: 30, end: 100, title: "B" },
]);
eq(gap[0].to, gap[1].from, "a gap between chapters stayed open");
same(segments(0, chapters), [{ from: 0, to: 1, title: "" }]);

same(hoverAt(0.05, 100, chapters), { time: 5, chapter: "Intro" });
same(hoverAt(0.5, 100, chapters), { time: 50, chapter: "Act" });
same(hoverAt(0.5, 100, []), { time: 50, chapter: "" });

eq(clock(65, 600), "1:05");
eq(clock(65, 5400), "0:01:05");
eq(clock(3725, 5400), "1:02:05");

const idle = { ended: false, menuOpen: false, fullscreen: false };
const press = (key: string, extra: Record<string, unknown> = {}) => ({
	key,
	ctrlKey: false,
	metaKey: false,
	altKey: false,
	target: null,
	...extra,
});
eq(keyFor(press(" "), idle), "toggle");
eq(keyFor(press(" ", { ctrlKey: true }), idle), null);
eq(keyFor(press(" ", { metaKey: true }), idle), null);
eq(keyFor(press(" ", { altKey: true }), idle), null);
eq(keyFor(press(" ", { target: { tagName: "INPUT" } }), idle), null);
eq(keyFor(press(" ", { target: { tagName: "SELECT" } }), idle), null);
eq(keyFor(press(" ", { target: { tagName: "TEXTAREA" } }), idle), null);
eq(keyFor(press("ArrowLeft"), idle), "back10");
eq(keyFor(press("ArrowRight"), idle), "forward10");
eq(keyFor(press("ArrowUp"), idle), "volumeUp");
eq(keyFor(press("ArrowDown"), idle), "volumeDown");
for (const [k, want] of [
	["m", "mute"],
	["c", "subtitles"],
	["i", "stats"],
	["n", "next"],
	["p", "previous"],
	["f", "fullscreen"],
	["M", "mute"],
] as const)
	eq(keyFor(press(k), idle), want, k);
eq(keyFor(press("q"), idle), null);
eq(keyFor(press("Escape"), { ...idle, menuOpen: true, fullscreen: true }), "closeMenu");
eq(keyFor(press("Escape"), { ...idle, fullscreen: true }), "exitFullscreen");
eq(keyFor(press("Escape"), idle), "exit");
const over = { ...idle, ended: true };
eq(keyFor(press(" "), over), null);
eq(keyFor(press("ArrowLeft"), over), null);
eq(keyFor(press("f"), over), null);
eq(keyFor(press("Escape"), { ...over, fullscreen: true }), "exit");

const first = seekFlash(null, "right", 0);
same(first, { side: "right", amount: 10, at: 0 });
const second = seekFlash(first, "right", 500);
eq(second.amount, 20);
eq(seekFlash(second, "right", 1500).amount, 10, "a pause of over 900 ms did not reset");
eq(seekFlash(second, "left", 600).amount, 10, "a change of side did not reset");
eq(seekFlash(seekFlash(second, "right", 1200), "right", 2000).amount, 40);

eq(endsAt(new Date(2026, 10, 2, 21, 14), 5400, 1, "en-GB"), "22:44");
eq(endsAt(new Date(2026, 10, 2, 21, 14), 5400, 2, "en-GB"), "21:59");
eq(endsAt(new Date(2026, 10, 2, 23, 50), 1200, 1, "en-US"), "00:10");

const watched = [{ media_id: "m2", position_seconds: 120, duration_seconds: 3000, watched: false }];
const title = {
	id: 1,
	title: "Show",
	year: 2022,
	overview: "o",
	genres: ["Drama"],
	rating: 8,
	added_at: "",
	seasons: [
		{
			number: 1,
			name: "S1",
			episodes: [
				{
					id: 10,
					number: 1,
					title: "One",
					overview: "a",
					runtime: 40,
					has_file: true,
					media_file_id: "m1",
				},
				{
					id: 11,
					number: 2,
					title: "Two",
					overview: "b",
					runtime: 40,
					has_file: true,
					media_file_id: "m2",
				},
				{
					id: 12,
					number: 3,
					title: "Three",
					overview: "c",
					runtime: 40,
					has_file: true,
					media_file_id: "m3",
				},
			],
		},
	],
};
const movie = {
	id: 5,
	title: "Film",
	year: 2020,
	overview: "ov",
	genres: ["Drama"],
	rating: 7,
	added_at: "",
	runtime: 125,
	media_files: [{ id: "f5" }],
};
const source = {
	title: async (_k: string, id: number) => (id === 5 ? movie : title),
	watchState: async () => watched,
	artwork: (k: string, id: number, name: string) => `/${k}/${id}/${name}`,
	everything: async () => [
		{ ...movie, kind: "movies" },
		{ ...title, id: 2, title: "Other", kind: "series" },
		{ ...title, id: 3, title: "More", kind: "series" },
	],
};
const mid = await loadLaunch(source, "series", 1, 11, true);
eq(mid.key, "m2");
eq(mid.item.mediaId, "m2");
eq(mid.item.start, 120);
eq(mid.view.heading, "Two");
eq(mid.view.synopsis, "b");
eq(mid.view.next?.href, "/player/series/1?ep=12");
eq(mid.view.prev?.href, "/player/series/1?ep=10");
eq(mid.view.more.length, 0);
eq((await loadLaunch(source, "series", 1, 11, false)).item.start, 0);
const last = await loadLaunch(source, "series", 1, 12, false);
eq(last.view.next, undefined);
eq(last.view.more.length > 0, true, "the last episode has no more-like-this");
const film = await loadLaunch(source, "movies", 5, null, false);
eq(film.item.mediaId, "f5");
eq(film.view.next, undefined);
eq(film.view.prev, undefined);
eq(
	film.view.more.every((m: { href: string }) => !m.href.endsWith("/movies/5")),
	true,
);
let failed = "";
await loadLaunch(source, "series", 1, 99, false).catch((e: Error) => (failed = e.message));
eq(failed, "no episode 99");
console.log("player-view-check: ok");
process.exit(0);
