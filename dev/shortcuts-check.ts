// node dev/shortcuts-check.ts: the desktop shell's key map.
import { keyAction } from "../src/lib/shells/desktop/shortcuts.ts";

let bad = 0;
const eq = (got: unknown, want: unknown) => {
	if (got === want) return;
	bad++;
	console.log(`FAIL: got ${JSON.stringify(got)}, want ${JSON.stringify(want)}`);
};
const k = (key: string, o = {}) => ({ key, ctrlKey: false, metaKey: false, target: null, ...o });
const input = { tagName: "INPUT" };

eq(keyAction(k("k", { ctrlKey: true }), false, false), "search");
eq(keyAction(k("k", { metaKey: true }), true, false), "search");
eq(keyAction(k("k", { ctrlKey: true }), true, false), null);
eq(keyAction(k("k", { ctrlKey: true, target: input }), false, false), "search");
eq(keyAction(k("g"), false, false), "g");
eq(keyAction(k("h"), false, true), "home");
eq(keyAction(k("m"), false, true), "movies");
eq(keyAction(k("s"), false, true), "shows");
eq(keyAction(k(","), false, true), "settings");
eq(keyAction(k("x"), false, true), null);
eq(keyAction(k("h"), false, false), null);
eq(keyAction(k("g", { ctrlKey: true }), false, false), null);
eq(keyAction(k("?"), false, false), "help");
eq(keyAction(k("?", { target: input }), false, false), null);
eq(keyAction(k("g", { target: { tagName: "TEXTAREA" } }), false, false), null);
eq(keyAction(k("h", { target: input }), false, true), null);
eq(keyAction(k("Escape"), false, false), "close");
eq(keyAction(k("Escape", { target: input }), false, false), "close");

eq(keyAction(k("k", { ctrlKey: true }), false, false, true), null);
eq(keyAction(k("g"), false, false, true), null);
eq(keyAction(k("h"), false, true, true), null);
eq(keyAction(k("?"), false, false, true), "help");
eq(keyAction(k("Escape"), false, false, true), "close");

console.log(bad ? `${bad} failed` : "shortcuts ok");
process.exit(bad ? 1 : 0);
