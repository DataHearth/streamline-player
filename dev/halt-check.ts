// node dev/halt-check.ts
import { ok } from "node:assert";
import { haltFor } from "../src/lib/core/playback/halt.ts";

const ICONS = [
	"audio-lines",
	"ban",
	"alert",
	"cpu",
	"folder",
	"play",
	"retry",
	"server",
	"wifi-off",
];
const error = (code: string) => JSON.stringify({ error: { code, message: "m" } });

const KINDS = {
	stop: error("session_stopped"),
	restart: error("server_restarting"),
	busy: error("transcode_limit"),
	missing: error("file_not_found"),
	nocodec: error("transcode_unavailable"),
	error: error("internal"),
	unreachable: "connection refused",
};

for (const [kind, raw] of Object.entries(KINDS)) {
	const { halt } = haltFor(raw, { host: "streamline.home.arpa" }, "mpv");
	ok(halt.kind === kind, `${kind}: got ${halt.kind}`);
	ok(ICONS.includes(halt.icon), `${kind}: ${String(halt.icon)}`);
	for (const a of halt.alts ?? []) ok(ICONS.includes(a.icon), `${kind} alt: ${String(a.icon)}`);
}

Object.defineProperty(globalThis.navigator, "onLine", { value: false, configurable: true });
const { halt } = haltFor("connection refused", { host: "h" }, "mpv");
ok(halt.kind === "offline" && ICONS.includes(halt.icon), `offline: ${String(halt.icon)}`);
