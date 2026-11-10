// What the browse screens read and write, through the contract proxy: the home feed, profile edits, the picture,
// an empty library and failing artwork.
import { spawn } from "node:child_process";
import { strictEqual as eq, ok } from "node:assert";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const NOW = "1794225600";
const REASONS = ["continue", "new_episodes", "added"];
const PNG = Buffer.concat([Buffer.from("89504e470d0a1a0a", "hex"), Buffer.alloc(1024 - 8)]);

function firstLine(child: ReturnType<typeof spawn>, pattern: RegExp) {
	return new Promise<string>((resolve, reject) => {
		child.once("exit", (code) => reject(new Error(`exited with ${code} before ${pattern}`)));
		let seen = "";
		child.stdout?.on("data", (chunk) => {
			seen += String(chunk);
			const m = pattern.exec(seen);
			if (m) resolve(m[1]);
		});
	});
}

const children: ReturnType<typeof spawn>[] = [];
const dir = mkdtempSync(join(tmpdir(), "browse-check-"));
mkdirSync(join(dir, "art", "p"), { recursive: true });
writeFileSync(join(dir, "art", "p", "dune2.jpg"), PNG);

// A mock behind a proxy; returns the proxy's API root and the file its violations land in.
async function start(name: string, ...extra: string[]) {
	const mock = spawn(
		"python3",
		[
			"dev/mock-streamline.py",
			"--listen",
			"127.0.0.1:0",
			"--secret",
			"check",
			"--engine",
			"http://127.0.0.1:9",
			"--clip",
			"/nonexistent.mkv",
			"--now",
			NOW,
			"--art",
			join(dir, "art"),
			...extra,
		],
		{ stdio: ["ignore", "pipe", "ignore"] },
	);
	children.push(mock);
	const report = join(dir, `${name}.jsonl`);
	const mockAt = await firstLine(mock, /listening on (\S+)/);
	const proxy = spawn(
		process.execPath,
		[
			"dev/contract/proxy.ts",
			"--listen",
			"127.0.0.1:0",
			"--upstream",
			`http://${mockAt}`,
			"--spec",
			"docs/contract/player.openapi.yaml",
			"--report",
			report,
		],
		{ stdio: ["ignore", "pipe", "inherit"] },
	);
	children.push(proxy);
	const root = `http://${await firstLine(proxy, /listening on (\S+)/)}`;
	return { root, report };
}

const violations = (report: string) =>
	existsSync(report) ? readFileSync(report, "utf8").split("\n").filter(Boolean) : [];

try {
	const full = await start("full");
	const api = `${full.root}/api/v1`;
	const login = (await fetch(`${api}/auth/token`, {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({
			grant_type: "password",
			email: "antoine@example.net",
			password: "streamline",
			device_name: "Chrome",
		}),
	}).then((r) => r.json())) as any;
	const auth = { Authorization: `Bearer ${login.access_token}` };
	const call = (path: string, init: RequestInit = {}) =>
		fetch(`${api}${path}`, { ...init, headers: { ...auth, ...(init.headers as object) } });
	const json = (path: string, init?: RequestInit) => call(path, init).then((r) => r.json() as any);
	const patch = (body: object) =>
		call("/me", {
			method: "PATCH",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify(body),
		});
	const put = (body: Buffer, type: string) =>
		call("/me/picture", { method: "PUT", headers: { "Content-Type": type }, body });

	const home = await json("/home");
	eq(home.featured.length, 3);
	for (const f of home.featured) ok(REASONS.includes(f.reason), f.reason);
	ok(home.arrivals.length > 0);
	const days = home.arrivals.map((d: any) => d.date);
	eq(days.join(), [...days].sort().reverse().join());
	for (const day of home.arrivals) ok(day.items.length > 0);

	eq(
		((await patch({ display_name: "A. Langlois" }).then((r) => r.json())) as any).name,
		"A. Langlois",
	);
	eq((await json("/me")).name, "A. Langlois");
	const before = (await json("/me")).email;
	const taken = await patch({ device_email: "taken@example.net" });
	eq(taken.status, 409);
	eq(((await taken.json()) as any).error.code, "email_taken");
	const invalid = await patch({ device_email: "nope" });
	eq(invalid.status, 422);
	eq(((await invalid.json()) as any).error.code, "invalid_email");
	eq((await json("/me")).email, before);

	eq((await json("/me")).picture, false);
	eq((await put(PNG, "image/png")).status, 204);
	const me = await json("/me");
	eq(me.picture, true);
	ok(/^[A-Z]{1,2}$/.test(me.initials), me.initials);
	eq((await call("/posters/users/me/photo.jpg")).status, 200);
	const big = await put(Buffer.alloc(3 * 1024 * 1024), "image/png");
	eq(big.status, 413);
	eq(((await big.json()) as any).error.code, "too_large");
	eq((await put(PNG, "image/gif")).status, 415);
	eq((await call("/me/picture", { method: "DELETE" })).status, 204);
	eq((await json("/me")).picture, false);
	eq((await call("/posters/users/me/photo.jpg")).status, 404);

	const poster = "/posters/movies/1/poster.jpg";
	const mock = (path: string) => fetch(`${full.root}${path}`, { method: "POST" });
	eq((await call(poster)).status, 200);
	eq((await mock("/dev/art/off")).status, 204);
	eq((await call(poster)).status, 404);
	eq((await mock("/dev/art/on")).status, 204);
	eq((await call(poster)).status, 200);

	const files = (await json("/movies")).items;
	ok(files.every((m: any) => m.file_summary.size > 0));

	const lines = violations(full.report);
	eq(lines.length, 0, lines.join("\n"));

	const empty = await start("empty", "--empty");
	const emptyApi = `${empty.root}/api/v1`;
	const feed = await fetch(`${emptyApi}/home`).then((r) => r.json() as any);
	eq(feed.featured.length, 0);
	eq(feed.arrivals.length, 0);
	eq((await fetch(`${emptyApi}/movies`).then((r) => r.json() as any)).items.length, 0);
	const emptyLines = violations(empty.report);
	eq(emptyLines.length, 0, emptyLines.join("\n"));

	console.log("browse-check: ok");
} finally {
	for (const child of children) child.kill();
}
