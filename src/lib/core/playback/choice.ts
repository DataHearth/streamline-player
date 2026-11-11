import type { AudioTrack, SubTrack } from "#lib/core/playback/port.ts";
import { initialAudio, language } from "#lib/core/playback/rules.ts";

/** `audio` is a language's English name; `subs` is "off", "forced" (the track that goes with the audio) or a language's English name. */
export type Choice = { audio: string; subs: string };
export type Choices = Record<string, Choice>;
export type Scope = "title" | "season" | "episode";
export type Ref = { kind: "movies" | "series"; id: number; season?: number; episode?: number };

const KEY = "lang-choices";

const keyOf = (r: Ref, scope: Scope = "episode") =>
	[
		r.kind,
		r.id,
		...(scope !== "title" && r.season !== undefined ? [r.season] : []),
		...(scope === "episode" && r.episode !== undefined ? [r.episode] : []),
	].join(":");

/** The most specific choice that applies: the episode's, else its season's, else the title's. */
export function effectiveChoice(all: Choices, r: Ref): Choice | null {
	return all[keyOf(r, "episode")] ?? all[keyOf(r, "season")] ?? all[keyOf(r, "title")] ?? null;
}

/** Setting a wider scope clears the narrower choices inside it, so the newest word wins. */
export function applyChoice(all: Choices, r: Ref, scope: Scope, c: Choice): Choices {
	const at = keyOf(r, scope);
	const next = Object.fromEntries(
		Object.entries(all).filter(([k]) => scope === "episode" || !k.startsWith(`${at}:`)),
	);
	next[at] = c;
	return next;
}

export function loadChoices(): Choices {
	try {
		return JSON.parse(localStorage.getItem(KEY) ?? "{}");
	} catch {
		return {};
	}
}

export function saveChoices(all: Choices) {
	try {
		localStorage.setItem(KEY, JSON.stringify(all));
	} catch {
		// Applies until the app closes.
	}
}

export const choiceFor = (r: Ref) => effectiveChoice(loadChoices(), r);

export const audioLanguages = (audio: AudioTrack[]) => [
	...new Set(audio.map((t) => language(t.language))),
];

export const subtitleLanguages = (subs: SubTrack[]) => [
	...new Set(subs.filter((t) => !t.forced).map((t) => language(t.language))),
];

/** What the player does with nothing chosen: the device's settings applied to this file's tracks. */
export function defaultChoice(
	audio: AudioTrack[],
	o: {
		audioLang: string;
		subsMode: string;
		myLanguage: string;
		anime?: boolean;
		animeRule?: boolean;
	},
): Choice {
	const played = initialAudio(audio, {
		anime: !!o.anime,
		animeRule: !!o.animeRule,
		audioLang: o.audioLang,
	});
	const original = (audio.find((t) => t.default) ?? audio[0])?.language;
	const lang = played?.language ?? original;
	return {
		audio: lang ? language(lang) : "",
		subs: o.subsMode === "Off" ? "off" : o.subsMode === "Always" ? o.myLanguage : "forced",
	};
}

/** The subtitle rule the player runs: the title's own choice, else the device's mode. */
export function subsRule(
	c: Choice | null | undefined,
	deviceMode: string,
): { subsMode: string; subLanguage?: string } {
	if (!c) return { subsMode: deviceMode };
	if (c.subs === "off") return { subsMode: "Off" };
	if (c.subs === "forced") return { subsMode: "Auto" };
	return { subsMode: deviceMode, subLanguage: c.subs };
}
