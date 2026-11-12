import type { PairRequest } from "#lib/core/types.ts";

const CODE = /^[BCDFGHJKLMNPQRSTVWXZ23456789]{6}$/;
const MINUTE_MS = 60_000;

/** `XXX-XXX` from what a person typed, or null when it cannot be a pairing code. */
export function normalizeCode(raw: string): string | null {
	const code = raw.replace(/[\s-]/g, "").toUpperCase();
	return CODE.test(code) ? `${code.slice(0, 3)}-${code.slice(3)}` : null;
}

export function requestMeta(
	r: PairRequest,
	now: Date,
	words: { justNow: string; minutes(n: number): string },
): string {
	const minutes = Math.floor((now.getTime() - Date.parse(r.requested_at)) / MINUTE_MS);
	return `${r.platform} · ${minutes < 1 ? words.justNow : words.minutes(minutes)} · ${r.ip}`;
}
