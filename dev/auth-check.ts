// node dev/auth-check.ts
import { match, strictEqual as eq } from "node:assert";
import { browserDeviceName, challengeFor, pkcePair, safeNext } from "../src/lib/core/auth.ts";

eq(
	await challengeFor("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
	"E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
);
const p = await pkcePair();
match(p.verifier, /^[A-Za-z0-9_-]{43}$/);
eq(await challengeFor(p.verifier), p.challenge);

eq(
	browserDeviceName("Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0"),
	"Firefox on Linux",
);
eq(
	browserDeviceName(
		"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Safari/605.1.15",
	),
	"Safari on macOS",
);
eq(
	browserDeviceName(
		"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36 Edg/140.0.0.0",
	),
	"Edge on Windows",
);
eq(
	browserDeviceName(
		"Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36",
	),
	"Chrome on Linux",
);

eq(safeNext("/library/movies?x=1"), "/library/movies?x=1");
eq(safeNext("//evil.example"), "/");
eq(safeNext("https://evil.example"), "/");
eq(safeNext("/\\evil.example"), "/");
eq(safeNext(null), "/");
