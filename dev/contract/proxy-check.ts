// The proxy reports a response off the contract and a route the contract lacks, and nothing for a valid reply.
import { spawn } from "node:child_process";
import { strictEqual as eq, ok } from "node:assert";
import { once } from "node:events";
import { mkdtempSync, readFileSync, existsSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";

const replies: Record<string, unknown> = {
	"/api/v1/me": { name: 1 },
	"/api/v1/watchstate": { items: [] },
	"/api/v1/nowhere": {},
};
const upstream = createServer((req, res) => {
	res.setHeader("Content-Type", "application/json");
	res.end(JSON.stringify(replies[req.url ?? ""] ?? {}));
});
upstream.listen(0, "127.0.0.1");
await once(upstream, "listening");
const upstreamPort = (upstream.address() as { port: number }).port;

const report = join(mkdtempSync(join(tmpdir(), "proxy-check-")), "report.jsonl");
const proxy = spawn(
	process.execPath,
	[
		"dev/contract/proxy.ts",
		"--listen",
		"127.0.0.1:0",
		"--upstream",
		`http://127.0.0.1:${upstreamPort}`,
		"--spec",
		"docs/contract/player.openapi.yaml",
		"--report",
		report,
	],
	{ stdio: ["ignore", "pipe", "inherit"] },
);
const listening = await new Promise<string>((resolve, reject) => {
	proxy.once("exit", (code) => reject(new Error(`proxy exited with ${code}`)));
	proxy.stdout.on("data", (chunk) => {
		const m = /listening on (\S+)/.exec(String(chunk));
		if (m) resolve(m[1]);
	});
});

try {
	for (const path of Object.keys(replies))
		await fetch(`http://${listening}${path}`).then((r) => r.text());
	const lines = existsSync(report)
		? readFileSync(report, "utf8")
				.split("\n")
				.filter(Boolean)
				.map((l) => JSON.parse(l))
		: [];
	eq(lines.length, 2);
	ok(lines.some((l) => l.path === "/api/v1/me" && l.where === "response"));
	ok(lines.some((l) => l.path === "/api/v1/nowhere" && l.where === "undocumented"));
	console.log("proxy-check: ok");
} finally {
	proxy.kill();
	upstream.close();
}
