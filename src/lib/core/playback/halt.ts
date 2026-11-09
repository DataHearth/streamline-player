import { m as i18n } from "#lib/paraglide/messages.js";

const RETRY_SECS = 10;
const OFFLINE_CHECK_SECS = 5;

export type HaltIcon =
	| "audio-lines"
	| "ban"
	| "alert"
	| "cpu"
	| "folder"
	| "play"
	| "retry"
	| "server"
	| "wifi-off";

export type Halt = {
	kind: "stop" | "restart" | "busy" | "unreachable" | "offline" | "nocodec" | "missing" | "error";
	eyebrow: string;
	icon: HaltIcon;
	title: string;
	body?: string;
	mono?: string;
	note?: string;
	wait?: string;
	count?: (left: number) => string;
	failed: boolean;
	retryLabel?: string;
	/** What the admin wrote, and who; `by` without a message says no reason was given. */
	message?: { by: string; text: string };
	alts?: {
		icon: HaltIcon;
		label: string;
		sub: string;
		run: () => void;
	}[];
	left?: number;
	total?: number;
};

type Context = { host: string; at?: string; titleName?: string };

/** `raw` is what Rust returned: the server's error as JSON, anything else means no answer at all. */
export function haltFor(
	raw: string,
	{ host, at, titleName }: Context,
	playerName: string,
): { halt: Halt; retryIn?: number } {
	let error: { code?: string; message?: string; retry_after?: number; by?: string } | null = null;
	try {
		const parsed = JSON.parse(raw);
		error = parsed.error ?? parsed;
	} catch {
		// Not JSON: the request never got an answer from the server.
	}
	if (error?.code === "session_stopped") {
		return {
			halt: {
				kind: "stop",
				eyebrow: i18n.halt_stop_eyebrow(),
				icon: "ban",
				title: i18n.halt_stop_title(),
				message: { by: error.by ?? "", text: error.message ?? "" },
				note: at ? i18n.halt_stop_note_at({ at }) : i18n.halt_stop_note(),
				failed: true,
				retryLabel: i18n.halt_try_again(),
			},
		};
	}
	if (error?.code === "server_restarting") {
		return {
			retryIn: error.retry_after ?? RETRY_SECS,
			halt: {
				kind: "restart",
				eyebrow: i18n.halt_restart_eyebrow(),
				icon: "retry",
				title: i18n.halt_restart_title(),
				message: error.message ? { by: error.by ?? "", text: error.message } : undefined,
				count: (left) =>
					at ? i18n.halt_restart_count_at({ left, at }) : i18n.halt_restart_count({ left }),
				failed: false,
			},
		};
	}
	if (error?.code === "transcode_limit") {
		return {
			retryIn: error.retry_after ?? RETRY_SECS,
			halt: {
				kind: "busy",
				eyebrow: i18n.halt_busy_eyebrow(),
				icon: "cpu",
				title: i18n.halt_busy_title(),
				body: i18n.halt_busy_body({ host }),
				count: (left) => i18n.halt_count_retry({ left }),
				note: i18n.halt_busy_note({ at: at ?? "" }),
				failed: false,
				retryLabel: i18n.halt_try_now(),
			},
		};
	}
	if (error?.code === "file_not_found" || error?.code === "media_not_found") {
		return {
			halt: {
				kind: "missing",
				eyebrow: i18n.halt_missing_eyebrow(),
				icon: "folder",
				title: i18n.halt_missing_title(),
				body: i18n.halt_missing_body(),
				mono: error.message,
				note: i18n.halt_missing_note(),
				failed: true,
			},
		};
	}
	if (error?.code === "transcode_unavailable" || error?.code === "ffmpeg_unavailable") {
		return {
			halt: {
				kind: "nocodec",
				eyebrow: i18n.halt_nocodec_eyebrow(),
				icon: "alert",
				title: i18n.halt_nocodec_title({ title: titleName ?? "" }),
				body: i18n.halt_nocodec_body({ player: playerName, message: error.message ?? "" }),
				note: i18n.halt_nocodec_note(),
				failed: true,
			},
		};
	}
	if (error) {
		return {
			halt: {
				kind: "error",
				eyebrow: i18n.halt_error_eyebrow(),
				icon: "alert",
				title: i18n.halt_error_title(),
				mono: error.message ?? error.code ?? raw,
				failed: true,
				retryLabel: i18n.halt_try_now(),
			},
		};
	}
	if (navigator.onLine === false) {
		return {
			retryIn: OFFLINE_CHECK_SECS,
			halt: {
				kind: "offline",
				eyebrow: i18n.halt_offline_eyebrow(),
				icon: "wifi-off",
				title: i18n.halt_offline_title(),
				body: at ? i18n.halt_offline_body_at({ at }) : i18n.halt_offline_body(),
				wait: i18n.halt_offline_wait({ secs: OFFLINE_CHECK_SECS }),
				failed: false,
				retryLabel: i18n.halt_try_again(),
			},
		};
	}
	return {
		retryIn: RETRY_SECS,
		halt: {
			kind: "unreachable",
			eyebrow: i18n.halt_unreachable_eyebrow(),
			icon: "server",
			title: i18n.halt_unreachable_title({ host }),
			body: i18n.halt_unreachable_body(),
			count: (left) =>
				at ? i18n.halt_unreachable_count_at({ left, at }) : i18n.halt_count_retry({ left }),
			failed: true,
			retryLabel: i18n.halt_try_now(),
		},
	};
}
