import { readFileSync } from "node:fs";
import { createRequire, stripTypeScriptTypes } from "node:module";
import { fileURLToPath } from "node:url";

const { compileModule } = createRequire(import.meta.url)("svelte/compiler");

export async function load(url, context, next) {
	if (!url.endsWith(".svelte.ts")) return next(url, context);
	const file = fileURLToPath(url);
	const src = stripTypeScriptTypes(readFileSync(file, "utf8"), { mode: "strip" });
	const out = compileModule(src, { filename: file, generate: "client", dev: false });
	return { format: "module", source: out.js.code, shortCircuit: true };
}
