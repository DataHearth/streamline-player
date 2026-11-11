// node dev/detail-check.ts
import { deepStrictEqual as eq } from "node:assert";
import {
	applyChoice,
	audioLanguages,
	choiceFor,
	defaultChoice,
	effectiveChoice,
	subtitleLanguages,
} from "../src/lib/core/playback/choice.ts";
import {
	defaultSeason,
	episodeState,
	moreLikeThis,
	newEpisodes,
	primaryAction,
	techRows,
} from "../src/lib/core/detail.ts";
import type { AudioTrack, SubTrack } from "../src/lib/core/playback/port.ts";
import type { Episode, HomeFeed, Title, Watch } from "../src/lib/core/types.ts";

const movie = (id: number, name: string, genres: string[], media?: string): Title => ({
	id,
	title: name,
	year: 2020,
	overview: "",
	genres,
	rating: 8,
	added_at: "2026-10-01T00:00:00Z",
	media_files: media ? [{ id: media }] : undefined,
});
const ep = (id: number, number: number, has_file = true): Episode => ({
	id,
	number,
	title: `E${number}`,
	overview: "",
	runtime: 50,
	has_file,
	media_file_id: `m${id}`,
});
const show: Title = {
	...movie(10, "Show", ["Drama"]),
	seasons: [
		{ number: 1, name: "S1", episodes: [ep(1, 1), ep(2, 2), ep(3, 3)] },
		{
			number: 2,
			name: "S2",
			episodes: [
				ep(11, 1),
				ep(12, 2),
				ep(13, 3),
				ep(14, 4),
				ep(15, 5),
				ep(16, 6),
				ep(17, 7),
				ep(18, 8),
				ep(19, 9, false),
			],
		},
	],
};
const row = (media: string, pos: number, watched = false): Watch => ({
	media_id: media,
	position_seconds: pos,
	duration_seconds: 100,
	watched,
});

const movies = [
	movie(1, "Dune", ["Science Fiction", "Drama"], "d"),
	movie(2, "Arrival", ["Science Fiction"], "a"),
	movie(3, "Heat", ["Crime", "Drama"], "h"),
	movie(4, "Alien", ["Horror", "Science Fiction"], "x"),
];
const none = movie(5, "Nothing", ["Drama"]);

eq(primaryAction(movies[0]!, "movies", [])?.label, "play");
eq(primaryAction(movies[0]!, "movies", [row("d", 50)])?.label, "resume");
eq(primaryAction(movies[0]!, "movies", [row("d", 50)])?.pct, 50);
eq(primaryAction(movies[0]!, "movies", [row("d", 100, true)])?.label, "play");
eq(primaryAction(none, "movies", []), null);

const watch = [row("m14", 40), row("m11", 100, true)];
const p = primaryAction(show, "series", watch);
eq([p?.season, p?.episode, p?.label], [2, 4, "resume"]);
const all = show
	.seasons!.flatMap((s) => s.episodes)
	.filter((e) => e.has_file)
	.map((e) => row(e.media_file_id, 100, true));
const done = primaryAction(show, "series", all);
eq([done?.season, done?.episode, done?.label], [1, 1, "play"]);
const fresh = primaryAction(show, "series", [row("m1", 100, true), row("m2", 100, true)]);
eq([fresh?.season, fresh?.episode, fresh?.label], [1, 3, "play"]);
eq(
	primaryAction(
		{ ...show, seasons: [{ number: 1, name: "", episodes: [ep(1, 1, false)] }] },
		"series",
		[],
	),
	null,
);

const s2 = show.seasons![1]!.episodes;
eq(episodeState(s2[8]!, watch), "wanted");
eq(episodeState(s2[0]!, watch), "watched");
eq(episodeState(s2[3]!, watch), "progress");
eq(episodeState(s2[4]!, watch), "unwatched");

eq(defaultSeason(show, watch, null), 2);
eq(defaultSeason(show, watch, 1), 1);
eq(defaultSeason(show, watch, 99), 2);
eq(defaultSeason(show, all, null), 1);

const tagged = [...movies, none].map((t) => ({ ...t, kind: "movies" as const }));
eq(
	moreLikeThis(movies[0]!, "movies", tagged, 2).some((t) => t.id === 1 && t.kind === "movies"),
	false,
);
eq(
	moreLikeThis(movies[0]!, "movies", tagged, 10).map((t) => t.id),
	[2, 4],
);
eq(
	moreLikeThis(movies[0]!, "movies", tagged, 1).map((t) => t.id),
	[2],
);
eq(
	moreLikeThis(
		movies[0]!,
		"movies",
		[{ ...show, genres: ["Science Fiction"], kind: "series" as const, id: 1 }, ...tagged],
		10,
	).length,
	3,
);

const feed: HomeFeed = {
	featured: [],
	arrivals: [
		{
			date: "2026-11-02",
			items: [{ kind: "series", id: 10, at: "2026-11-02T10:00:00Z", season: 2, episodes: [5, 6] }],
		},
		{
			date: "2026-09-01",
			items: [{ kind: "series", id: 10, at: "2026-09-01T10:00:00Z", season: 2, episodes: [1] }],
		},
	],
};
eq([...newEpisodes(feed, 10, new Date("2026-11-04T00:00:00Z"))], ["2:5", "2:6"]);
eq([...newEpisodes(feed, 11, new Date("2026-11-04T00:00:00Z"))], []);
eq([...newEpisodes(null, 10, new Date())], []);

const ctx = {
	subs: [
		{ format: "SRT", burn: false },
		{ format: "VobSub", burn: true },
	],
	added: "Today",
	size: "9 GB",
};
const q = { resolution: "2160p", codec: "HEVC", hdr: "HDR10", audio: "TrueHD", channels: "7.1" };
const rows = techRows({ ...movies[0]!, file_summary: q }, "movies", null, ctx).map((r) => r.key);
eq(rows, ["video", "audio", "subs", "file", "added"]);
eq(
	techRows({ ...show, file_summary: q }, "series", null, ctx).map((r) => r.key),
	["video", "audio", "added"],
);
eq(techRows({ ...movies[0]!, file_summary: q }, "movies", null, ctx)[2], {
	key: "subs",
	value: "SRT, VobSub",
	count: 2,
	mode: { kind: "burn", formats: ["VobSub"] },
});
eq(
	techRows({ ...movies[0]!, file_summary: q }, "movies", null, {
		...ctx,
		video: { copy: false, out: "H.264" },
	})[0]?.mode,
	{ kind: "converted", to: "H.264" },
);

const track = (index: number, lang: string, def = false): AudioTrack => ({
	index,
	codec: "eac3",
	channels: 6,
	language: lang,
	default: def,
});
const sub = (index: number, lang: string, forced = false): SubTrack => ({
	index,
	codec: "subrip",
	language: lang,
	forced,
	sdh: false,
	files: {},
});
const audio = [track(0, "jpn", true), track(1, "eng"), track(2, "eng")];
eq(audioLanguages(audio), ["Japanese", "English"]);
eq(subtitleLanguages([sub(0, "eng"), sub(1, "eng", true), sub(2, "fre")]), ["English", "French"]);
eq(
	defaultChoice(audio, { audioLang: "Original language", subsMode: "Auto", myLanguage: "English" }),
	{ audio: "Japanese", subs: "forced" },
);
eq(defaultChoice(audio, { audioLang: "English", subsMode: "Always", myLanguage: "French" }), {
	audio: "English",
	subs: "French",
});
eq(defaultChoice(audio, { audioLang: "French", subsMode: "Off", myLanguage: "English" }), {
	audio: "Japanese",
	subs: "off",
});
eq(
	defaultChoice(audio, {
		audioLang: "English",
		subsMode: "Auto",
		myLanguage: "English",
		anime: true,
		animeRule: true,
	}).audio,
	"Japanese",
);

const r = { kind: "series" as const, id: 10, season: 2, episode: 4 };
let c = applyChoice({}, r, "episode", { audio: "English", subs: "off" });
eq(effectiveChoice(c, r), { audio: "English", subs: "off" });
eq(effectiveChoice(c, { ...r, episode: 5 }), null);
c = applyChoice(c, r, "season", { audio: "Japanese", subs: "English" });
eq(effectiveChoice(c, r), { audio: "Japanese", subs: "English" });
eq(effectiveChoice(c, { ...r, episode: 5 }), { audio: "Japanese", subs: "English" });
c = applyChoice(c, r, "title", { audio: "French", subs: "forced" });
eq(effectiveChoice(c, r), { audio: "French", subs: "forced" });
eq(effectiveChoice(c, { ...r, season: 1, episode: 1 }), { audio: "French", subs: "forced" });
eq(
	effectiveChoice(
		applyChoice({}, { kind: "movies", id: 1 }, "title", { audio: "x", subs: "off" }),
		{ kind: "movies", id: 1 },
	),
	{ audio: "x", subs: "off" },
);
eq(choiceFor({ kind: "movies", id: 1 }), null);

console.log("detail-check ok");
