// node dev/messages-check.ts: every message exists in both languages and is used somewhere in src/.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const keys = (locale: string) => Object.keys(JSON.parse(readFileSync(`messages/${locale}.json`, "utf8"))).filter((k) => k !== "$schema");
const files = (dir: string): string[] =>
	dir.endsWith("paraglide") ? [] : readdirSync(dir).flatMap((f) => (statSync(join(dir, f)).isDirectory() ? files(join(dir, f)) : /\.(svelte|ts)$/.test(f) ? [join(dir, f)] : []));

const [en, fr] = [keys("en"), keys("fr")];
const source = files("src").map((f) => readFileSync(f, "utf8")).join("\n");
const problems = [
	...en.filter((k) => !fr.includes(k)).map((k) => `missing in fr: ${k}`),
	...fr.filter((k) => !en.includes(k)).map((k) => `missing in en: ${k}`),
	// Messages are called as i18n.key(…) or passed as i18n.key.
	...en.filter((k) => !new RegExp(`\\bi18n\\.${k}\\b`).test(source)).map((k) => `unused: ${k}`),
];
for (const p of problems) console.log(p);
console.log(`${en.length} messages, ${problems.length} problems`);
process.exit(problems.length ? 1 : 0);
