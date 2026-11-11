// node dev/playback-check.ts
import { strictEqual as eq, deepStrictEqual as same } from "node:assert";
import { autoForcedSub, cueAt, initialAudio, parseCues } from "../src/lib/core/playback/rules.ts";

const srt =
	"1\r\n00:00:01,000 --> 00:00:02,500\r\n<i>Hello</i>\r\n\r\n2\n00:00:02,000 --> 00:00:04,000 X:1\nthere\n";
same(parseCues(srt), [
	{ start: 1, end: 2.5, text: "Hello" },
	{ start: 2, end: 4, text: "there" },
]);
eq(cueAt(parseCues(srt), 2.2), "Hello\nthere");
eq(cueAt(parseCues(srt), 5), "");

const jpn = { index: 1, codec: "aac", channels: 2, language: "jpn", default: false };
const eng = { index: 0, codec: "eac3", channels: 6, language: "eng", default: true };
eq(initialAudio([eng, jpn], { anime: true, animeRule: true, audioLang: "Original language" }), jpn);
eq(
	initialAudio([eng, jpn], { anime: false, animeRule: true, audioLang: "Original language" }),
	undefined,
);
eq(initialAudio([eng, jpn], { anime: false, animeRule: false, audioLang: "English" }), eng);
eq(
	initialAudio([jpn, eng], {
		anime: true,
		animeRule: true,
		audioLang: "English",
		chosen: "English",
	}),
	eng,
);
eq(
	initialAudio([jpn, eng], {
		anime: true,
		animeRule: true,
		audioLang: "English",
		chosen: "German",
	}),
	jpn,
);

const sub = (index: number, language: string, forced = false, sdh = false) => ({
	index,
	codec: "subrip",
	language,
	forced,
	sdh,
	files: {},
});
const base = { anime: false, animeRule: false, myLanguage: "English", burnOnly: () => false };
// Forced subtitles follow the audio's language; "Off" turns them off; "Always" picks the full track in my language, non-SDH first.
eq(
	autoForcedSub({
		...base,
		subs: [sub(0, "fre", true), sub(1, "eng")],
		audio: { ...eng, language: "fre" },
		subsMode: "Auto",
	})?.index,
	0,
);
eq(
	autoForcedSub({
		...base,
		subs: [sub(0, "fre", true)],
		audio: { ...eng, language: "fre" },
		subsMode: "Off",
	}),
	null,
);
eq(
	autoForcedSub({
		...base,
		subs: [sub(0, "eng", false, true), sub(1, "eng")],
		audio: eng,
		subsMode: "Always",
	})?.index,
	1,
);
// A burn-only forced track is never picked on its own.
eq(
	autoForcedSub({
		...base,
		subs: [sub(0, "fre", true)],
		audio: { ...eng, language: "fre" },
		subsMode: "Auto",
		burnOnly: () => true,
	}),
	null,
);

// A language chosen on the title page is always on, whatever the device's mode says.
eq(
	autoForcedSub({
		...base,
		subs: [sub(0, "eng"), sub(1, "fre")],
		audio: eng,
		subsMode: "Off",
		subLanguage: "French",
	})?.index,
	1,
);
