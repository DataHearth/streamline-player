export type SignInErrorCode = "state" | "denied" | "invalid_grant" | "credentials" | "network";

export class SignInError extends Error {
	readonly code: SignInErrorCode;
	constructor(code: SignInErrorCode) {
		super(code);
		this.code = code;
	}
}

const base64url = (bytes: Uint8Array) =>
	btoa(String.fromCharCode(...bytes))
		.replaceAll("+", "-")
		.replaceAll("/", "_")
		.replaceAll("=", "");

export async function challengeFor(verifier: string): Promise<string> {
	const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(verifier));
	return base64url(new Uint8Array(digest));
}

export async function pkcePair(): Promise<{ verifier: string; challenge: string }> {
	const verifier = base64url(crypto.getRandomValues(new Uint8Array(32)));
	return { verifier, challenge: await challengeFor(verifier) };
}

// Edge and Opera also say Chrome, and Chrome also says Safari: the order matters.
export function browserDeviceName(userAgent: string): string {
	const browser = /Edg\//.test(userAgent)
		? "Edge"
		: /Firefox\//.test(userAgent)
			? "Firefox"
			: /Chrome\//.test(userAgent)
				? "Chrome"
				: /Safari\//.test(userAgent)
					? "Safari"
					: "Browser";
	const os = /Windows/.test(userAgent)
		? "Windows"
		: /Android/.test(userAgent)
			? "Android"
			: /iPhone|iPad/.test(userAgent)
				? "iOS"
				: /Mac OS X|Macintosh/.test(userAgent)
					? "macOS"
					: /Linux|X11/.test(userAgent)
						? "Linux"
						: "unknown OS";
	return `${browser} on ${os}`;
}

/** Only a path on this site may be the place sign-in returns to. */
export const safeNext = (next: string | null | undefined): string =>
	next?.startsWith("/") && !next.startsWith("//") && !next.startsWith("/\\") ? next : "/";
