export type Action = "search" | "home" | "movies" | "shows" | "settings" | "help" | "close";

type Key = {
	key: string;
	ctrlKey: boolean;
	metaKey: boolean;
	target: { tagName?: string } | null;
};

const AFTER_G: Record<string, Action> = {
	h: "home",
	m: "movies",
	s: "shows",
	",": "settings",
};

/** The player owns the keyboard: only the shortcut sheet and closing it stay with the shell. */
export function keyAction(
	e: Key,
	mac: boolean,
	pendingG: boolean,
	inPlayer = false,
): Action | "g" | null {
	if (e.key === "Escape") return "close";
	if (inPlayer) return e.key === "?" ? "help" : null;
	if (e.key.toLowerCase() === "k" && (mac ? e.metaKey : e.ctrlKey)) return "search";
	const tag = e.target?.tagName;
	if (tag === "INPUT" || tag === "TEXTAREA") return null;
	if (e.ctrlKey || e.metaKey) return null;
	if (pendingG && AFTER_G[e.key]) return AFTER_G[e.key] ?? null;
	if (e.key === "g") return "g";
	if (e.key === "?") return "help";
	return null;
}
