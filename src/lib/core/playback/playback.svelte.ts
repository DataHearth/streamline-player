import {
	autoCap,
	PRESETS,
	presetOf,
	type Quality,
	rememberedThroughput,
	rememberThroughput,
	stepDown,
} from "#lib/core/playback/auto.ts";
import { type Halt, haltFor } from "#lib/core/playback/halt.ts";
import {
	audioName,
	audioOutName,
	CODEC_NAMES,
	clock,
	fileMbit,
	languageLabel,
	modeName,
	originalName,
	subInfo,
} from "#lib/core/playback/names.ts";
import type {
	AudioTrack,
	Cue,
	PlaybackPort,
	PlayerCommand,
	Session,
	SessionTracks,
	Stats,
	SubTrack,
	Tracks,
} from "#lib/core/playback/port.ts";
import {
	autoForcedSub,
	cueAt,
	DEFAULT_SIZE_PX,
	initialAudio,
	parseCues,
	SIZE_PX,
} from "#lib/core/playback/rules.ts";
import { decodeHere, FRAME_RATE_MODE, myLanguage, prefs } from "#lib/core/prefs.svelte.ts";
import { toast } from "#lib/core/toast.svelte.ts";
import type { Decision } from "#lib/core/types.ts";
import { m as i18n } from "#lib/paraglide/messages.js";

const HEARTBEAT_MS = 10_000;
const MBIT = 1_000_000;
const STALL_CHECK_MS = 4000;
const STALL_GUESS_MS = 12_000;
// Going up restarts the stream, and every step down makes the next try at going up wait twice as long.
const STEP_UP_HOLD_MS = 60_000;
const STEP_UP_HOLD_MAX_MS = 16 * 60_000;
export const UP_NEXT_SECS = 10;
const STATS_MS = 1000;
const SUB_OFFSET_LIMIT = 10;
const AUDIO_DELAY_LIMIT = 500;
const RECOVERY_WINDOW_MS = 30_000;
const HALT_EXIT_MS = 240;

export type Preview = {
	bitRate: number;
	original?: Decision;
	presets: Record<number, Decision>;
	audio: Record<number, Decision>;
};
export type PlayItem = {
	mediaId: string;
	titleName: string;
	anime: boolean;
	start: number;
	onEnd?: () => void;
};
/** What only the screen can do when the playback changes under it. */
export type PlaybackUi = {
	halted?: () => void;
	ended?: () => void;
	started?: () => void;
	settled?: () => void;
	subtitlesUnavailable?: () => void;
};

export class Playback {
	#port: PlaybackPort;
	#item: PlayItem;
	#ui: PlaybackUi;
	#cues: Cue[] = [];
	#picked = false;
	#introSkipped = false;
	#measuredInStall: number | null = null;
	#stallSince = 0;
	#stepUpHold = STEP_UP_HOLD_MS;
	#betterSince: number | null = null;
	#stallTimer: ReturnType<typeof setInterval> | undefined;
	#stepping = false;
	#previewId = 0;
	#retryTimer: ReturnType<typeof setInterval> | undefined;
	#unhaltTimer: ReturnType<typeof setTimeout> | undefined;
	#statsTimer: ReturnType<typeof setInterval> | undefined;
	#startId = 0;
	#gone = false;
	#paused = false;
	#lastRecovery = 0;
	#subTouched = false;

	session = $state<Session | null>(null);
	position = $state(0);
	duration = $state(0);
	buffered = $state(0);
	playing = $state(false);
	buffering = $state(true);
	ended = $state(false);
	tracks = $state<Tracks | null>(null);
	audioPick = $state<number | null>(null);
	subPick = $state<number | null>(null);
	burn = $state(false);
	burnBack = $state("");
	subOffset = $state(0);
	audioDelay = $state(0);
	cueText = $state("");
	quality = $state<Quality>("auto");
	autoPick = $state<number | null | undefined>(undefined);
	measured = $state<number | null>(null);
	measuredOnce = $state(false);
	preview = $state.raw<Preview | null>(null);
	speed = $state(1);
	stats = $state(false);
	live = $state<Stats | null>(null);
	encodeSpeed = $state<number | null>(null);
	upNextDismissed = $state(false);
	halt = $state<Halt | null>(null);
	haltLeaving = $state(false);
	host = $state("");
	/** Where Streamline places this device; anything but a clear "home" is away. */
	network = $state<"home" | "away" | null>(null);
	positionAt = 0;

	readonly introOn = $derived.by(() => {
		const intro = this.tracks?.markers.intro;
		return !!intro && this.position >= intro[0] && this.position < intro[1];
	});
	// Without a marker the card shows for the very end only, so no episode ending gets cut.
	readonly creditsAt = $derived.by(
		() =>
			this.tracks?.markers.credits?.[0] ??
			(this.#item.onEnd && this.duration > 0 ? this.duration - UP_NEXT_SECS : null),
	);
	readonly upNextOn = $derived.by(
		() =>
			!!this.#item.onEnd &&
			this.creditsAt !== null &&
			this.position >= this.creditsAt &&
			!this.upNextDismissed &&
			!this.ended,
	);
	readonly creditsOn = $derived(
		!!this.tracks?.markers.credits &&
			!this.upNextOn &&
			this.creditsAt !== null &&
			this.position >= this.creditsAt &&
			this.position < this.duration - 1,
	);
	readonly chapterName = $derived(
		this.tracks?.chapters.findLast((c) => this.position >= c.start)?.title ?? "",
	);
	readonly full = $derived(this.session?.mode === "direct" || this.session?.mode === "remux");
	readonly originalPlays = $derived(
		!this.preview?.original ||
			(!this.preview.original.error && this.preview.original.video.action === "copy"),
	);
	// A file this device can't play as-is has no Original to pick.
	readonly qSel = $derived<Quality>(
		this.quality === "original" && !this.originalPlays ? "auto" : this.quality,
	);
	readonly selectedAudio = $derived(
		this.tracks?.audio.find((t) => t.index === this.audioPick) ??
			this.tracks?.audio.find((t) => t.default) ??
			this.tracks?.audio[0] ??
			null,
	);

	constructor(port: PlaybackPort, item: PlayItem, ui: PlaybackUi = {}) {
		this.#port = port;
		this.#item = item;
		this.#ui = ui;
		this.position = item.start;
	}

	/** Starts listening and the first session; the returned teardown also leaves the session. */
	begin(): () => void {
		this.#port.serverHost().then((h) => (this.host = h));
		const listeners = [
			this.#port.on<{ position: number; buffered: number; duration: number }>("position", (e) => {
				this.position = e.position;
				this.buffered = e.buffered;
				this.positionAt = performance.now();
				this.duration = e.duration;
				this.#watch();
			}),
			this.#port.on<{ state: string }>("state", (e) => {
				this.buffering = e.state === "buffering";
				this.playing = e.state === "playing";
				if (!this.buffering) {
					clearInterval(this.#stallTimer);
					this.#stallTimer = undefined;
				} else if (this.#stallTimer === undefined) {
					this.#newStallEvidence();
					this.#stallTimer = setInterval(() => this.#autoStepDown(), STALL_CHECK_MS);
				}
				if (e.state === "ended") {
					if (this.#item.onEnd) this.#item.onEnd();
					else this.#finish();
				}
			}),
			this.#port.on<{ code: string; message: string }>("error", (e) => {
				if (
					e.code.startsWith("ERROR_CODE_IO_") &&
					Date.now() - this.#lastRecovery > RECOVERY_WINDOW_MS
				) {
					this.#recoverSession();
					return;
				}
				this.buffering = false;
				this.#showHalt({
					kind: "error",
					eyebrow: i18n.halt_error_eyebrow(),
					icon: "alert",
					title: i18n.halt_stream_stopped_title(),
					mono: `${e.code}: ${e.message}`,
					failed: true,
					retryLabel: i18n.halt_try_now(),
				});
			}),
			this.#port.on<{ bitrate: number; source: "meter" | "reads" }>("bandwidth", (e) => {
				// Reads stop at what a converted stream's encoder produces, so only the meter sees its link.
				if (!this.session || (this.session.mode === "direct") !== (e.source === "reads")) return;
				// One slow segment is common. Two in a row are a slow link.
				this.#measuredInStall = Math.max(e.bitrate, this.measured ?? 0);
				this.measured = e.bitrate;
				this.measuredOnce = true;
				if (this.network) rememberThroughput(e.bitrate, this.network);
				this.#considerStepUp();
			}),
		];
		this.start(this.#item.start);
		const cueTimer = setInterval(() => this.#updateCue(), 100);
		const beat = setInterval(() => {
			if (this.session)
				this.#port
					.heartbeat(this.session.session_id, this.position, !this.playing)
					.catch((e) => this.#serverStopped(e));
		}, HEARTBEAT_MS);
		return () => {
			clearInterval(this.#stallTimer);
			clearInterval(cueTimer);
			clearInterval(beat);
			clearInterval(this.#retryTimer);
			clearInterval(this.#statsTimer);
			for (const l of listeners) l.then((off) => off());
			this.#startId++;
			this.#gone = true;
			this.leave();
		};
	}

	#cmd(cmd: PlayerCommand, args?: Record<string, unknown>) {
		if (cmd === "seek" || cmd === "load") this.#newStallEvidence();
		return this.#port.command(cmd, args);
	}

	#newStallEvidence() {
		this.#stallSince = Date.now();
		this.#measuredInStall = null;
	}

	// Auto-skip and auto-play follow the position, so they are checked wherever it or the markers change.
	#watch() {
		if (this.#gone) return;
		if (this.introOn && prefs.autoSkip && !this.#introSkipped && !this.ended) {
			this.#introSkipped = true;
			this.skip();
		}
		if (
			prefs.autoplay &&
			this.#item.onEnd &&
			this.upNextOn &&
			this.creditsAt !== null &&
			this.position >= this.creditsAt + UP_NEXT_SECS
		)
			this.#item.onEnd();
	}

	#updateCue() {
		if (!this.#cues.length) {
			if (this.cueText) this.cueText = "";
			return;
		}
		const now =
			(this.playing
				? this.position + ((performance.now() - this.positionAt) / 1000) * this.speed
				: this.position) - this.subOffset;
		const text = cueAt(this.#cues, now);
		if (text !== this.cueText) this.cueText = text;
	}

	syncTiming() {
		const size = (SIZE_PX[prefs.subSize] ?? DEFAULT_SIZE_PX) / DEFAULT_SIZE_PX;
		this.#cmd("timing", {
			subtitleOffset: this.subOffset,
			subtitleScale: size,
			audioDelay: this.audioDelay,
		}).catch(() => {});
	}

	nudgeSub(step: number) {
		this.subOffset =
			step === 0
				? 0
				: Math.max(
						-SUB_OFFSET_LIMIT,
						Math.min(SUB_OFFSET_LIMIT, Math.round((this.subOffset + step) * 10) / 10),
					);
		this.syncTiming();
	}

	nudgeAudio(step: number) {
		this.audioDelay =
			step === 0
				? 0
				: Math.max(-AUDIO_DELAY_LIMIT, Math.min(AUDIO_DELAY_LIMIT, this.audioDelay + step));
		this.syncTiming();
	}

	resetTiming(what: "sub" | "audio") {
		if (what === "sub") this.nudgeSub(0);
		else this.nudgeAudio(0);
	}

	#currentCap() {
		if (typeof this.quality === "number") return this.quality;
		return this.qSel === "auto" ? (this.autoPick ?? null) : null;
	}

	async #refreshPreview() {
		const id = ++this.#previewId;
		const list = this.tracks?.audio ?? [];
		const base = {
			audio_index: this.audioPick,
			burn_in_subtitle: this.burn ? this.subPick : null,
		};
		const capMbit = this.#currentCap();
		const cap = capMbit && capMbit * MBIT;
		try {
			const r = await this.#port.decisions(this.#item.mediaId, [
				base,
				...PRESETS.map((n) => ({ ...base, max_bitrate: n * MBIT })),
				...list.map((t) => ({ ...base, audio_index: t.index, max_bitrate: cap })),
			]);
			if (id !== this.#previewId) return;
			this.preview = {
				bitRate: r.bit_rate,
				original: r.decisions[0],
				presets: Object.fromEntries(
					PRESETS.flatMap((n, i) =>
						r.decisions[i + 1] ? [[n, r.decisions[i + 1] as Decision]] : [],
					),
				),
				audio: Object.fromEntries(
					list.flatMap((t, i) =>
						r.decisions[i + 1 + PRESETS.length]
							? [[t.index, r.decisions[i + 1 + PRESETS.length] as Decision]]
							: [],
					),
				),
			};
		} catch {
			// The panels fall back to what the running session says.
		}
	}

	async #offerAlternatives() {
		await this.#refreshPreview();
		const preview = this.preview;
		if (this.halt?.kind !== "busy" || !preview) return;
		const cap = this.#currentCap();
		const wanted = cap ? preview.presets[cap] : preview.original;
		const alts: NonNullable<Halt["alts"]> = [];
		const orig = preview.original;
		if (orig && !orig.error && !orig.needs_slot) {
			alts.push({
				icon: "play",
				label: i18n.player_alt_play_original(),
				sub: `${modeName(orig.mode)} · ${originalName(this.tracks?.video ?? null)} · ${fileMbit(preview.bitRate)}`,
				run: () => {
					this.quality = "original";
					this.start();
					toast(i18n.player_toast_playing_original({ mode: modeName(orig.mode) }));
				},
			});
		}
		if (
			wanted &&
			!wanted.error &&
			wanted.video.action === "copy" &&
			wanted.audio.action !== "copy"
		) {
			const t = this.tracks?.audio.find((x) => {
				const d = this.preview?.audio[x.index];
				return d && !d.error && !d.needs_slot;
			});
			if (t) {
				const name = `${languageLabel(t.language)} · ${audioName(t)}`;
				alts.push({
					icon: "audio-lines",
					label: i18n.player_alt_switch_audio({ name }),
					sub: i18n.player_alt_plays_as_is(),
					run: () => {
						this.audioPick = t.index;
						this.start();
						toast(i18n.player_toast_playing({ name }));
					},
				});
			}
		}
		const need =
			wanted && !wanted.error
				? [...wanted.video_reasons, ...wanted.audio_reasons].map((r) => r.text).join(", ")
				: "";
		this.halt = {
			...this.halt,
			alts,
			body: need
				? i18n.halt_busy_body_need({ host: this.host, need })
				: i18n.halt_busy_body({ host: this.host }),
		};
	}

	#autoForced() {
		const tracks = this.tracks;
		if (this.#subTouched || !tracks || tracks.preparing) return;
		const want = autoForcedSub({
			subs: tracks.subtitles,
			audio: this.selectedAudio,
			anime: this.#item.anime,
			animeRule: prefs.animeRule,
			subsMode: prefs.subsMode,
			myLanguage: myLanguage(),
			burnOnly: (t) => !!subInfo(t).burnOnly,
		});
		if ((want?.index ?? null) !== this.subPick) this.pickSub(want, false);
	}

	async pickSub(t: SubTrack | null, byViewer = true) {
		this.#subTouched ||= byViewer;
		const info = t && subInfo(t);
		const wantsBurn = !!info?.burnOnly;
		const changedBurn = wantsBurn !== this.burn || (wantsBurn && this.subPick !== t?.index);
		if (wantsBurn && !this.burn && this.session) this.burnBack = modeName(this.session.mode);
		this.subPick = t?.index ?? null;
		this.burn = wantsBurn;
		this.#cues = [];
		if (changedBurn) await this.#repatch({ burn_in_subtitle: wantsBurn ? t?.index : null });
		try {
			if (!t || !info || wantsBurn) {
				await this.#cmd("subtitle", {});
			} else if (info.kind === "text") {
				await this.#cmd("subtitle", {});
				this.#cues = parseCues(await this.#port.subtitleText(t.files.vtt as string));
			} else if (info.kind === "styled") {
				await this.#cmd("subtitle", {
					kind: "ass",
					url: t.files.ass,
					fonts: this.tracks?.fonts ?? [],
				});
			} else {
				await this.#cmd("subtitle", { kind: "pgs", url: t.files.sup });
			}
			this.syncTiming();
		} catch {
			this.#ui.subtitlesUnavailable?.();
		}
	}

	async pickAudio(t: AudioTrack) {
		if (t.index === this.selectedAudio?.index) return;
		this.audioPick = t.index;
		await this.#repatch({ audio_index: t.index });
		this.#autoForced();
	}

	#sessionTracks(): SessionTracks {
		return {
			max_bitrate: (this.#currentCap() ?? 0) * MBIT || null,
			audio_index: this.audioPick,
			burn_in_subtitle: this.burn ? this.subPick : null,
		};
	}

	async refreshTracks() {
		this.tracks = await this.#port.mediaTracks(this.#item.mediaId).catch(() => this.tracks);
		if (this.tracks && !this.#picked) {
			this.#picked = true;
			this.audioPick =
				initialAudio(this.tracks.audio, {
					anime: this.#item.anime,
					animeRule: prefs.animeRule,
					audioLang: prefs.audioLang,
				})?.index ?? null;
		}
		this.#autoForced();
		this.#watch();
		await this.#refreshPreview();
	}

	#clearHalt() {
		clearInterval(this.#retryTimer);
		clearTimeout(this.#unhaltTimer);
		this.halt = null;
		this.haltLeaving = false;
	}

	#unhalt() {
		clearInterval(this.#retryTimer);
		this.haltLeaving = true;
		this.#unhaltTimer = setTimeout(() => this.#clearHalt(), HALT_EXIT_MS);
	}

	#showHalt(h: Halt) {
		this.setStats(false);
		this.#cmd("pause").catch(() => {});
		clearTimeout(this.#unhaltTimer);
		this.haltLeaving = false;
		this.halt = h;
		this.#ui.halted?.();
	}

	#haltWithRetry(h: Halt, secs: number) {
		this.#showHalt({ ...h, left: secs, total: secs });
		clearInterval(this.#retryTimer);
		this.#retryTimer = setInterval(() => {
			const halt = this.halt;
			if (!halt?.left) return;
			if (halt.left <= 1) this.start();
			else this.halt = { ...halt, left: halt.left - 1 };
		}, 1000);
	}

	#resumedToast(after: Halt["kind"] | undefined, s: Session) {
		if (after === "unreachable") toast(i18n.player_toast_reconnected());
		else if (after === "offline") toast(i18n.player_toast_back_online(), { icon: "wifi" });
		else if (after === "busy" && s.mode !== "direct" && s.mode !== "remux")
			toast(i18n.player_toast_conversion_started(), {
				icon: "cpu",
				text:
					s.video.action === "copy"
						? i18n.player_toast_audio_to({ audio: audioOutName(s) })
						: `${s.video.height}p ${CODEC_NAMES[s.video.codec] ?? s.video.codec}`,
			});
	}

	async start(at = this.position) {
		const id = ++this.#startId;
		const after = this.halt?.kind;
		// The halt screen stays up through the attempt: a retry that fails again only updates it.
		clearInterval(this.#retryTimer);
		this.#stop();
		this.buffering = true;
		try {
			this.network ??= await this.#port.network();
			this.measuredOnce ||= rememberedThroughput(this.network) !== null;
			if (!this.#picked)
				this.quality = presetOf(this.network === "home" ? prefs.qHome : prefs.qAway);
			const capabilities = await this.#port.capabilities();
			await this.refreshTracks();
			if (id !== this.#startId) return;
			if (this.autoPick === undefined)
				this.autoPick = autoCap(
					rememberedThroughput(this.network ?? "away"),
					this.preview && this.preview.bitRate / MBIT,
					this.originalPlays,
				);
			const started = await this.#port.startSession(
				this.#item.mediaId,
				capabilities,
				at,
				this.#sessionTracks(),
			);
			// The viewer left while the server was still answering.
			if (id !== this.#startId) {
				this.#port.stopSession(started.session_id).catch(() => {});
				return;
			}
			this.session = started;
			await this.#cmd("frameRate", { mode: FRAME_RATE_MODE[prefs.frameRate] ?? "off" }).catch(
				() => {},
			);
			await this.#cmd("load", { url: started.url, decode: decodeHere(), at });
			if (!this.#paused) await this.#cmd("play");
			if (this.halt) this.#unhalt();
			this.#ui.started?.();
			this.#resumedToast(after, started);
		} catch (e) {
			if (id !== this.#startId) return;
			this.buffering = false;
			this.#failedToStart(String(e));
		}
	}

	// The session can vanish under a healthy player: a pause past the orphan timeout, an engine
	// restart, a network cut. The viewer only sees a spinner. Failing again right after a recovery
	// means something else is wrong, so that one is shown.
	async #recoverSession() {
		this.#lastRecovery = Date.now();
		const at = this.position;
		await this.#stop();
		await this.start(at);
		if (this.session && (!this.halt || this.haltLeaving)) toast(i18n.player_toast_reconnected());
	}

	// The heartbeat is where Streamline says an admin stopped this stream or is restarting the server.
	#serverStopped(e: unknown) {
		const raw = String(e);
		if (!raw.includes('"session_stopped"') && !raw.includes('"server_restarting"')) return;
		this.#startId++;
		this.session = null;
		this.#cmd("stop").catch(() => {});
		this.#failedToStart(raw);
	}

	#failedToStart(raw: string) {
		let { halt: h, retryIn } = haltFor(
			raw,
			{ host: this.host, at: clock(this.position), titleName: this.#item.titleName },
			this.#port.playerName,
		);
		if (h.kind === "busy" && this.halt?.kind === "busy")
			h = { ...h, alts: this.halt.alts, body: this.halt.body };
		if (retryIn) this.#haltWithRetry(h, retryIn);
		else this.#showHalt(h);
		if (h.kind === "busy") this.#offerAlternatives();
	}

	async #stop() {
		if (!this.session) return;
		const id = this.session.session_id;
		this.session = null;
		await this.#port.stopSession(id).catch(() => {});
	}

	/** The viewer leaves: nothing that is still starting may survive, and the display goes back. */
	async exit() {
		this.#clearHalt();
		this.#startId++;
		this.#cmd("restoreDisplay").catch(() => {});
		await this.leave();
	}

	async leave() {
		if (this.session)
			await this.#port.heartbeat(this.session.session_id, this.position, true).catch(() => {});
		await this.#stop();
		this.#cmd("stop").catch(() => {});
		this.#cmd("subtitle", {}).catch(() => {});
		this.#cmd("speed", { speed: 1 }).catch(() => {});
		this.#cmd("timing", { subtitleOffset: 0, subtitleScale: 1, audioDelay: 0 }).catch(() => {});
	}

	#finish() {
		if (this.ended) return;
		this.ended = true;
		this.#clearHalt();
		this.#startId++;
		this.setStats(false);
		this.buffering = false;
		this.leave();
		this.#ui.ended?.();
	}

	replay() {
		this.ended = false;
		this.#paused = false;
		this.position = 0;
		this.start(0);
	}

	skip() {
		const end = this.introOn ? this.tracks?.markers.intro?.[1] : undefined;
		if (end === undefined) return this.#finish();
		this.seekTo(end);
	}

	pickSpeed(x: number) {
		this.speed = x;
		this.#cmd("speed", { speed: x }).catch(() => {});
	}

	togglePlay() {
		const wasPlaying = this.playing;
		this.#paused = wasPlaying;
		this.#cmd(wasPlaying ? "pause" : "play");
	}

	seek(delta: number) {
		const target = Math.max(
			0,
			this.duration > 0
				? Math.min(this.duration - 1, this.position + delta)
				: this.position + delta,
		);
		this.seekTo(target);
	}

	seekTo(at: number) {
		this.position = at;
		this.#cmd("seek", { seconds: at });
		this.#watch();
	}

	setStats(on: boolean) {
		if (on === this.stats) return;
		this.stats = on;
		clearInterval(this.#statsTimer);
		this.#statsTimer = undefined;
		if (!on) return;
		const poll = async () => {
			this.live = await (this.#cmd("stats") as Promise<Stats>).catch(() => this.live);
			const id = this.session?.session_id;
			this.encodeSpeed =
				id && this.session?.mode !== "direct" && this.session?.mode !== "remux"
					? await this.#port.sessionInfo(id).then(
							(i) => i.speed,
							() => null,
						)
					: null;
		};
		poll();
		this.#statsTimer = setInterval(poll, STATS_MS);
	}

	dismissUpNext() {
		this.upNextDismissed = true;
	}

	retry() {
		return this.start();
	}

	async pickQuality(q: Quality) {
		if (q === this.quality || !this.session) return;
		const before = this.#currentCap();
		this.quality = q;
		const cap = this.#currentCap();
		if (cap !== before) await this.#repatch({ max_bitrate: cap && cap * MBIT });
	}

	// The stream keeps playing if the server can't convert right now: a stutter beats the busy screen.
	async #moveAuto(pick: number | null) {
		this.#stepping = true;
		const moved = await this.#repatch({ max_bitrate: pick && pick * MBIT }, true);
		if (moved) this.autoPick = pick;
		this.#betterSince = null;
		this.#stepping = false;
		return moved;
	}

	async #autoStepDown() {
		if (this.qSel !== "auto" || !this.session || this.#paused || this.halt || this.#stepping)
			return;
		const guess =
			this.session.video.action === "copy" && Date.now() - this.#stallSince >= STALL_GUESS_MS;
		const target = stepDown(this.#measuredInStall, this.session.bandwidth / MBIT, guess);
		if (target === null || !(await this.#moveAuto(target))) return;
		this.#stepUpHold = Math.min(this.#stepUpHold * 2, STEP_UP_HOLD_MAX_MS);
		toast(i18n.player_toast_quality_lowered({ value: target }), {
			icon: "gauge",
			warning: true,
			text: i18n.player_toast_quality_lowered_note(),
		});
	}

	#considerStepUp() {
		const target = autoCap(
			this.measured,
			this.preview && this.preview.bitRate / MBIT,
			this.originalPlays,
		);
		const session = this.session;
		const steady =
			this.qSel === "auto" && session && !this.#paused && !this.buffering && !this.halt;
		if (!steady || this.#bandwidthAt(target) <= session.bandwidth) {
			this.#betterSince = null;
			return;
		}
		this.#betterSince ??= Date.now();
		if (Date.now() - this.#betterSince >= this.#stepUpHold && !this.#stepping)
			this.#moveAuto(target);
	}

	// Presets above what a conversion aims for produce the same stream, so going there gains nothing.
	#bandwidthAt(cap: number | null) {
		const d = cap ? this.preview?.presets[cap] : this.preview?.original;
		return d && !d.error ? d.bandwidth : (cap ?? Infinity) * MBIT;
	}

	async #repatch(changes: Record<string, unknown>, quiet = false) {
		if (!this.session) return false;
		const at = this.position;
		this.buffering = true;
		try {
			const wasDirect = this.session.mode === "direct";
			this.session = await this.#port.patchSession(this.session.session_id, {
				...changes,
				position: at,
			});
			if (wasDirect !== (this.session.mode === "direct")) this.measured = null;
			await this.#cmd("load", { url: this.session.url, decode: decodeHere(), at });
			if (!this.#paused) await this.#cmd("play");
		} catch (e) {
			if (String(e).includes('"not_found"')) {
				await this.#recoverSession();
			} else if (quiet) {
				return false;
			} else {
				this.buffering = false;
				this.#failedToStart(String(e));
			}
		}
		this.#refreshPreview();
		if (!quiet) this.#ui.settled?.();
		return true;
	}
}
