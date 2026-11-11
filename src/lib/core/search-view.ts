import type { Kind, Title } from "#lib/core/types.ts";

export type Hit = Title & { kind: Kind };

const KEY = "recent-searches";
const MAX_RECENT = 4;
const MAX_GENRES = 9;

export const tagged = (movies: Title[], shows: Title[]): Hit[] => [
	...movies.map((t) => ({ ...t, kind: "movies" as const })),
	...shows.map((t) => ({ ...t, kind: "series" as const })),
];

const norm = (x: string) => x.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

/** Titles containing the query, or with a genre that starts with it; those that start with it come first, then the better rated. */
export function searchTitles(all: Hit[], query: string): Hit[] {
	const q = norm(query.trim());
	if (!q) return [];
	const starts = (t: Hit) => (norm(t.title).startsWith(q) ? 0 : 1);
	return all
		.filter((t) => norm(t.title).includes(q) || t.genres.some((g) => norm(g).startsWith(q)))
		.sort((a, b) => starts(a) - starts(b) || b.rating - a.rating);
}

/** The genres with the most titles, each with the title whose poster stands for it. */
export function genreTiles(all: Hit[]) {
	const by = new Map<string, Hit[]>();
	for (const t of all) for (const g of t.genres) by.set(g, [...(by.get(g) ?? []), t]);
	return [...by]
		.map(([name, list]) => ({ name, count: list.length, cover: list[0]! }))
		.sort((a, b) => b.count - a.count || a.name.localeCompare(b.name))
		.slice(0, MAX_GENRES);
}

/** Newest first, without repeats, whatever the case. */
export function rememberSearch(recent: string[], query: string): string[] {
	const q = query.trim();
	if (!q) return recent;
	return [q, ...recent.filter((r) => r.toLowerCase() !== q.toLowerCase())].slice(0, MAX_RECENT);
}

export function loadRecent(): string[] {
	try {
		const v = JSON.parse(localStorage.getItem(KEY) ?? "[]");
		return Array.isArray(v) ? v.filter((x) => typeof x === "string").slice(0, MAX_RECENT) : [];
	} catch {
		return [];
	}
}

export function saveRecent(recent: string[]) {
	try {
		localStorage.setItem(KEY, JSON.stringify(recent));
	} catch {
		// Kept until the app closes.
	}
}
