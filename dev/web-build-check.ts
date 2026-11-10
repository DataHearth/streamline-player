// node dev/web-build-check.ts — checks the web build in build/: a strict CSP and no Tauri code.
import { strictEqual as eq, ok } from "node:assert";
import { createHash } from "node:crypto";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const html = readFileSync("build/index.html", "utf8");

const meta = html.match(/<meta[^>]*http-equiv="content-security-policy"[^>]*>/i)?.[0];
ok(meta, "index.html has no content-security-policy meta");
const policy = meta.match(/content="([^"]*)"/)?.[1] ?? "";
const script = policy.split(";").find((d) => d.trim().startsWith("script-src")) ?? "";
ok(script, "no script-src");
ok(!script.includes("'unsafe-inline'"), "script-src allows unsafe-inline");
ok(!script.includes("'unsafe-eval'"), "script-src allows unsafe-eval");

for (const [, body] of html.matchAll(/<script(?![^>]*\ssrc=)[^>]*>([\s\S]*?)<\/script>/g)) {
	const hash = `'sha256-${createHash("sha256").update(body).digest("base64")}'`;
	ok(script.includes(hash), `inline script is not in script-src: ${hash}`);
}

const files = (dir: string): string[] =>
	readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
		e.isDirectory() ? files(join(dir, e.name)) : [join(dir, e.name)],
	);
const tauri = files("build").filter(
	(f) => /\.(html|js|css|json|map)$/.test(f) && readFileSync(f, "utf8").includes("__TAURI"),
);
eq(tauri.length, 0, `__TAURI found in ${tauri.join(", ")}`);
