import type { AudioTrack, Cue, SubTrack } from "#lib/core/playback/port.ts";

export const SIZE_PX: Record<string, number> = { S: 20, M: 26, L: 32, XL: 40 };
export const DEFAULT_SIZE_PX = 26;

const parseStamp = (t: string) =>
	t.split(":").reduce((acc, part) => acc * 60 + Number(part.replace(",", ".")), 0);

export function parseCues(raw: string): Cue[] {
	const out: Cue[] = [];
	for (const block of raw.replace(/\r/g, "").split(/\n{2,}/)) {
		const lines = block.split("\n");
		const at = lines.findIndex((l) => l.includes("-->"));
		const [from, to] = lines[at]?.split("-->") ?? [];
		if (!from || !to) continue;
		const text = lines
			.slice(at + 1)
			.join("\n")
			.replace(/<[^>]*>/g, "")
			.trim();
		if (text)
			out.push({
				start: parseStamp(from.trim()),
				end: parseStamp(to.trim().split(/\s/)[0] ?? ""),
				text,
			});
	}
	return out;
}

// ponytail: a linear scan on each tick, a few thousand cues at most; binary search if it ever shows.
export const cueAt = (cues: Cue[], at: number) =>
	cues
		.filter((c) => c.start <= at && at < c.end)
		.map((c) => c.text)
		.join("\n");

export function language(code: string) {
	try {
		return new Intl.DisplayNames(["en"], { type: "language" }).of(code) ?? code;
	} catch {
		return code || "Unknown";
	}
}

export const isAnime = (genres: string[]) => genres.some((g) => /^(anime|animation)$/i.test(g));

const animeAudio = (list: AudioTrack[], anime: boolean, animeRule: boolean) =>
	anime && animeRule ? list.find((t) => language(t.language) === "Japanese") : undefined;

export function initialAudio(
	list: AudioTrack[],
	o: { anime: boolean; animeRule: boolean; audioLang: string; chosen?: string },
) {
	const chosen = o.chosen ? list.find((t) => language(t.language) === o.chosen) : undefined;
	if (chosen) return chosen;
	const wanted =
		o.audioLang === "Original language"
			? undefined
			: list.find((t) => language(t.language) === o.audioLang);
	return animeAudio(list, o.anime, o.animeRule) ?? wanted;
}

/**
 * A forced track translates the foreign passages of the audio it goes with. Never burned in on
 * its own: that would convert the video without the viewer asking.
 */
export function autoForcedSub(o: {
	subs: SubTrack[];
	audio: AudioTrack | null;
	anime: boolean;
	animeRule: boolean;
	subsMode: string;
	myLanguage: string;
	/** A language the viewer asked subtitles in for this title: always on, in place of the app's language. */
	subLanguage?: string;
	burnOnly: (t: SubTrack) => boolean;
}): SubTrack | null {
	const audio = o.audio && language(o.audio.language);
	const usable = o.subs.filter((t) => !o.burnOnly(t));
	const full = usable
		.filter((t) => !t.forced && language(t.language) === (o.subLanguage ?? o.myLanguage))
		.sort((a, b) => Number(a.sdh) - Number(b.sdh))[0];
	const forced = usable.find((t) => t.forced && language(t.language) === audio);
	// The selected audio is one of the tracks, so an anime-rule Japanese pick means the list has one.
	const always =
		!!o.subLanguage || o.subsMode === "Always" || (o.anime && o.animeRule && audio === "Japanese");
	const want = always ? (full ?? forced) : o.subsMode === "Off" ? undefined : forced;
	return want ?? null;
}
