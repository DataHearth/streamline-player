import { getLocale, setLocale } from "#lib/paraglide/runtime.js";

const DEFAULTS = {
	qHome: "Original",
	qAway: "Auto",
	autoplay: true,
	autoSkip: false,
	audioLang: "Original language",
	subsMode: "Auto",
	animeRule: false,
	subSize: "M",
	subBg: "Shadow",
	subColor: "White",
	techDetails: false,
	language: "English",
	frameRate: "On start",
	passEAC3: "Pass through",
	passTrueHD: "Pass through",
	passDTS: "Decode here",
};
export type Prefs = typeof DEFAULTS;
export type ChoiceKey = { [K in keyof Prefs]: Prefs[K] extends string ? K : never }[keyof Prefs];
export type ToggleKey = { [K in keyof Prefs]: Prefs[K] extends boolean ? K : never }[keyof Prefs];

const KEY = "prefs";
// The player kept its subtitle style under its own key before Settings existed.
const OLD_STYLE_KEY = "subtitle-style";

function load(): Prefs {
	try {
		return {
			...DEFAULTS,
			...JSON.parse(localStorage.getItem(OLD_STYLE_KEY) ?? "{}"),
			...JSON.parse(localStorage.getItem(KEY) ?? "{}"),
		};
	} catch {
		return { ...DEFAULTS };
	}
}

/**
 * This device's settings: each TV, phone or computer is set up on its own. `language` mirrors
 * Paraglide's locale (its own localStorage entry, else the TV's language), not the stored value.
 */
export const prefs = $state({ ...load(), language: getLocale() === "fr" ? "Français" : "English" });

export function setPref<K extends keyof Prefs>(key: K, value: Prefs[K]) {
	prefs[key] = value;
	// Reloads the app: messages are read when a page renders, not reactively.
	if (key === "language") return setLocale(value === "Français" ? "fr" : "en");
	try {
		localStorage.setItem(KEY, JSON.stringify(prefs));
	} catch {
		// Applies until the app closes.
	}
}

/** The language "your language" means: the app's. */
export const myLanguage = () => (prefs.language === "Français" ? "French" : "English");

/** Codecs the viewer wants decoded on this device instead of passed through to the TV or receiver. */
export const decodeHere = () =>
	(
		[
			["eac3", prefs.passEAC3],
			["truehd", prefs.passTrueHD],
			["dts", prefs.passDTS],
		] as const
	)
		.filter(([, v]) => v === "Decode here")
		.map(([c]) => c);

export const FRAME_RATE_MODE: Record<string, "off" | "start" | "always"> = {
	Off: "off",
	"On start": "start",
	Always: "always",
};
