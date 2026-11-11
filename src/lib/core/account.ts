import type { Me } from "#lib/core/types.ts";

export type Draft = { name: string; email: string };
export type FieldError = { field: "name" | "email" | "picture"; message: string };

export const PICTURE_TYPES = ["image/jpeg", "image/png", "image/webp"];
export const PICTURE_MAX = 2 * 1024 * 1024;

/** What `FieldError.message` holds when the server sent no words of its own: the screen words these. */
export const CODES = [
	"empty_name",
	"invalid_email",
	"email_taken",
	"too_large",
	"unsupported_type",
	"unknown",
] as const;
export type Code = (typeof CODES)[number];

const FIELD: Record<string, FieldError["field"]> = {
	email_taken: "email",
	invalid_email: "email",
	too_large: "picture",
	unsupported_type: "picture",
};

export function changed(me: Me, d: Draft): { name?: string; email?: string } {
	const name = d.name.trim();
	const email = d.email.trim();
	return {
		...(name !== me.name ? { name } : {}),
		...(email !== me.email ? { email } : {}),
	};
}

export function validate(d: Draft): FieldError | null {
	if (!d.name.trim()) return { field: "name", message: "empty_name" };
	if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(d.email.trim()))
		return { field: "email", message: "invalid_email" };
	return null;
}

export function refusal(body: string): FieldError {
	let code = "";
	let message = "";
	try {
		const e = JSON.parse(body)?.error;
		code = typeof e?.code === "string" ? e.code : "";
		message = typeof e?.message === "string" ? e.message : "";
	} catch {}
	return {
		field: FIELD[code] ?? "name",
		message: message || (CODES.includes(code as Code) ? code : "unknown"),
	};
}

/** The problem with a picture before any request leaves, as the server would name it. */
export function pictureProblem(file: {
	type: string;
	size: number;
}): "unsupported_type" | "too_large" | null {
	if (!PICTURE_TYPES.includes(file.type)) return "unsupported_type";
	return file.size > PICTURE_MAX ? "too_large" : null;
}
