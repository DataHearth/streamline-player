import type { Decision, DecisionOption } from "#lib/core/types.ts";

export type PlayerCommand =
	| "frameRate"
	| "load"
	| "pause"
	| "play"
	| "restoreDisplay"
	| "seek"
	| "speed"
	| "stats"
	| "stop"
	| "subtitle"
	| "timing";
export type PlayerEvent =
	| "position"
	| "state"
	| "error"
	| "bandwidth"
	| "back"
	| "subtitleFailed"
	| "voiceKey";

export type Reason = { code: string; text: string };
export type Session = {
	session_id: string;
	mode: "direct" | "remux" | "audio_transcode" | "transcode";
	url: string;
	bandwidth: number;
	video: {
		action: string;
		codec: string;
		width: number;
		height: number;
		hdr: string;
		dv_profile?: number;
	};
	audio: { action: string; codec: string; channels: number };
	video_reasons: Reason[];
	audio_reasons: Reason[];
	hw: string;
};
export type SessionTracks = {
	max_bitrate: number | null;
	audio_index: number | null;
	burn_in_subtitle: number | null;
};
export type Stats = {
	player?: string;
	version: string;
	buffer: number;
	videoBitrate: number;
	audioBitrate: number;
	rendered: number;
	dropped: number;
	displayHz: number;
};
export type AudioTrack = {
	index: number;
	codec: string;
	channels: number;
	language: string;
	default: boolean;
};
export type SubTrack = {
	index: number;
	codec: string;
	language: string;
	forced: boolean;
	sdh: boolean;
	files: Record<string, string>;
};
export type Tracks = {
	preparing: boolean;
	container: string;
	video: {
		codec: string;
		width: number;
		height: number;
		profile?: string;
		hdr?: string;
		dv?: unknown;
	} | null;
	chapters: { start: number; end: number; title: string }[];
	markers: { intro: [number, number] | null; credits: [number, number] | null };
	audio: AudioTrack[];
	subtitles: SubTrack[];
	fonts: string[];
};
export type SubKind = "text" | "styled" | "image";
export type Cue = { start: number; end: number; text: string };

/** Everything the controller needs from the platform: the native player and the Rust commands around it. */
export type PlaybackPort = {
	readonly playerName: string;
	command(cmd: PlayerCommand, args?: Record<string, unknown>): Promise<unknown>;
	on<T>(event: PlayerEvent, cb: (e: T) => void): Promise<() => void>;
	capabilities(): Promise<unknown>;
	decisions(
		mediaId: string,
		options: DecisionOption[],
	): Promise<{ bit_rate: number; decisions: Decision[] }>;
	network(): Promise<"home" | "away">;
	serverHost(): Promise<string>;
	startSession(
		mediaId: string,
		capabilities: unknown,
		position: number,
		tracks: SessionTracks,
	): Promise<Session>;
	patchSession(id: string, changes: Record<string, unknown>): Promise<Session>;
	sessionInfo(id: string): Promise<{ speed: number | null }>;
	stopSession(id: string): Promise<unknown>;
	heartbeat(id: string, position: number, paused: boolean): Promise<unknown>;
	mediaTracks(mediaId: string): Promise<Tracks>;
	subtitleText(url: string): Promise<string>;
};
