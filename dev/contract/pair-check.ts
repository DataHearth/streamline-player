// The device approval endpoints, the pairing lifetime and links, through the contract proxy.
import { spawn } from "node:child_process";
import { strictEqual as eq, ok } from "node:assert";
import { existsSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

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
const dir = mkdtempSync(join(tmpdir(), "pair-check-"));

// A mock with no --clip behind a proxy; returns the proxy's API root and the file its violations land in.
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
	return { api: `${root}/api/v1`, report };
}

const violations = (report: string) =>
	existsSync(report) ? readFileSync(report, "utf8").split("\n").filter(Boolean) : [];
const JSON_HEADERS = { "Content-Type": "application/json" };
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

try {
	const main = await start("main", "--player-url", "http://localhost:1430");
	const { api } = main;
	const login = (await fetch(`${api}/auth/token`, {
		method: "POST",
		headers: JSON_HEADERS,
		body: JSON.stringify({
			grant_type: "password",
			email: "antoine@example.net",
			password: "streamline",
			device_name: "Chrome",
		}),
	}).then((r) => r.json())) as any;
	const bearer = { Authorization: `Bearer ${login.access_token}` };
	const start1 = async () =>
		(await fetch(`${api}/auth/device/code`, {
			method: "POST",
			headers: JSON_HEADERS,
			body: JSON.stringify({ client_id: "c1", name: "framework-13", platform: "Linux" }),
		}).then((r) => r.json())) as any;
	const poll = (device_code: string) =>
		fetch(`${api}/auth/device/token`, {
			method: "POST",
			headers: JSON_HEADERS,
			body: JSON.stringify({ device_code }),
		});
	const post = (code: string, action: string, headers: object = bearer) =>
		fetch(`${api}/auth/device/${code}/${action}`, { method: "POST", headers });
	const code404 = async (r: Response, want: string) => {
		eq(((await r.json()) as any).error.code, want);
	};

	const a = await start1();
	eq(
		a.verification_uri_complete,
		`http://localhost:1430/pair?code=${a.user_code}`,
		"the pairing link points at the player",
	);
	eq(a.verification_uri, "http://localhost:1430/pair");

	const seen = await fetch(`${api}/auth/device/${a.user_code}`, { headers: bearer });
	eq(seen.status, 200);
	const req = (await seen.json()) as any;
	eq(req.name, "framework-13");
	eq(req.platform, "Linux");
	ok(!Number.isNaN(Date.parse(req.requested_at)), req.requested_at);
	ok(req.ip, "ip");

	const asDevice = await fetch(`${api}/auth/device/${a.user_code}`, {
		headers: { "X-API-Key": "nope" },
	});
	eq(asDevice.status, 401);
	const missing = await fetch(`${api}/auth/device/ZZZ-ZZZ`, { headers: bearer });
	eq(missing.status, 404);
	await code404(missing, "no_pending_pairing");

	eq((await post(a.user_code, "approve")).status, 204);
	const approved = await poll(a.device_code);
	eq(approved.status, 200);
	const key = ((await approved.json()) as any).api_key;
	ok(key, "api_key");
	const again = await post(a.user_code, "approve");
	eq(again.status, 404);
	await code404(again, "no_pending_pairing");

	const forbidden = await fetch(`${api}/auth/device/${a.user_code}`, {
		headers: { "X-API-Key": key },
	});
	eq(forbidden.status, 403);
	await code404(forbidden, "session_required");
	const b = await start1();
	const forbiddenPost = await post(b.user_code, "approve", { "X-API-Key": key });
	eq(forbiddenPost.status, 403);
	await code404(forbiddenPost, "session_required");

	eq((await post(b.user_code, "deny")).status, 204);
	const denied = await poll(b.device_code);
	eq(denied.status, 400);
	await code404(denied, "access_denied");

	const c = await start1();
	const loose = c.user_code.replace("-", "").toLowerCase();
	eq((await fetch(`${api}/auth/device/${loose}`, { headers: bearer })).status, 200);
	eq((await post(loose, "approve")).status, 204);

	const short = await start("short", "--pair-ttl", "1");
	const d = (await fetch(`${short.api}/auth/device/code`, {
		method: "POST",
		headers: JSON_HEADERS,
		body: JSON.stringify({ client_id: "c2", name: "tv", platform: "Android TV" }),
	}).then((r) => r.json())) as any;
	eq(d.verification_uri_complete.endsWith(`/pair?code=${d.user_code}`), true);
	await sleep(1200);
	const late = await fetch(`${short.api}/auth/device/token`, {
		method: "POST",
		headers: JSON_HEADERS,
		body: JSON.stringify({ device_code: d.device_code }),
	});
	eq(late.status, 400);
	await code404(late, "expired_token");

	for (const r of [main.report, short.report]) eq(violations(r).join("\n"), "", "contract report");
	console.log("pair-check: ok");
} finally {
	for (const child of children) child.kill();
}
