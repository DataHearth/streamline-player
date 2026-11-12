// node dev/approve-check.ts
import { strictEqual as eq } from "node:assert";
import { normalizeCode, requestMeta } from "../src/lib/core/approve.ts";
import type { PairRequest } from "../src/lib/core/types.ts";

eq(normalizeCode(" k7f-q2m "), "K7F-Q2M");
eq(normalizeCode("k7fq2m"), "K7F-Q2M");
eq(normalizeCode("K7F Q2M"), "K7F-Q2M");
eq(normalizeCode("K7F"), null);
eq(normalizeCode(""), null);
eq(normalizeCode("K7F-Q2M9"), null);
eq(normalizeCode("K7F-A2M"), null);

const words = {
	justNow: "requested just now",
	minutes: (n: number) => `requested ${n} min ago`,
};
const request: PairRequest = {
	user_code: "K7F-Q2M",
	name: "Living room",
	platform: "Android TV",
	requested_at: "2026-11-12T12:00:00Z",
	ip: "192.168.1.42",
};
const at = (seconds: number) => new Date(Date.parse(request.requested_at) + seconds * 1000);
eq(requestMeta(request, at(20), words), "Android TV · requested just now · 192.168.1.42");
eq(requestMeta(request, at(-5), words), "Android TV · requested just now · 192.168.1.42");
eq(requestMeta(request, at(60), words), "Android TV · requested 1 min ago · 192.168.1.42");
eq(requestMeta(request, at(180), words), "Android TV · requested 3 min ago · 192.168.1.42");
console.log("approve-check: ok");
