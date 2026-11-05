import { CircleAlert, Cpu, Folder, Server, WifiOff } from "@lucide/svelte";
import type { Component } from "svelte";

const PLAYER_NAME = "ExoPlayer";
const RETRY_SECS = 10;
const OFFLINE_CHECK_SECS = 5;

export type Halt = {
	kind: "busy" | "unreachable" | "offline" | "nocodec" | "missing" | "error";
	eyebrow: string;
	icon: Component<{ size?: number; strokeWidth?: number }>;
	title: string;
	body?: string;
	mono?: string;
	note?: string;
	wait?: string;
	count?: (left: number) => string;
	failed: boolean;
	retryLabel?: string;
	alts?: { icon: Component<{ size?: number; strokeWidth?: number }>; label: string; sub: string; run: () => void }[];
	left?: number;
	total?: number;
};

type Context = { host: string; at?: string; titleName?: string };

/** `raw` is what Rust returned: the server's error as JSON, anything else means no answer at all. */
export function haltFor(raw: string, { host, at, titleName }: Context): { halt: Halt; retryIn?: number } {
	let error: { code?: string; message?: string; retry_after?: number } | null = null;
	try {
		const parsed = JSON.parse(raw);
		error = parsed.error ?? parsed;
	} catch {
		// Not JSON: the request never got an answer from the server.
	}
	if (error?.code === "transcode_limit") {
		return {
			retryIn: error.retry_after ?? RETRY_SECS,
			halt: {
				kind: "busy",
				eyebrow: "Server busy",
				icon: Cpu,
				title: "Waiting for a free slot",
				body: `All conversions on ${host} are in use, and this stream needs one.`,
				count: (left) => `Trying again in ${left} s`,
				note: `You’re at ${at}. Playback starts as soon as a slot frees up.`,
				failed: false,
				retryLabel: "Try now",
			},
		};
	}
	if (error?.code === "file_not_found" || error?.code === "media_not_found") {
		return {
			halt: {
				kind: "missing",
				eyebrow: "File missing",
				icon: Folder,
				title: "This file isn’t on the server",
				body: "Streamline can’t find it where the library expects it. It may have been moved, renamed or deleted.",
				mono: error.message,
				note: "Let your admin know. If it was moved, a library scan finds it again.",
				failed: true,
			},
		};
	}
	if (error?.code === "transcode_unavailable" || error?.code === "ffmpeg_unavailable") {
		return {
			halt: {
				kind: "nocodec",
				eyebrow: "Can’t play here",
				icon: CircleAlert,
				title: `${titleName} can’t play on this device`,
				body: `${PLAYER_NAME} can’t decode this file’s video, and the server can’t convert it: ${error.message}.`,
				note: "Play it on another device, or ask your admin to turn on software conversion.",
				failed: true,
			},
		};
	}
	if (error) {
		return {
			halt: {
				kind: "error",
				eyebrow: "Playback error",
				icon: CircleAlert,
				title: "This can’t play right now",
				mono: error.message ?? error.code ?? raw,
				failed: true,
				retryLabel: "Try now",
			},
		};
	}
	if (navigator.onLine === false) {
		return {
			retryIn: OFFLINE_CHECK_SECS,
			halt: {
				kind: "offline",
				eyebrow: "Offline",
				icon: WifiOff,
				title: "This device lost its connection",
				body: at ? `Playback picks up at ${at} as soon as it’s back.` : "This picks up as soon as it’s back.",
				wait: "Waiting for a network · checking every 5 s",
				failed: false,
				retryLabel: "Try again",
			},
		};
	}
	return {
		retryIn: RETRY_SECS,
		halt: {
			kind: "unreachable",
			eyebrow: "Can’t reach the server",
			icon: Server,
			title: `${host} isn’t answering`,
			body: "The server may be off or restarting, or this device is away from your home network without the VPN.",
			count: (left) => `Trying again in ${left} s${at ? ` · your place is saved at ${at}` : ""}`,
			failed: true,
			retryLabel: "Try now",
		},
	};
}
