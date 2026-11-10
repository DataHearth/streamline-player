// The mock's browser sign-in: PKCE, password, rotating refresh tokens, revocation and CORS, all through the contract proxy.
import { spawn } from "node:child_process";
import { strictEqual as eq, ok } from "node:assert";
import { existsSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const VERIFIER = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const CHALLENGE = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
const ORIGIN = "http://localhost:1430";
const REDIRECT = `${ORIGIN}/auth/callback`;

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
		"--access-ttl",
		"2",
	],
	{ stdio: ["ignore", "pipe", "ignore"] },
);
const report = join(mkdtempSync(join(tmpdir(), "auth-flow-")), "report.jsonl");
let proxy: ReturnType<typeof spawn> | undefined;

try {
	const mockAt = await firstLine(mock, /listening on (\S+)/);
	proxy = spawn(
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
	const api = `http://${await firstLine(proxy, /listening on (\S+)/)}/api/v1`;

	const call = (path: string, init: RequestInit & { token?: string } = {}) => {
		const { token, ...rest } = init;
		return fetch(`${api}${path}`, {
			redirect: "manual",
			...rest,
			headers: {
				...(rest.body ? { "Content-Type": "application/json" } : {}),
				...(token ? { Authorization: `Bearer ${token}` } : {}),
				...(rest.headers as Record<string, string>),
			},
		});
	};
	const token = (body: object) =>
		call("/auth/token", { method: "POST", body: JSON.stringify(body) });
	const grant = (body: object) => token(body).then((r) => r.json() as Promise<any>);

	const methods: any = await call("/auth/methods").then((r) => r.json());
	ok(methods.sso.some((s: any) => s.id === "authentik"));

	const query = new URLSearchParams({
		provider: "authentik",
		redirect_uri: REDIRECT,
		state: "st-1",
		code_challenge: CHALLENGE,
		code_challenge_method: "S256",
	});
	const authorize = async () => {
		const r = await call(`/auth/authorize?${query}`);
		eq(r.status, 302);
		return new URL(r.headers.get("location") as string);
	};
	const back = await authorize();
	eq(back.searchParams.get("state"), "st-1");
	const code = back.searchParams.get("code") as string;
	ok(code);

	const exchange = (c: string, verifier: string) =>
		grant({
			grant_type: "authorization_code",
			code: c,
			code_verifier: verifier,
			redirect_uri: REDIRECT,
			device_name: "Firefox on Linux",
		});
	const first = await exchange(code, VERIFIER);
	eq(first.token_type, "Bearer");
	ok(first.access_token && first.refresh_token && first.device_id);
	eq((await exchange(code, VERIFIER)).error.code, "invalid_grant");

	const other = (await authorize()).searchParams.get("code") as string;
	eq((await exchange(other, "not-the-verifier")).error.code, "invalid_grant");

	const login = (password: string) =>
		token({
			grant_type: "password",
			email: "antoine@example.net",
			password,
			device_name: "Chrome",
		});
	const signed = await login("streamline").then((r) => r.json() as Promise<any>);
	ok(signed.access_token);
	const bad = await login("wrong");
	eq(bad.status, 401);
	eq(((await bad.json()) as any).error.code, "invalid_credentials");

	eq((await call("/me", { token: signed.access_token })).status, 200);
	await new Promise((r) => setTimeout(r, 2500));
	const expired = await call("/me", { token: signed.access_token });
	eq(expired.status, 401);
	eq(((await expired.json()) as any).error.code, "unauthorized");

	const refreshed = await grant({
		grant_type: "refresh_token",
		refresh_token: signed.refresh_token,
	});
	ok(refreshed.refresh_token && refreshed.refresh_token !== signed.refresh_token);
	eq((await call("/me", { token: refreshed.access_token })).status, 200);

	eq(
		(await grant({ grant_type: "refresh_token", refresh_token: signed.refresh_token })).error.code,
		"invalid_grant",
	);
	eq(
		(await grant({ grant_type: "refresh_token", refresh_token: refreshed.refresh_token })).error
			.code,
		"invalid_grant",
	);

	const preflight = await call("/me", { method: "OPTIONS", headers: { Origin: ORIGIN } });
	eq(preflight.status, 204);
	eq(preflight.headers.get("access-control-allow-origin"), ORIGIN);
	const evil = await call("/me", {
		method: "OPTIONS",
		headers: { Origin: "https://evil.example" },
	});
	eq(evil.headers.get("access-control-allow-origin"), null);

	const kept = await grant({
		grant_type: "password",
		email: "antoine@example.net",
		password: "streamline",
		device_name: "Safari",
	});
	const devices: any = await call("/devices", { token: kept.access_token }).then((r) => r.json());
	const mine = devices.items.find((d: any) => d.id === kept.device_id);
	eq(mine.kind, "browser");
	eq(mine.current, true);
	eq(mine.platform, "Web browser");
	ok(mine.expires_at);
	eq((await call("/devices/current", { method: "DELETE", token: kept.access_token })).status, 204);
	eq((await call("/me", { token: kept.access_token })).status, 401);

	const lines = existsSync(report) ? readFileSync(report, "utf8").split("\n").filter(Boolean) : [];
	eq(lines.length, 0, lines.join("\n"));
	console.log("auth-flow-check: ok");
} finally {
	proxy?.kill();
	mock.kill();
}
