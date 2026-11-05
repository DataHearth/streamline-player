<script lang="ts">
	import { orPoster } from "#lib/tv/api.ts";
	import { focusDefault as focusStageDefault, move } from "#lib/tv/focus.ts";
	import { toast } from "#lib/tv/toast.svelte.ts";
	import { addPluginListener, invoke } from "@tauri-apps/api/core";
	import {
		Activity,
		AudioLines,
		Captions,
		CaptionsOff,
		Check,
		ChevronDown,
		ChevronsRight,
		CircleAlert,
		Cpu,
		Folder,
		Gauge,
		LoaderCircle,
		Minus,
		Moon,
		Pause,
		Play,
		Plus,
		RotateCcw,
		RotateCw,
		Server,
		Settings,
		SkipForward,
		WifiOff,
		X,
	} from "@lucide/svelte";
	import { onMount, tick, type Component } from "svelte";

	type Next = { code: string; title: string; overview: string; still: string; stillPos: string; label: string; href: string };
	type More = { title: string; poster: string; href: string };
	let {
		mediaId,
		titleName,
		heading,
		eyebrow,
		frame,
		poster,
		framePos,
		start: startAt,
		next,
		more,
		onexit,
		onnext,
		onopen,
	}: {
		mediaId: string;
		titleName: string;
		heading: string;
		eyebrow: string;
		frame: string;
		poster: string;
		framePos: string;
		start: number;
		next: Next | null;
		more: More[];
		onexit: () => void;
		onnext: () => void;
		onopen: (href: string) => void;
	} = $props();

	type Reason = { code: string; text: string };
	type Session = {
		session_id: string;
		mode: "direct" | "remux" | "audio_transcode" | "transcode";
		url: string;
		video: { action: string; codec: string; width: number; height: number; hdr: string; dv_profile?: number };
		audio: { action: string; codec: string; channels: number };
		video_reasons: Reason[];
		audio_reasons: Reason[];
		hw: string;
	};
	type Quality = "auto" | "original" | number;
	type Tab = "subs" | "audio" | "quality" | "speed";
	type Stats = { version: string; buffer: number; throughput: number; videoBitrate: number; audioBitrate: number; rendered: number; dropped: number; displayHz: number };
	type AudioTrack = { index: number; codec: string; channels: number; language: string; default: boolean };
	type SubTrack = { index: number; codec: string; language: string; forced: boolean; sdh: boolean; files: Record<string, string> };
	type Tracks = { preparing: boolean; container: string; video: { codec: string; width: number; height: number; hdr?: string; dv?: unknown } | null; chapters: { start: number; end: number; title: string }[]; markers: { intro: [number, number] | null; credits: [number, number] | null }; audio: AudioTrack[]; subtitles: SubTrack[]; fonts: string[] };
	type StatRow = { k: string; v: string; c?: string; why?: boolean };
	type SubKind = "text" | "styled" | "image";
	type Cue = { start: number; end: number; text: string };
	type Tone = "ok" | "conv" | "warn";
	type Halt = {
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
		left?: number;
		total?: number;
	};
	type Flash = { ph: "idle" | "in" | "out"; amt: number; rot: number };

	const HIDE_MS = 3200;
	const HEARTBEAT_MS = 10_000;
	const SEEK_STEP = 10;
	const PLAYER_NAME = "ExoPlayer";
	const TAB_NAMES: Record<Tab, string> = { audio: "Audio", subs: "Subtitles", quality: "Quality", speed: "Speed" };
	const CODEC_NAMES: Record<string, string> = {
		h264: "H.264",
		hevc: "HEVC",
		av1: "AV1",
		vp9: "VP9",
		aac: "AAC",
		ac3: "AC3",
		eac3: "EAC3",
		dts: "DTS",
		truehd: "TrueHD",
		flac: "FLAC",
		opus: "Opus",
		mp3: "MP3",
		pcm_s16le: "PCM",
		pcm_s24le: "PCM",
	};
	const FMP4_AUDIO = ["aac", "mp3", "ac3", "eac3", "flac", "opus"];
	const SIZE_PX: Record<string, number> = { S: 20, M: 26, L: 32, XL: 40 };
	const DEFAULT_SIZE_PX = 26;
	const TV_CUE_SCALE = 1.7;
	const OSD_MS = 900;
	const UP_NEXT_SECS = 10;
	const SPEEDS = [0.5, 0.75, 1, 1.25, 1.5, 2];
	const STATS_MS = 1000;
	const SLOW_THROUGHPUT = 4_000_000;
	const SUB_OFFSET_LIMIT = 10;
	const AUDIO_DELAY_LIMIT = 500;
	const RECOVERY_WINDOW_MS = 30_000;
	const RETRY_SECS = 10;
	const OFFLINE_CHECK_SECS = 5;
	const MENU_EXIT_MS = 240;
	const SEEK_RUN_MS = 900;
	const MODE_LABEL = { direct: "Direct", remux: "Remux", audio_transcode: "Audio converted", transcode: "Converted" };

	let session = $state<Session | null>(null);
	// svelte-ignore state_referenced_locally
	let position = $state(startAt);
	let duration = $state(0);
	let playing = $state(false);
	let buffering = $state(true);
	let controls = $state(true);
	let menu = $state(false);
	let menuMounted = $state(false);
	let menuVis = $state(false);
	let host = $state("");
	let buffered = $state(0);
	let pfPause = $state(false);
	let tab = $state<Tab>("quality");
	let tracks = $state<Tracks | null>(null);
	let audioPick = $state<number | null>(null);
	let subPick = $state<number | null>(null);
	let burn = $state(false);
	let burnBack = $state("");
	let subOffset = $state(0);
	let audioDelay = $state(0);
	let cues = $state.raw<Cue[]>([]);
	let cueText = $state("");
	let styleOpen = $state(false);
	let prefs = $state(loadPrefs());
	let osd = $state<string | null>(null);
	let quality = $state<Quality>("original");
	let speed = $state(1);
	let stats = $state(false);
	let live = $state<Stats | null>(null);
	let ended = $state(false);
	let encodeSpeed = $state<number | null>(null);
	let upNextDismissed = $state(false);
	let subTouched = false;
	let halt = $state<Halt | null>(null);
	let skL = $state<Flash>({ ph: "idle", amt: 0, rot: 0 });
	let skR = $state<Flash>({ ph: "idle", amt: 0, rot: 0 });
	let pf = $state<"idle" | "in" | "out">("idle");

	let stage: HTMLElement;
	let capabilities: unknown = null;
	let lastMove = Date.now();
	let retryTimer: ReturnType<typeof setInterval> | undefined;
	let seekTimer: ReturnType<typeof setTimeout> | undefined;
	let startId = 0;
	let paused = false;
	let lastRecovery = 0;
	let pfTimer: ReturnType<typeof setTimeout> | undefined;
	let osdTimer: ReturnType<typeof setTimeout> | undefined;
	let positionAt = 0;
	let menuTimer: ReturnType<typeof setTimeout> | undefined;
	let seekLeft: boolean | null = null;
	let seekAt = 0;

	const introOn = $derived(!!tracks?.markers.intro && position >= tracks.markers.intro[0] && position < tracks.markers.intro[1]);
	// Without a marker the card shows for the very end only, so no episode ending gets cut.
	const creditsAt = $derived(tracks?.markers.credits?.[0] ?? (next && duration > 0 ? duration - UP_NEXT_SECS : null));
	const upNextOn = $derived(!!next && creditsAt !== null && position >= creditsAt && !upNextDismissed && !ended);
	const creditsOn = $derived(!!tracks?.markers.credits && !upNextOn && creditsAt !== null && position >= creditsAt && position < duration - 1);
	const skipOn = $derived((introOn || creditsOn) && !ended && !halt);
	const upNextFrac = $derived(creditsAt === null ? 0 : Math.min(1, Math.max(0, (position - creditsAt) / UP_NEXT_SECS)));
	const segs = $derived.by(() => {
		const list = tracks?.chapters ?? [];
		if (list.length < 2 || duration <= 0) return [{ flex: 1, fill: pct, buf: duration > 0 ? (buffered / duration) * 100 : 0 }];
		return list.map((c, i) => {
			const end = list[i + 1]?.start ?? duration;
			const len = Math.max(1, end - c.start);
			const within = (at: number) => Math.max(0, Math.min(100, ((at - c.start) / len) * 100));
			return { flex: len, fill: within(position), buf: within(buffered) };
		});
	});
	const chapterName = $derived(tracks?.chapters.findLast((c) => position >= c.start)?.title ?? "");
	const isEpisode = $derived(/ S\d+ E\d+$/.test(eyebrow));
	const controlsOn = $derived(controls || (!playing && !buffering) || menu);
	const cardBottom = $derived(controlsOn ? 220 : 50);
	const pct = $derived(duration > 0 ? (position / duration) * 100 : 0);
	const full = $derived(session?.mode === "direct" || session?.mode === "remux");
	const modeLabel = $derived.by(() => {
		if (!session) return "";
		const { height, hdr, dv_profile } = session.video;
		const range = dv_profile !== undefined ? " \u00B7 Dolby Vision" : hdr === "sdr" ? "" : " \u00B7 HDR";
		return `${height >= 2160 ? "4K" : `${height}p`}${range}`;
	});
	const modeBadge = $derived(session ? MODE_LABEL[session.mode].toLowerCase() : "");
	const modeTone = $derived<Tone>(full ? "ok" : "conv");
	const qualText = $derived(
		quality === "auto" ? "Auto" : quality === "original" ? "Original" : `${quality} Mbit/s`,
	);
	const menuNote = $derived.by(() => {
		if (!session) return "";
		const why = (r: Reason[]) => r.map((x) => x.text).join(", ");
		switch (session.mode) {
			case "direct":
				return "Playing the original file as-is. Nothing is converted.";
			case "remux":
				return `Original video and audio, repackaged for ${PLAYER_NAME} (${tracks?.container.toUpperCase() ?? ""} \u2192 fMP4). Nothing is converted.`;
			case "audio_transcode":
				return `Original video. The server converts the audio to ${audioOutName(session)} (${why(session.audio_reasons)}).`;
			case "transcode":
				return `The server converts the video to ${videoOutName(session)} (${why(session.video_reasons)}).`;
		}
	});

	const selectedSub = $derived(tracks?.subtitles.find((t) => t.index === subPick) ?? null);
	const selectedAudio = $derived(
		tracks?.audio.find((t) => t.index === audioPick) ?? tracks?.audio.find((t) => t.default) ?? tracks?.audio[0] ?? null,
	);
	const subKind = $derived<SubKind | "burn">(burn ? "burn" : selectedSub ? subInfo(selectedSub).kind : "text");
	const cueSize = $derived(Math.round((subKind === "text" || subKind === "styled" ? (SIZE_PX[prefs.subSize] ?? DEFAULT_SIZE_PX) : DEFAULT_SIZE_PX) * TV_CUE_SCALE));

	function loadPrefs() {
		const fallback = { subSize: "M", subBg: "Shadow", subColor: "White" };
		try {
			return { ...fallback, ...JSON.parse(localStorage.getItem("subtitle-style") ?? "{}") };
		} catch {
			return fallback;
		}
	}

	function setPref(key: "subSize" | "subBg" | "subColor", value: string) {
		prefs = { ...prefs, [key]: value };
		try {
			localStorage.setItem("subtitle-style", JSON.stringify(prefs));
		} catch {
			// The style still applies for this session.
		}
		syncTiming();
	}

	function language(code: string) {
		try {
			return new Intl.DisplayNames(["en"], { type: "language" }).of(code) ?? code;
		} catch {
			return code || "Unknown";
		}
	}

	function subInfo(t: SubTrack): { kind: SubKind; label: string; fmt: string; burnOnly?: boolean } {
		if (t.codec === "ass" || t.codec === "ssa") return { kind: "styled", label: "Styled (ASS)", fmt: "ASS" };
		if (t.codec === "hdmv_pgs_subtitle") return { kind: "image", label: "Image (PGS)", fmt: "PGS" };
		if (t.codec === "dvd_subtitle") return { kind: "image", label: "Image (VobSub)", fmt: "VobSub", burnOnly: true };
		return { kind: "text", label: t.codec === "webvtt" ? "Text (WebVTT)" : "Text (SRT)", fmt: "SRT" };
	}

	const subName = (t: SubTrack) => `${language(t.language)}${t.forced ? " (forced)" : ""}`;
	const channelLayout = (n: number) => (n >= 3 ? `${n - 1}.1` : `${n}.0`);
	const audioName = (t: AudioTrack) => `${CODEC_NAMES[t.codec] ?? t.codec} ${channelLayout(t.channels)}`;

	/** The engine's rule, for the tracks it hasn't been asked about yet. */
	function audioAsIs(t: AudioTrack) {
		const caps = capabilities as { audio?: { codecs: string[]; max_channels: number } } | null;
		if (!caps?.audio?.codecs.includes(t.codec) || t.channels > caps.audio.max_channels) return false;
		return session?.mode === "direct" || FMP4_AUDIO.includes(t.codec);
	}

	function audioTarget(t: AudioTrack) {
		const caps = capabilities as { audio?: { codecs: string[] } } | null;
		const has = (c: string) => caps?.audio?.codecs.includes(c);
		return t.channels > 2 && has("eac3") ? "EAC3 5.1" : t.channels > 2 && has("ac3") ? "AC3 5.1" : "AAC 2.0";
	}

	const audioNote = $derived.by(() => {
		if (!selectedAudio || !session) return "";
		const name = audioName(selectedAudio);
		return session.audio.action === "copy"
			? `${PLAYER_NAME} plays ${name} as-is.`
			: `${PLAYER_NAME} can\u2019t decode ${name}, so the server converts it to ${audioOutName(session)}.`;
	});

	const styleNote = $derived(
		subKind === "styled"
			? "Styled (ASS) subtitles keep the file\u2019s own fonts, colors and placement. Only Size applies, as a scale."
			: subKind === "image"
				? "Image subtitles are pictures, shown as they were made. Their look can\u2019t be changed."
				: subKind === "burn"
					? "Burned-in subtitles are part of the picture. Their look can\u2019t be changed."
					: "",
	);
	const styleSummary = $derived(
		subKind === "text"
			? `${prefs.subSize} \u00B7 ${prefs.subBg} \u00B7 ${prefs.subColor}`
			: subKind === "styled"
				? `File\u2019s own look \u00B7 ${prefs.subSize}`
				: "Drawn as authored",
	);
	const styleOk = (key: string) => subKind === "text" || (subKind === "styled" && key === "subSize");

	const fmtOffset = (v: number, seconds: boolean) => {
		const sign = v > 0 ? "+" : v < 0 ? "\u2212" : "";
		return seconds ? `${sign}${Math.abs(v).toFixed(1)} s` : `${sign}${Math.abs(v)} ms`;
	};

	function flashOsd(text: string) {
		clearTimeout(osdTimer);
		osd = text;
		osdTimer = setTimeout(() => (osd = null), OSD_MS);
	}

	function syncTiming() {
		const size = (SIZE_PX[prefs.subSize] ?? DEFAULT_SIZE_PX) / DEFAULT_SIZE_PX;
		player("timing", { subtitleOffset: subOffset, subtitleScale: size, audioDelay }).catch(() => {});
	}

	function nudgeSub(step: number) {
		subOffset = step === 0 ? 0 : Math.max(-SUB_OFFSET_LIMIT, Math.min(SUB_OFFSET_LIMIT, Math.round((subOffset + step) * 10) / 10));
		lastMove = Date.now();
		syncTiming();
		flashOsd(`Subtitles ${fmtOffset(subOffset, true)}`);
	}

	function nudgeAudio(step: number) {
		audioDelay = step === 0 ? 0 : Math.max(-AUDIO_DELAY_LIMIT, Math.min(AUDIO_DELAY_LIMIT, audioDelay + step));
		lastMove = Date.now();
		syncTiming();
		flashOsd(`Audio ${fmtOffset(audioDelay, false)}`);
	}

	const mbit = (bits: number) => `${(bits / 1_000_000).toFixed(bits < 1_000_000 ? 2 : 1)} Mbit/s`;
	const videoName = (codec: string, height: number) => `${CODEC_NAMES[codec] ?? codec.toUpperCase()} ${height}p`;
	const HDR_NAMES: Record<string, string> = { hdr10: "HDR10", hlg: "HLG" };
	const rangeName = (hdr?: string, dv?: boolean) => (dv ? " Dolby Vision" : hdr && HDR_NAMES[hdr] ? ` ${HDR_NAMES[hdr]}` : "");
	function videoOutName(s: Session) {
		const name = videoName(s.video.codec, s.video.height);
		if (s.video.action === "copy") return `${name}${rangeName(s.video.hdr, s.video.dv_profile !== undefined)}`;
		return `${name} ${HDR_NAMES[s.video.hdr] ?? "SDR"}`;
	}
	const originalSub = $derived(tracks?.video ? `${tracks.video.height}p ${CODEC_NAMES[tracks.video.codec] ?? tracks.video.codec}${rangeName(tracks.video.hdr, !!tracks.video.dv)}` : "");
	const audioOutName = (s: Session) => `${CODEC_NAMES[s.audio.codec] ?? s.audio.codec} ${channelLayout(s.audio.channels)}`;

	const statRows = $derived.by(() => {
		if (!session) return [];
		const copy = session.video.action === "copy";
		const direct = session.mode === "direct";
		const box = tracks?.container.toUpperCase() ?? "";
		const inName = tracks?.video ? `${videoName(tracks.video.codec, tracks.video.height)}${rangeName(tracks.video.hdr, !!tracks.video.dv)}` : "";
		const outName = videoOutName(session);
		const audioIn = selectedAudio ? audioName(selectedAudio) : "";
		const audioOut = audioOutName(session);
		const why = (list: Reason[]): StatRow[] => (list.length ? [{ k: "why", v: list.map((r) => r.text).join(" \u00B7 "), why: true }] : []);
		const known = [live && live.videoBitrate > 0 && `${mbit(live.videoBitrate)} video`, live && live.audioBitrate > 0 && `${mbit(live.audioBitrate)} audio`].filter(Boolean);
		const rows: StatRow[] = [
			{ k: "Mode", v: `${MODE_LABEL[session.mode]}${direct ? " \u00B7 the original file" : session.mode === "remux" ? " \u00B7 nothing converted" : ""}`, c: full ? "var(--status-available)" : "var(--status-downloading)" },
			{ k: "Player", v: `${PLAYER_NAME} \u00B7 Media3 ${live?.version ?? ""} \u00B7 Android TV` },
			{ k: "Video", v: copy ? `${outName} \u00B7 as-is` : `${inName} \u2192 ${outName}`, c: "var(--player-ink)" },
			...(copy ? [] : why(session.video_reasons)),
			{ k: "Audio", v: session.audio.action === "copy" ? `${audioIn} \u00B7 as-is` : `${audioIn} \u2192 ${audioOut}`, c: "var(--player-ink)" },
			...(session.audio.action === "copy" ? [] : why(session.audio_reasons)),
			{ k: "Container", v: direct ? `${box} \u00B7 the file itself` : `${box} \u2192 fMP4 \u00B7 HLS` },
			...(session.mode === "remux" ? [{ k: "why", v: `${box} not playable by ${PLAYER_NAME}`, why: true }] : []),
			...(encodeSpeed === null
				? []
				: [{ k: "Encode", v: `${encodeSpeed.toFixed(1)}\u00D7 realtime \u00B7 ${copy ? "audio only" : session.hw.toUpperCase()}`, c: encodeSpeed >= 1.2 ? "var(--status-available)" : encodeSpeed >= 1 ? "var(--status-warning)" : "var(--status-failed)" }]),
			{ k: "Subtitles", v: tracks?.preparing ? "Preparing\u2026" : selectedSub ? `${subName(selectedSub)} \u00B7 ${subInfo(selectedSub).fmt} \u00B7 ${burn ? "burned in by the server" : subRenderer(subInfo(selectedSub).kind)}` : "Off" },
			...(subOffset || audioDelay ? [{ k: "Offsets", v: `subtitles ${fmtOffset(subOffset, true)} \u00B7 audio ${fmtOffset(audioDelay, false)}` }] : []),
			...(known.length ? [{ k: "Bitrate", v: known.join(" \u00B7 ") }] : []),
		];
		if (live) {
			rows.push(
				{ k: "Throughput", v: mbit(live.throughput), c: live.throughput < SLOW_THROUGHPUT ? "var(--status-warning)" : undefined },
				{ k: "Buffer", v: `${live.buffer.toFixed(1)} s ahead` },
				{ k: "Frames", v: `${live.dropped.toLocaleString("en-US")} dropped / ${live.rendered.toLocaleString("en-US")}` },
				{ k: "Display", v: `${Math.round(live.displayHz * 1000) / 1000} Hz \u00B7 not matched` },
			);
		}
		rows.push({ k: "Speed", v: `${speed}\u00D7` }, { k: "Server", v: `${new URL(session.url).host} \u00B7 ${direct ? "/file" : "/hls"}` });
		return rows;
	});

	const subRenderer = (kind: SubKind) => (kind === "styled" ? "libass" : kind === "image" ? "Media3" : "web view");

	$effect(() => {
		if (!stats) return;
		const poll = async () => {
			live = await (player("stats") as Promise<Stats>).catch(() => live);
			const id = session?.session_id;
			encodeSpeed = id && session?.mode !== "direct" && session?.mode !== "remux" ? await invoke<{ speed: number | null }>("session_info", { id }).then((i) => i.speed, () => null) : null;
		};
		poll();
		const timer = setInterval(poll, STATS_MS);
		return () => clearInterval(timer);
	});

	const pop = (_: Element, { y = 6, scale = 0.98 } = {}) => ({
		duration: 220,
		css: (t: number) => `opacity:${t};transform:translateY(${(1 - t) * y}px) scale(${scale + (1 - scale) * t})`,
	});

	$effect(() => {
		if (next && upNextOn && creditsAt !== null && position >= creditsAt + UP_NEXT_SECS) leave().then(onnext);
	});

	// With the controls hidden, nothing else holds focus: OK must land on the card.
	$effect(() => {
		if ((skipOn || upNextOn) && !controlsOn) tick().then(() => stage?.querySelector<HTMLElement>(".sl-skip, .sl-upnext [data-tv-default]")?.focus());
	});

	const parseStamp = (t: string) => t.split(":").reduce((acc, part) => acc * 60 + Number(part.replace(",", ".")), 0);

	function parseCues(raw: string): Cue[] {
		const out: Cue[] = [];
		for (const block of raw.replace(/\r/g, "").split(/\n{2,}/)) {
			const lines = block.split("\n");
			const at = lines.findIndex((l) => l.includes("-->"));
			const [from, to] = lines[at]?.split("-->") ?? [];
			if (!from || !to) continue;
			const text = lines.slice(at + 1).join("\n").replace(/<[^>]*>/g, "").trim();
			if (text) out.push({ start: parseStamp(from.trim()), end: parseStamp(to.trim().split(/\s/)[0] ?? ""), text });
		}
		return out;
	}

	// ponytail: a linear scan on each tick, a few thousand cues at most; binary search if it ever shows.
	function updateCue() {
		if (!cues.length) {
			if (cueText) cueText = "";
			return;
		}
		const now = (playing ? position + ((performance.now() - positionAt) / 1000) * speed : position) - subOffset;
		const text = cues.filter((c) => c.start <= now && now < c.end).map((c) => c.text).join("\n");
		if (text !== cueText) cueText = text;
	}

	// A forced track translates the foreign passages of the audio it goes with. Never burned in on
	// its own: that would convert the video without the viewer asking.
	function autoForced() {
		if (subTouched || !tracks || tracks.preparing) return;
		const audio = selectedAudio && language(selectedAudio.language);
		const want = tracks.subtitles.find((t) => t.forced && !subInfo(t).burnOnly && language(t.language) === audio) ?? null;
		if ((want?.index ?? null) !== subPick) pickSub(want, false);
	}

	async function pickSub(t: SubTrack | null, byViewer = true) {
		subTouched ||= byViewer;
		const info = t && subInfo(t);
		const wantsBurn = !!info?.burnOnly;
		const changedBurn = wantsBurn !== burn || (wantsBurn && subPick !== t?.index);
		if (wantsBurn && !burn && session) burnBack = MODE_LABEL[session.mode];
		subPick = t?.index ?? null;
		burn = wantsBurn;
		cues = [];
		if (changedBurn) await repatch({ burn_in_subtitle: wantsBurn ? t?.index : null });
		try {
			if (!t || !info || wantsBurn) {
				await player("subtitle", {});
			} else if (info.kind === "text") {
				await player("subtitle", {});
				cues = parseCues(await invoke<string>("subtitle_text", { url: t.files.vtt }));
			} else if (info.kind === "styled") {
				await player("subtitle", { kind: "ass", url: t.files.ass, fonts: tracks?.fonts ?? [] });
			} else {
				await player("subtitle", { kind: "pgs", url: t.files.sup });
			}
			syncTiming();
		} catch {
			flashOsd("Subtitles unavailable");
		}
	}

	async function pickAudio(t: AudioTrack) {
		if (t.index === selectedAudio?.index) return;
		audioPick = t.index;
		await repatch({ audio_index: t.index });
		autoForced();
	}

	function sessionTracks() {
		return {
			max_bitrate: typeof quality === "number" ? quality * 1_000_000 : null,
			audio_index: audioPick,
			burn_in_subtitle: burn ? subPick : null,
		};
	}

	async function refreshTracks() {
		tracks = await invoke<Tracks>("media_tracks", { mediaId }).catch(() => tracks);
		autoForced();
	}

	const fmt = (s: number) => {
		const t = Math.max(0, Math.floor(s));
		const m = Math.floor(t / 60);
		const ss = String(t % 60).padStart(2, "0");
		return m >= 60 ? `${Math.floor(m / 60)}:${String(m % 60).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
	};
	const player = (cmd: string, args?: Record<string, unknown>) => invoke(`plugin:player|${cmd}`, args);

	function showControls(focus = false) {
		lastMove = Date.now();
		controls = true;
		if (focus) focusDefault();
	}

	async function focusDefault() {
		await tick();
		if (stage) focusStageDefault(stage);
	}

	function clearHalt() {
		clearInterval(retryTimer);
		halt = null;
	}

	function closeMenu() {
		menu = false;
		menuVis = false;
		clearTimeout(menuTimer);
		menuTimer = setTimeout(() => {
			if (!menu) menuMounted = false;
		}, MENU_EXIT_MS);
		focusDefault();
	}

	function showHalt(h: Halt) {
		if (menu) closeMenu();
		player("pause").catch(() => {});
		halt = h;
		focusDefault();
	}

	function haltWithRetry(h: Halt, secs: number) {
		showHalt({ ...h, left: secs, total: secs });
		clearInterval(retryTimer);
		retryTimer = setInterval(() => {
			if (!halt?.left) return;
			if (halt.left <= 1) start();
			else halt = { ...halt, left: halt.left - 1 };
		}, 1000);
	}

	function resumedToast(after: Halt["kind"] | undefined, s: Session) {
		if (after === "unreachable") toast("Reconnected \u00B7 playback resumed");
		else if (after === "offline") toast("Back online \u00B7 playback resumed", { icon: "wifi" });
		else if (after === "busy") toast("Conversion started", { icon: "cpu", text: s.video.action === "copy" ? `Audio \u2192 ${audioOutName(s)}` : `${s.video.height}p ${CODEC_NAMES[s.video.codec] ?? s.video.codec}` });
	}

	async function start(at = position) {
		const id = ++startId;
		const after = halt?.kind;
		clearHalt();
		stop();
		buffering = true;
		try {
			capabilities ??= await player("capabilities");
			refreshTracks();
			const started = await invoke<Session>("start_session", { mediaId, capabilities, position: at, tracks: sessionTracks() });
			// The viewer left while the server was still answering.
			if (id !== startId) {
				invoke("stop_session", { id: started.session_id }).catch(() => {});
				return;
			}
			session = started;
			await player("load", { url: session.url });
			if (at > 0) await player("seek", { seconds: at });
			if (!paused) await player("play");
			showControls(true);
			resumedToast(after, started);
		} catch (e) {
			if (id !== startId) return;
			buffering = false;
			failedToStart(String(e));
		}
	}

	// The session can vanish under a healthy player: a pause past the orphan timeout, an engine
	// restart, a network cut. The viewer only sees a spinner. Failing again right after a recovery
	// means something else is wrong, so that one is shown.
	async function recoverSession() {
		lastRecovery = Date.now();
		const at = position;
		await stop();
		await start(at);
		if (session && !halt) toast("Reconnected \u00B7 playback resumed");
	}

	const sessionGone = (raw: string) => raw.includes('"not_found"');

	function failedToStart(raw: string) {
		let error: { code?: string; message?: string; retry_after?: number } | null = null;
		try {
			error = JSON.parse(raw);
		} catch {
			// Not JSON: the request never got an answer from the server.
		}
		const at = fmt(position);
		if (error?.code === "transcode_limit") {
			haltWithRetry(
				{
					kind: "busy",
					eyebrow: "Server busy",
					icon: Cpu,
					title: "Waiting for a free slot",
					body: `All conversions on ${host} are in use, and this stream needs one.`,
					count: (left) => `Trying again in ${left} s`,
					note: `You\u2019re at ${at}. Playback starts as soon as a slot frees up.`,
					failed: false,
					retryLabel: "Try now",
				},
				error.retry_after ?? RETRY_SECS,
			);
		} else if (error?.code === "file_not_found" || error?.code === "media_not_found") {
			showHalt({
				kind: "missing",
				eyebrow: "File missing",
				icon: Folder,
				title: "This file isn\u2019t on the server",
				body: "Streamline can\u2019t find it where the library expects it. It may have been moved, renamed or deleted.",
				mono: error.message,
				note: "Let your admin know. If it was moved, a library scan finds it again.",
				failed: true,
			});
		} else if (error?.code === "transcode_unavailable" || error?.code === "ffmpeg_unavailable") {
			showHalt({
				kind: "nocodec",
				eyebrow: "Can\u2019t play here",
				icon: CircleAlert,
				title: `${titleName} can\u2019t play on this device`,
				body: `${PLAYER_NAME} can\u2019t decode this file\u2019s video, and the server can\u2019t convert it: ${error.message}.`,
				note: "Play it on another device, or ask your admin to turn on software conversion.",
				failed: true,
			});
		} else if (error) {
			showHalt({
				kind: "error",
				eyebrow: "Playback error",
				icon: CircleAlert,
				title: "This can\u2019t play right now",
				mono: error.message ?? error.code ?? raw,
				failed: true,
				retryLabel: "Try now",
			});
		} else if (navigator.onLine === false) {
			haltWithRetry(
				{
					kind: "offline",
					eyebrow: "Offline",
					icon: WifiOff,
					title: "This device lost its connection",
					body: `Playback picks up at ${at} as soon as it\u2019s back.`,
					wait: "Waiting for a network \u00B7 checking every 5 s",
					failed: false,
					retryLabel: "Try again",
				},
				OFFLINE_CHECK_SECS,
			);
		} else {
			haltWithRetry(
				{
					kind: "unreachable",
					eyebrow: "Can\u2019t reach the server",
					icon: Server,
					title: `${host} isn\u2019t answering`,
					body: "The server may be off or restarting, or this device is away from your home network without the VPN.",
					count: (left) => `Trying again in ${left} s \u00B7 your place is saved at ${at}`,
					failed: true,
					retryLabel: "Try now",
				},
				RETRY_SECS,
			);
		}
	}

	async function stop() {
		if (!session) return;
		const id = session.session_id;
		session = null;
		await invoke("stop_session", { id }).catch(() => {});
	}

	async function exit() {
		clearHalt();
		startId++;
		await leave();
		onexit();
	}

	async function leave() {
		if (session) await invoke("heartbeat", { id: session.session_id, position, paused: true }).catch(() => {});
		await stop();
		player("stop").catch(() => {});
		player("subtitle", {}).catch(() => {});
		player("speed", { speed: 1 }).catch(() => {});
		player("timing", { subtitleOffset: 0, subtitleScale: 1, audioDelay: 0 }).catch(() => {});
	}

	function finish() {
		if (ended) return;
		ended = true;
		clearHalt();
		startId++;
		if (menu) closeMenu();
		stats = false;
		buffering = false;
		leave();
		focusDefault();
	}

	function replay() {
		ended = false;
		paused = false;
		position = 0;
		start(0);
	}

	function skip() {
		const end = introOn ? tracks?.markers.intro?.[1] : undefined;
		if (end === undefined) return finish();
		position = end;
		player("seek", { seconds: end });
	}

	function pickSpeed(x: number) {
		speed = x;
		player("speed", { speed: x }).catch(() => {});
	}

	function togglePlay() {
		const wasPlaying = playing;
		paused = wasPlaying;
		player(wasPlaying ? "pause" : "play");
		pfPause = wasPlaying;
		clearTimeout(pfTimer);
		pf = "in";
		pfTimer = setTimeout(() => {
			pf = "out";
			pfTimer = setTimeout(() => (pf = "idle"), 220);
		}, 520);
	}

	function seek(delta: number) {
		const target = Math.max(0, duration > 0 ? Math.min(duration - 1, position + delta) : position + delta);
		position = target;
		player("seek", { seconds: target });
		const left = delta < 0;
		const cur = left ? skL : skR;
		const now = Date.now();
		const run = seekLeft === left && now - seekAt < SEEK_RUN_MS && cur.ph !== "idle";
		seekLeft = left;
		seekAt = now;
		const flash: Flash = { ph: "in", amt: (run ? cur.amt : 0) + Math.abs(delta), rot: (run ? cur.rot : 0) + (left ? -360 : 360) };
		if (left) {
			skL = flash;
			if (skR.ph === "in") skR = { ...skR, ph: "out" };
		} else {
			skR = flash;
			if (skL.ph === "in") skL = { ...skL, ph: "out" };
		}
		clearTimeout(seekTimer);
		seekTimer = setTimeout(() => {
			if (left) skL = { ...skL, ph: "out" };
			else skR = { ...skR, ph: "out" };
			seekTimer = setTimeout(() => {
				skL = { ...skL, ph: "idle", rot: 0 };
				skR = { ...skR, ph: "idle", rot: 0 };
			}, 220);
		}, 720);
	}

	async function pickQuality(q: Quality) {
		if (q === quality || !session) return;
		quality = q;
		await repatch({ max_bitrate: typeof q === "number" ? q * 1_000_000 : null });
	}

	async function repatch(changes: Record<string, unknown>) {
		if (!session) return;
		const at = position;
		buffering = true;
		try {
			session = await invoke<Session>("patch_session", {
				id: session.session_id,
				changes: { ...changes, position: at },
			});
			await player("load", { url: session.url });
			await player("seek", { seconds: at });
			if (!paused) await player("play");
		} catch (e) {
			if (sessionGone(String(e))) {
				await recoverSession();
			} else {
				buffering = false;
				failedToStart(String(e));
			}
		}
		if (!menu) focusDefault();
	}

	function openMenu(next: Tab) {
		tab = next;
		menu = true;
		menuMounted = true;
		clearTimeout(menuTimer);
		requestAnimationFrame(() =>
			requestAnimationFrame(() => {
				if (menu) menuVis = true;
			}),
		);
		refreshTracks();
		focusDefault();
	}

	function back() {
		if (halt || ended) return exit();
		if (menu) return closeMenu();
		if (stats) {
			stats = false;
			return focusDefault();
		}
		if (controls) {
			controls = false;
			return;
		}
		exit();
	}

	function onKey(e: KeyboardEvent) {
		const k = e.key;
		if (k === "Escape" || k === "Backspace" || k === "GoBack") {
			e.preventDefault();
			back();
			return;
		}
		const arrow = k.startsWith("Arrow");
		if (!arrow && k !== "Enter") return;
		if (halt || menu || ended) {
			if (arrow) {
				e.preventDefault();
				move(k, stage);
			}
			return;
		}
		if (!controlsOn) {
			const onCard = (document.activeElement as HTMLElement | null)?.closest(".sl-skip, .sl-upnext");
			if (onCard && k === "Enter") return;
			if (onCard && upNextOn && (k === "ArrowLeft" || k === "ArrowRight")) {
				e.preventDefault();
				move(k, stage);
				return;
			}
			e.preventDefault();
			if (k === "ArrowLeft") seek(-SEEK_STEP);
			else if (k === "ArrowRight") seek(SEEK_STEP);
			else if (k === "Enter") {
				togglePlay();
				showControls(true);
			} else showControls(true);
			return;
		}
		lastMove = Date.now();
		const scrub = (document.activeElement as HTMLElement | null)?.hasAttribute("data-scrub");
		if (scrub && (k === "ArrowLeft" || k === "ArrowRight")) {
			e.preventDefault();
			seek(k === "ArrowLeft" ? -SEEK_STEP : SEEK_STEP);
		} else if (scrub && k === "Enter") {
			e.preventDefault();
			togglePlay();
		} else if (scrub && k === "ArrowDown") {
			e.preventDefault();
			focusDefault();
		} else if (arrow) {
			e.preventDefault();
			move(k, stage);
		} else if (!document.activeElement?.hasAttribute("data-tv")) {
			e.preventDefault();
			focusDefault();
		}
	}

	onMount(() => {
		invoke<string>("server_host").then((h) => (host = h));
		const listeners = [
			addPluginListener<{ position: number; buffered: number; duration: number }>("player", "position", (e) => {
				position = e.position;
				buffered = e.buffered;
				positionAt = performance.now();
				duration = e.duration;
			}),
			addPluginListener<{ state: string }>("player", "state", (e) => {
				buffering = e.state === "buffering";
				playing = e.state === "playing";
				if (e.state === "ended") {
					if (next) leave().then(onnext);
					else finish();
				}
			}),
			addPluginListener<{ code: string; message: string }>("player", "error", (e) => {
				if (e.code.startsWith("ERROR_CODE_IO_") && Date.now() - lastRecovery > RECOVERY_WINDOW_MS) {
					recoverSession();
					return;
				}
				buffering = false;
				showHalt({ kind: "error", eyebrow: "Playback error", icon: CircleAlert, title: "This stream stopped", mono: `${e.code}: ${e.message}`, failed: true, retryLabel: "Try now" });
			}),
			addPluginListener("player", "back", back),
			addPluginListener("player", "subtitleFailed", () => flashOsd("Subtitles unavailable")),
		];
		start(startAt);
		const cueTimer = setInterval(updateCue, 100);
		const beat = setInterval(() => {
			if (session) invoke("heartbeat", { id: session.session_id, position, paused: !playing }).catch(() => {});
		}, HEARTBEAT_MS);
		const hide = setInterval(() => {
			if (controls && playing && !menu && Date.now() - lastMove > HIDE_MS) controls = false;
		}, 250);
		return () => {
			clearInterval(cueTimer);
			clearInterval(beat);
			clearInterval(hide);
			clearInterval(retryTimer);
			clearTimeout(menuTimer);
			listeners.forEach((l) => l.then((h) => h.unregister()));
			startId++;
			leave();
		};
	});
</script>

<svelte:window onkeydown={onKey} />

{#snippet flash(f: Flash, side: "left" | "right")}
	<div class="sk {side}" aria-hidden="true">
		<div class="sk-disc {f.ph}" style:--dir={side === "left" ? -14 : 14}>
			<span class="sk-ico" style:transform="rotate({f.rot}deg)" style:transition={f.ph === "idle" ? "none" : "transform 450ms var(--ease)"}>
				{#if side === "left"}<RotateCcw size={46} strokeWidth={1.5} />{:else}<RotateCw size={46} strokeWidth={1.5} />{/if}
			</span>
			<span class="sk-amt">{f.amt}</span>
		</div>
	</div>
{/snippet}

{#snippet option(label: string, selected: boolean, onclick: () => void, sub = "", badge = "", tone: Tone = "ok", busy = false, pill = "")}
	<button data-tv="1" data-tv-default={selected ? "1" : undefined} class="opt" class:sel={selected} disabled={busy} style:opacity={busy ? 0.62 : 1} {onclick}>
		<span class="opt-check" style:opacity={selected ? 1 : 0}><Check size={16} strokeWidth={2.5} /></span>
		<span class="opt-text">
			<span class="opt-label" style:font-weight={selected ? 600 : 500}>{label}{#if pill}<span class="opt-pill">{pill}</span>{/if}</span>
			{#if sub}<span class="opt-sub">{#if busy}<span class="opt-spin"><LoaderCircle size={11} strokeWidth={2} /></span>{/if}{sub}</span>{/if}
		</span>
		{#if badge}<span class="badge {tone}">{badge}</span>{/if}
	</button>
{/snippet}

{#snippet stepper(label: string, desc: string, value: string, zero: boolean, what: string, dec: () => void, inc: () => void, reset: () => void, disabled = false)}
	<div class="stepper" style:opacity={disabled ? 0.5 : 1}>
		<span class="stepper-text">
			<span class="stepper-label">{label}</span>
			{#if !zero}<span class="stepper-desc">{desc}</span>{/if}
		</span>
		<div class="stepper-box" style:pointer-events={disabled ? "none" : "auto"}>
			<button data-tv="1" disabled={disabled} class="step" aria-label="{what} earlier" onclick={dec}><Minus size={14} strokeWidth={2.25} /></button>
			<span class="stepper-value" style:color={zero ? "var(--player-ink-muted)" : "var(--player-accent-text)"}>{value}</span>
			<button data-tv="1" disabled={disabled} class="step" aria-label="{what} later" onclick={inc}><Plus size={14} strokeWidth={2.25} /></button>
		</div>
		<button data-tv="1" disabled={zero} class="reset" style:opacity={zero ? 0 : 1} aria-label="Reset {what.toLowerCase()} timing" onclick={reset}>Reset</button>
	</div>
{/snippet}

{#snippet styleGroup(label: string, key: "subSize" | "subBg" | "subColor", values: string[])}
	{@const on = styleOk(key)}
	{@const idx = values.indexOf(prefs[key])}
	<div class="style-row" style:opacity={on ? 1 : 0.42}>
		<span class="style-label">{label}</span>
		<div class="seg-box" style:pointer-events={on ? "auto" : "none"}>
			<span
				class="seg-ind"
				aria-hidden="true"
				style:width="calc((100% - {6 + (values.length - 1) * 4}px) / {values.length})"
				style:transform="translateX(calc({Math.max(0, idx)} * (100% + 4px)))"
				style:background={on ? "var(--player-accent)" : "var(--surface-2)"}
				style:opacity={idx < 0 ? 0 : 1}
			></span>
			{#each values as v}
				<button data-tv="1" disabled={!on} class="seg-opt" style:color={prefs[key] === v && on ? "var(--fg-on-accent)" : "var(--player-ink-muted)"} onclick={() => setPref(key, v)}>{v}</button>
			{/each}
		</div>
	</div>
{/snippet}

<div class="stage" bind:this={stage}>
	<div class="scrim" style:opacity={controlsOn ? 1 : 0}></div>

	{#if buffering}<div class="sl-spin" role="status" aria-label="Buffering"></div>{/if}

	{#if cueText}
		<div class="cue-wrap" style:bottom="{controlsOn ? 400 : 90}px">
			<span
				class="sl-cue"
				style:font-size="{cueSize}px"
				style:color={prefs.subColor === "Yellow" ? "#FDE68A" : "#FFFFFF"}
				style:text-shadow={prefs.subBg === "Shadow" ? "0 2px 4px rgba(0,0,0,0.9), 0 0 12px rgba(0,0,0,0.6)" : prefs.subBg === "None" ? "0 0 2px rgba(0,0,0,0.9)" : "none"}
				style:background={prefs.subBg === "Box" ? "rgba(0,0,0,0.72)" : "transparent"}
				style:padding={prefs.subBg === "Box" ? "4px 12px" : "0"}>{cueText}</span>
		</div>
	{/if}

	{#if osd}
		<div class="osd-wrap"><div class="sl-osd">{osd}</div></div>
	{/if}

	<div class="pf" aria-hidden="true">
		<div class="pf-disc {pf}">
			{#if pfPause}<Pause size={36} fill="currentColor" />{:else}<Play size={36} fill="currentColor" />{/if}
		</div>
	</div>
	{@render flash(skL, "left")}
	{@render flash(skR, "right")}

	<div class="chrome" style:opacity={controlsOn ? 1 : 0}>
		{#if controlsOn && !ended}
			<div class="controls">
				<div class="head">
					<span class="tv-heading"><span class="tv-eyebrow">{eyebrow}</span><span class="tv-title">{heading}</span></span>
					<span class="mode">{modeLabel}</span>
				</div>
				<button data-tv="1" data-scrub="1" class="scrub" aria-label="Seek">
					<div class="track">{#each segs as g}<div class="seg" style:flex={g.flex}><div class="buf" style:width="{g.buf}%"></div><div class="fill" style:width="{g.fill}%"></div></div>{/each}</div>
					<div class="thumb" style:left="{pct}%"></div>
				</button>
				<div class="times"><span class="cur">{fmt(position)}</span><span>{chapterName}</span><span>-{fmt(duration - position)}</span></div>
				<div class="row">
					<button data-tv="1" data-tv-default="1" class="play" aria-label={playing ? "Pause" : "Play"} onclick={togglePlay}>
						{#if playing}<Pause size={36} fill="currentColor" />{:else}<Play size={36} fill="currentColor" />{/if}
					</button>
					<button data-tv="1" class="pill" onclick={() => openMenu("subs")}>
						{#if selectedSub}<Captions size={28} strokeWidth={1.9} />{subName(selectedSub)}{:else}<CaptionsOff size={28} strokeWidth={1.9} />Subtitles{/if}
					</button>
					<button data-tv="1" class="pill" onclick={() => openMenu("audio")}><AudioLines size={28} strokeWidth={1.9} />Audio</button>
					<button data-tv="1" class="pill" onclick={() => openMenu("quality")}><Settings size={28} strokeWidth={1.9} />{qualText}</button>
					<button data-tv="1" class="pill" onclick={() => openMenu("speed")}><Gauge size={28} strokeWidth={1.9} />{speed}&times;</button>
					<button data-tv="1" class="pill" onclick={() => (stats = !stats)}><Activity size={28} strokeWidth={1.9} />Stats</button>
					{#if next}<button data-tv="1" class="pill" onclick={() => leave().then(onnext)}><SkipForward size={28} strokeWidth={1.9} />Next episode</button>{/if}
				</div>
			</div>
		{/if}
	</div>

	{#if menuMounted}
		<div class="menu-scrim" class:vis={menuVis}></div>
		<div class="menu" class:vis={menuVis} data-tv-trap={menu ? "1" : undefined} inert={!menu}>
			<div class="menu-head">
				<span class="menu-title">{TAB_NAMES[tab]}</span>
				<div class="grow"></div>
				<button data-tv="1" class="close" aria-label="Close" onclick={closeMenu}><X size={16} strokeWidth={2} /></button>
			</div>
			<div class="menu-body">
				{#if tab === "quality"}
					<div class="note"><span class="note-dot" style:background={full ? "var(--status-available)" : "var(--status-downloading)"}></span><span>{menuNote}</span></div>
					{@render option("Auto", quality === "auto", () => pickQuality("auto"), "Now Original", quality === "auto" ? modeBadge : "", modeTone)}
					{@render option("Original", quality === "original", () => pickQuality("original"), originalSub, quality === "original" ? modeBadge : "", modeTone)}
					{#each [20, 8, 4, 2, 1] as n}
						{@const on = quality === n}
						{@const same = on && session?.video.action === "copy"}
						{@render option(`${n} Mbit/s`, on, () => pickQuality(n), same ? "Same as Original" : on && session ? `${session.video.height}p ${CODEC_NAMES[session.video.codec] ?? session.video.codec}` : "", on ? modeBadge : "", modeTone, false, on && !same && session?.video.hdr !== "sdr" ? "HDR" : "")}
					{/each}
				{:else if tab === "speed"}
					{#each SPEEDS as x}{@render option(x === 1 ? "Normal" : `${x}\u00D7`, speed === x, () => pickSpeed(x))}{/each}
				{:else if tab === "audio"}
					<div class="note"><span class="note-dot" style:background={session?.audio.action === "copy" ? "var(--status-available)" : "var(--status-downloading)"}></span><span>{audioNote}</span></div>
					{#each tracks?.audio ?? [] as t, i}
						{@const asIs = t.index === selectedAudio?.index ? session?.audio.action === "copy" : audioAsIs(t)}
						{@render option(
							`${language(t.language)}${i === 0 ? " (original)" : ""}`,
							t.index === selectedAudio?.index,
							() => pickAudio(t),
							asIs ? audioName(t) : `${audioName(t)} \u2192 ${t.index === selectedAudio?.index && session ? (CODEC_NAMES[session.audio.codec] ?? session.audio.codec) : audioTarget(t)}`,
							asIs ? "as-is" : "converted",
							asIs ? "ok" : "conv",
						)}
					{/each}
					{@render stepper(
						"Audio delay",
						audioDelay > 0 ? "Sound plays later than the picture." : "Sound plays earlier than the picture.",
						fmtOffset(audioDelay, false),
						audioDelay === 0,
						"Audio",
						() => nudgeAudio(-10),
						() => nudgeAudio(10),
						() => nudgeAudio(0),
					)}
					<div class="night">
						<Moon size={16} strokeWidth={1.75} />
						<span class="night-text"><span class="night-title">Night mode</span><span>Evens out loud and quiet scenes</span></span>
						<span class="night-later">Later</span>
					</div>
				{:else}
					{#if tracks?.preparing}
						<div class="note"><span class="note-dot" style:background="var(--status-downloading)"></span><span>This file was just added. Streamline is still preparing its subtitles; they show up here in about a minute.</span></div>
					{:else if burn && selectedSub}
						<div class="note"><span class="note-dot" style:background="var(--status-warning)"></span><span>{PLAYER_NAME} can&rsquo;t draw {subInfo(selectedSub).fmt} subtitles, so the server burns {subName(selectedSub)} into the picture. That converts the video; any other track goes back to {burnBack}.</span></div>
					{/if}
					{@render option("Off", subPick === null, () => pickSub(null))}
					{#each tracks?.subtitles ?? [] as t}
						{@const info = subInfo(t)}
						{#if tracks?.preparing && !info.burnOnly}
							{@render option(subName(t), false, () => {}, "Preparing\u2026", "", "ok", true)}
						{:else if info.burnOnly}
							{@render option(subName(t), subPick === t.index, () => pickSub(t), `${info.label} \u00B7 burn in`, "converts the video", "warn")}
						{:else}
							{@render option(subName(t), subPick === t.index, () => pickSub(t), `${info.label}${t.sdh ? " \u00B7 sound descriptions" : ""}`)}
						{/if}
					{/each}
					{@render stepper(
						"Sync",
						subOffset > 0 ? "Subtitles show later than in the file." : "Subtitles show earlier than in the file.",
						fmtOffset(subOffset, true),
						subOffset === 0,
						"Subtitles",
						() => nudgeSub(-0.1),
						() => nudgeSub(0.1),
						() => nudgeSub(0),
						subPick === null || !!tracks?.preparing,
					)}
					<div class="style-box">
						<button data-tv="1" class="style-head" aria-expanded={styleOpen} onclick={() => (styleOpen = !styleOpen)}>
							<span class="style-eyebrow">Subtitle style</span>
							<span class="style-summary">{styleSummary}</span>
							<span class="style-chevron" style:transform={styleOpen ? "rotate(180deg)" : "none"}><ChevronDown size={15} strokeWidth={2} /></span>
						</button>
						{#if styleOpen}
							{#if styleNote}<div class="style-note">{styleNote}</div>{/if}
							{@render styleGroup("Size", "subSize", ["S", "M", "L", "XL"])}
							{@render styleGroup("Background", "subBg", ["None", "Shadow", "Box"])}
							{@render styleGroup("Color", "subColor", ["White", "Yellow"])}
						{/if}
					</div>
				{/if}
			</div>
		</div>
	{/if}

	{#if skipOn}
		<div class="card-slot skip-slot" style:bottom="{cardBottom}px" transition:pop={{ y: 10, scale: 0.96 }}>
			<button data-tv="1" class="sl-skip" onclick={skip}><ChevronsRight size={18} strokeWidth={2} />{introOn ? "Skip intro" : "Skip credits"}</button>
		</div>
	{/if}

	{#if upNextOn && next}
		<div class="card-slot sl-upnext" style:bottom="{cardBottom}px">
			<div class="upnext-art">
				<img src={next.still} alt="" style:object-position={next.stillPos} onerror={orPoster(poster)} />
				<div class="upnext-fade"></div>
				<div class="upnext-cap">
					<span class="upnext-code">UP NEXT &middot; {next.code}</span>
					<span class="upnext-title">{next.title}</span>
				</div>
				<div class="upnext-timer" role="progressbar" aria-label="Next episode starts when this fills" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(upNextFrac * 100)}>
					<div style:transform="scaleX({upNextFrac})"></div>
				</div>
			</div>
			<div class="upnext-row">
				<button data-tv="1" data-tv-default="1" class="btn-ink" onclick={() => leave().then(onnext)}><Play size={15} fill="currentColor" /><span>Play now</span></button>
				<button data-tv="1" class="btn-outline" onclick={() => { upNextDismissed = true; focusDefault(); }}>Watch credits</button>
			</div>
		</div>
	{/if}

	{#if stats}
		<div class="sl-stats" style:bottom="{controlsOn ? 220 : 50}px" transition:pop>
			<div class="stats-head">
				<span>Playback stats</span>
				<button data-tv="1" class="stats-close" aria-label="Close stats" onclick={() => { stats = false; focusDefault(); }}><X size={14} strokeWidth={2} /></button>
			</div>
			{#each statRows as r}
				<div class="stats-row">
					<span>{r.k}</span><span class:why={r.why} style:color={r.why ? "var(--fg-subtle)" : (r.c ?? "var(--player-ink-muted)")}>{r.v}</span>
				</div>
			{/each}
		</div>
	{/if}

	{#if ended}
		<div class="end" data-tv-trap="1">
			<img class="frame" src={frame} alt="" style:object-position={framePos} onerror={orPoster(poster)} />
			<div class="halt-scrim end-scrim"></div>
			<div class="halt-zoom">
				{#if next}
					<div class="end-next">
						<div class="halt-col end-col">
							<span class="eyebrow" style:color="var(--player-accent-text)">Up next &middot; {next.code}</span>
							<span class="halt-title">{next.title}</span>
							{#if next.overview}<span class="end-syn">{next.overview}</span>{/if}
							<div class="actions">
								<button data-tv="1" data-tv-default="1" class="primary" onclick={onnext}><Play size={16} fill="currentColor" />{next.label}</button>
								<button data-tv="1" class="secondary" onclick={exit}>Done</button>
							</div>
						</div>
						<div class="end-still"><img src={next.still} alt="" style:object-position={next.stillPos} onerror={orPoster(poster)} /></div>
					</div>
				{:else}
					<div class="end-done-col">
						<div class="halt-col end-col">
							<span class="eyebrow" style:color="var(--player-accent-text)">{isEpisode ? `Finished \u00B7 ${eyebrow.split(" \u00B7 ").at(-1)}` : "Finished"}</span>
							<span class="halt-title">{isEpisode ? eyebrow.split(" \u00B7 ")[0] : heading}</span>
							<div class="actions">
								<button data-tv="1" data-tv-default="1" class="primary end-done" onclick={exit}>Done</button>
								<button data-tv="1" class="secondary end-again" onclick={replay}><RotateCcw size={16} strokeWidth={2} />Watch again</button>
							</div>
						</div>
						{#if more.length}
							<div class="end-more">
								<span class="more-title">More like this</span>
								<div class="more-row">
									{#each more as it}
										<button data-tv="1" class="more-card" aria-label={it.title} onclick={() => onopen(it.href)}><img src={it.poster} alt="" loading="lazy" /></button>
									{/each}
								</div>
							</div>
						{/if}
					</div>
				{/if}
			</div>
		</div>
	{/if}

	{#if halt}
		<div class="halt" role="alertdialog" aria-label={halt.eyebrow} data-tv-trap="1">
			<img class="frame" src={frame} alt="" style:object-position={framePos} onerror={orPoster(poster)} />
			<div class="halt-scrim"></div>
			<div class="halt-zoom">
				<div class="halt-col">
					<span class="eyebrow" style:color={halt.failed ? "var(--status-failed)" : "var(--status-warning)"}>
						<halt.icon size={14} strokeWidth={2} />
						{halt.eyebrow}
					</span>
					<span class="halt-title">{halt.title}</span>
					{#if halt.body}<span class="halt-body">{halt.body}</span>{/if}
					{#if halt.mono}<span class="halt-mono">{halt.mono}</span>{/if}
					{#if halt.count && halt.left && halt.total}
						<div class="count">
							<div class="count-track"><div class="count-fill" style:width="{(halt.left / halt.total) * 100}%" style:background={halt.failed ? "var(--player-ink-muted)" : "var(--status-warning)"}></div></div>
							<span class="count-text">{halt.count(halt.left)}</span>
						</div>
					{/if}
					{#if halt.wait}<span class="halt-wait"><span class="wait-dot"></span>{halt.wait}</span>{/if}
					{#if halt.note}<span class="halt-note">{halt.note}</span>{/if}
					<div class="actions">
						{#if halt.retryLabel}<button data-tv="1" data-tv-default="1" class="primary" onclick={() => start()}><RotateCw size={16} strokeWidth={2} />{halt.retryLabel}</button>{/if}
						<button data-tv="1" data-tv-default={halt.retryLabel ? undefined : "1"} class="secondary" onclick={exit}>Back to {titleName}</button>
					</div>
				</div>
			</div>
		</div>
	{/if}
</div>

<style>
	.stage {
		position: fixed;
		left: 0;
		top: 0;
		width: 1920px;
		height: 1080px;
		zoom: var(--tv-zoom, 1);
		overflow: hidden;
		user-select: none;
	}

	.scrim {
		position: absolute;
		inset: 0;
		pointer-events: none;
		background: linear-gradient(180deg, rgba(0, 0, 0, 0.62) 0%, rgba(0, 0, 0, 0) 22%, rgba(0, 0, 0, 0) 62%, var(--player-scrim) 100%);
		transition: opacity var(--dur-slow);
	}

	.sl-spin {
		position: absolute;
		left: 50%;
		top: 50%;
		width: 72px;
		height: 72px;
		margin: -36px 0 0 -36px;
		border: 3px solid rgba(255, 255, 255, 0.18);
		border-top-color: var(--player-ink);
		border-radius: 50%;
		animation: sl-spinr 800ms linear infinite;
		pointer-events: none;
	}

	@keyframes sl-spinr {
		to {
			transform: rotate(360deg);
		}
	}

	.pf {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		pointer-events: none;
	}

	.pf-disc,
	.sk-disc {
		zoom: 1.75;
		width: 92px;
		height: 92px;
		display: grid;
		place-items: center;
		border-radius: 50%;
		background: rgba(0, 0, 0, 0.6);
		backdrop-filter: blur(12px);
		color: var(--player-ink);
		opacity: 0;
		transform: scale(0.85);
	}

	.pf-disc.in,
	.sk-disc.in {
		opacity: 1;
		transform: none;
		transition:
			opacity 120ms var(--ease),
			transform var(--dur-base) var(--ease);
	}

	.pf-disc.out,
	.sk-disc.out {
		transition:
			opacity var(--dur-exit) var(--ease-in),
			transform var(--dur-exit) var(--ease-in);
	}

	.pf-disc.out {
		transform: scale(1.15);
	}

	.sk-disc.out {
		transform: translateX(calc(var(--dir) * 1px));
	}

	.sk {
		position: absolute;
		top: 0;
		bottom: 0;
		width: 100%;
		display: grid;
		place-items: center;
		pointer-events: none;
	}

	.sk.left {
		left: 0;
	}

	.sk.right {
		right: 0;
	}

	.sk-ico {
		grid-area: 1 / 1;
		display: grid;
	}

	.sk-amt {
		grid-area: 1 / 1;
		font: 700 13px var(--font-mono);
		font-variant-numeric: tabular-nums;
	}

	.chrome {
		position: absolute;
		inset: 0;
		pointer-events: none;
		transition: opacity var(--dur-slow);
	}

	.controls {
		position: absolute;
		left: 120px;
		right: 120px;
		bottom: 72px;
		display: flex;
		flex-direction: column;
		gap: 18px;
		pointer-events: auto;
	}

	.head {
		display: flex;
		align-items: flex-end;
		justify-content: space-between;
		gap: 40px;
	}

	.tv-heading {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.tv-eyebrow {
		font: 500 22px var(--font-mono);
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--player-accent-text);
	}

	.tv-title {
		font-size: 46px;
		font-weight: 700;
		letter-spacing: -0.02em;
		line-height: 1.1;
	}

	.mode {
		display: flex;
		align-items: center;
		gap: 12px;
		font: 500 22px var(--font-mono);
		color: var(--player-ink-muted);
	}

	.scrub {
		position: relative;
		height: 44px;
		display: flex;
		align-items: center;
		padding: 0;
		border: 0;
		background: transparent;
		cursor: pointer;
		outline: none;
		border-radius: var(--radius-player-controls);
	}

	.track {
		position: relative;
		flex: 1;
		height: 10px;
		display: flex;
		gap: 5px;
	}

	.seg {
		position: relative;
		flex: 1;
		height: 100%;
		border-radius: 5px;
		background: var(--player-buffer);
		overflow: hidden;
	}

	.buf {
		position: absolute;
		left: 0;
		top: 0;
		bottom: 0;
		background: var(--player-track);
	}

	.fill {
		position: absolute;
		left: 0;
		top: 0;
		bottom: 0;
		background: var(--player-ink);
	}

	.thumb {
		position: absolute;
		top: 50%;
		width: 28px;
		height: 28px;
		margin: -14px 0 0 -14px;
		border-radius: 50%;
		background: var(--player-ink);
		box-shadow: 0 0 0 8px color-mix(in srgb, var(--player-accent) 45%, transparent);
	}

	.times {
		display: flex;
		justify-content: space-between;
		font: 500 24px var(--font-mono);
		color: var(--player-ink-muted);
		margin-top: -6px;
	}

	.cur {
		color: var(--player-ink);
	}

	.row {
		display: flex;
		align-items: center;
		gap: 16px;
		margin-top: 6px;
	}

	.play {
		width: 84px;
		height: 84px;
		display: grid;
		place-items: center;
		border: 0;
		border-radius: 50%;
		background: rgba(255, 255, 255, 0.14);
		color: var(--player-ink);
		cursor: pointer;
		outline: none;
		transition: transform var(--dur-fast);
	}

	.play:focus {
		background: var(--player-ink);
		color: var(--fg-on-accent);
		transform: scale(1.08);
	}

	.pill {
		height: 64px;
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 0 28px;
		border: 0;
		border-radius: 32px;
		background: var(--player-control-hover);
		color: var(--player-ink);
		font-size: 24px;
		font-weight: 500;
		cursor: pointer;
		outline: none;
		white-space: nowrap;
		transition: transform var(--dur-fast);
	}

	.pill:focus {
		background: var(--player-ink);
		color: var(--fg-on-accent);
		transform: scale(1.06);
	}

	.menu-scrim {
		position: absolute;
		inset: 0;
		background: rgba(0, 0, 0, 0.35);
		opacity: 0;
		transition: opacity var(--dur-base) var(--ease);
	}

	.menu-scrim.vis {
		opacity: 1;
	}

	.menu {
		position: absolute;
		right: 40px;
		bottom: 40px;
		width: 330px;
		max-height: calc(100% - 80px);
		zoom: 1.75;
		display: flex;
		flex-direction: column;
		border-radius: 16px;
		background: rgba(17, 17, 25, 0.94);
		backdrop-filter: blur(28px) saturate(150%);
		border: 1px solid var(--border-strong);
		box-shadow: 0 24px 64px rgba(0, 0, 0, 0.6);
		overflow: hidden;
		opacity: 0;
		transform: translateX(24px);
		transform-origin: 100% 100%;
		transition: opacity 180ms var(--ease), transform 240ms var(--ease);
	}

	.menu.vis {
		opacity: 1;
		transform: none;
	}

	.menu-head {
		display: flex;
		align-items: center;
		gap: 2px;
		padding: 8px 8px 0;
		border-bottom: 1px solid var(--border);
	}

	.menu-title {
		height: 40px;
		display: flex;
		align-items: center;
		padding: 0 10px;
		font-size: 14px;
		font-weight: 600;
		color: var(--player-ink);
	}

	.grow {
		flex: 1;
	}

	.close {
		width: 34px;
		height: 34px;
		margin-bottom: 4px;
		display: grid;
		place-items: center;
		border: 0;
		border-radius: 8px;
		background: transparent;
		color: var(--fg-subtle);
		cursor: pointer;
		outline: none;
	}

	.close:focus,
	.opt:focus {
		background: var(--player-ink);
		color: var(--fg-on-accent);
	}

	.menu-body {
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		padding: 8px;
	}

	.note {
		display: flex;
		gap: 9px;
		margin: 2px 4px 8px;
		padding: 10px 12px;
		border-radius: var(--radius-md);
		background: var(--surface);
		border: 1px solid var(--border);
		font-size: 12px;
		line-height: 1.5;
		color: var(--player-ink-muted);
	}

	.note-dot {
		flex: none;
		width: 7px;
		height: 7px;
		margin-top: 5px;
		border-radius: 50%;
	}

	.opt {
		width: 100%;
		min-height: 40px;
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 5px 10px;
		border: 0;
		border-radius: var(--radius-md);
		background: transparent;
		color: var(--player-ink);
		text-align: left;
		cursor: pointer;
		outline: none;
	}

	.opt.sel {
		background: color-mix(in srgb, var(--player-accent) 10%, transparent);
	}

	.opt-text {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 1px;
	}

	.opt-sub {
		display: flex;
		align-items: center;
		gap: 5px;
		font: 400 11px var(--font-mono);
		color: var(--fg-subtle);
	}

	.opt-spin {
		display: grid;
		animation: slp-spin 1s linear infinite;
	}

	@keyframes slp-spin {
		to {
			transform: rotate(360deg);
		}
	}

	.badge {
		flex: none;
		padding: 2px 7px;
		border-radius: var(--radius-full);
		font: 500 10.5px var(--font-mono);
		border: 1px solid;
	}

	.badge.ok {
		color: var(--status-available);
		background: color-mix(in srgb, var(--status-available) 12%, transparent);
		border-color: color-mix(in srgb, var(--status-available) 25%, transparent);
	}

	.badge.conv {
		color: var(--status-downloading);
		background: color-mix(in srgb, var(--status-downloading) 14%, transparent);
		border-color: color-mix(in srgb, var(--status-downloading) 30%, transparent);
	}

	.badge.warn {
		color: var(--status-warning);
		background: color-mix(in srgb, var(--status-warning) 12%, transparent);
		border-color: color-mix(in srgb, var(--status-warning) 30%, transparent);
	}

	.stepper {
		margin: 8px 4px 2px;
		padding-top: 8px;
		border-top: 1px solid var(--border);
		display: flex;
		align-items: center;
		gap: 10px;
		transition: opacity var(--dur-base);
	}

	.stepper-text {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.stepper-label {
		font-size: 13px;
		font-weight: 500;
		color: var(--player-ink);
	}

	.stepper-desc {
		font-size: 11.5px;
		line-height: 1.4;
		color: var(--fg-subtle);
		text-wrap: pretty;
	}

	.stepper-box {
		flex: none;
		display: flex;
		align-items: center;
		gap: 2px;
		padding: 3px;
		border-radius: 9px;
		background: var(--surface);
	}

	.step {
		width: 30px;
		height: 30px;
		display: grid;
		place-items: center;
		border: 0;
		border-radius: 7px;
		background: transparent;
		color: var(--player-ink);
		outline: none;
	}

	.stepper-value {
		min-width: 62px;
		text-align: center;
		font: 500 12px var(--font-mono);
		font-variant-numeric: tabular-nums;
	}

	.reset {
		flex: none;
		height: 30px;
		padding: 0 6px;
		border: 0;
		border-radius: 7px;
		background: transparent;
		color: var(--player-accent-text);
		font-size: 12px;
		font-weight: 500;
		outline: none;
		transition: opacity var(--dur-fast);
	}

	.step:focus,
	.reset:focus,
	.seg-opt:focus,
	.style-head:focus {
		background: var(--player-ink);
		color: var(--fg-on-accent);
	}

	.night {
		margin: 10px 4px 2px;
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 10px 12px;
		border-radius: var(--radius-md);
		border: 1px dashed var(--border-strong);
		color: var(--fg-subtle);
	}

	.night-text {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 1px;
		font-size: 11.5px;
		line-height: 1.4;
	}

	.night-title {
		font-size: 13px;
		font-weight: 500;
		color: var(--player-ink-muted);
	}

	.night-later {
		flex: none;
		font: 500 10px var(--font-mono);
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--fg-faint);
	}

	.style-box {
		margin: 8px 4px 2px;
		padding-top: 4px;
		border-top: 1px solid var(--border);
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.style-head {
		width: 100%;
		min-height: 38px;
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 0 2px;
		border: 0;
		border-radius: 8px;
		background: transparent;
		color: var(--player-ink);
		text-align: left;
		outline: none;
	}

	.style-eyebrow {
		flex: none;
		font: 500 10.5px var(--font-mono);
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--fg-faint);
	}

	.style-summary {
		flex: 1;
		min-width: 0;
		text-align: right;
		font: 400 11.5px var(--font-mono);
		color: var(--fg-subtle);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.style-chevron {
		flex: none;
		display: grid;
		color: var(--fg-subtle);
		transition: transform var(--dur-base) var(--ease);
	}

	.style-note {
		margin-top: -4px;
		font-size: 11.5px;
		line-height: 1.45;
		color: var(--fg-subtle);
		text-wrap: pretty;
	}

	.style-row {
		display: flex;
		align-items: center;
		gap: 10px;
		transition: opacity var(--dur-base);
	}

	.style-label {
		width: 84px;
		font-size: 12.5px;
		color: var(--fg-subtle);
	}

	.seg-box {
		position: relative;
		flex: 1;
		display: flex;
		gap: 4px;
		padding: 3px;
		border-radius: 9px;
		background: var(--surface);
	}

	.seg-ind {
		position: absolute;
		left: 3px;
		top: 3px;
		bottom: 3px;
		border-radius: 7px;
		transition: transform var(--dur-slow) var(--ease), background-color var(--dur-base), opacity var(--dur-base);
	}

	.seg-opt {
		position: relative;
		flex: 1;
		height: 30px;
		border: 0;
		border-radius: 7px;
		background: transparent;
		font-size: 12.5px;
		font-weight: 500;
		outline: none;
		transition: color var(--dur-base);
	}

	.cue-wrap {
		position: absolute;
		left: 0;
		right: 0;
		display: flex;
		justify-content: center;
		pointer-events: none;
		transition: bottom 250ms var(--ease);
	}

	.sl-cue {
		display: inline-block;
		max-width: 80%;
		border-radius: var(--radius-sm);
		font-family: var(--font-sans);
		font-weight: 500;
		line-height: 1.3;
		text-align: center;
		white-space: pre-line;
	}

	.osd-wrap {
		position: absolute;
		left: 50%;
		top: 50%;
		transform: translate(-50%, -50%);
		pointer-events: none;
	}

	.sl-osd {
		display: inline-flex;
		align-items: center;
		gap: 12px;
		padding: 18px 30px;
		border-radius: 14px;
		background: rgba(0, 0, 0, 0.6);
		backdrop-filter: blur(12px);
		color: var(--player-ink);
		font: 500 40px var(--font-mono);
	}

	.opt.sel:focus {
		background: var(--player-ink);
	}

	.opt-check {
		flex: none;
		width: 18px;
		display: grid;
		place-items: center;
		color: var(--player-accent);
	}

	.opt-label {
		display: flex;
		align-items: center;
		gap: 7px;
		font-size: 14px;
	}

	.opt-pill {
		padding: 1px 5px;
		border-radius: 4px;
		border: 1px solid var(--border-strong);
		font: 600 9.5px var(--font-mono);
		letter-spacing: 0.06em;
		color: var(--player-ink-muted);
	}

	.card-slot {
		position: absolute;
		right: 70px;
		zoom: 1.75;
	}

	.skip-slot {
		transition: bottom 250ms var(--ease);
	}

	.sl-skip {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		height: 44px;
		padding: 0 18px;
		border: 1px solid rgba(255, 255, 255, 0.22);
		border-radius: var(--radius-player-controls);
		background: rgba(0, 0, 0, 0.45);
		backdrop-filter: blur(12px);
		color: var(--player-ink);
		font-size: 14px;
		font-weight: 600;
		cursor: pointer;
		outline: none;
		transition: transform var(--dur-fast);
	}

	.sl-skip:focus {
		background: var(--player-ink);
		color: var(--fg-on-accent);
		transform: scale(1.06);
	}

	.sl-upnext {
		width: 340px;
		overflow: hidden;
		border: 1px solid rgba(255, 255, 255, 0.12);
		border-radius: var(--radius-lg);
		background: var(--player-glass-strong);
		backdrop-filter: blur(24px);
		box-shadow: 0 24px 64px rgba(0, 0, 0, 0.6);
		color: var(--player-ink);
		animation: slp-rise var(--dur-slow) var(--ease);
	}

	.upnext-art {
		position: relative;
		aspect-ratio: 16 / 8;
		overflow: hidden;
	}

	.upnext-art img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		display: block;
	}

	.upnext-fade {
		position: absolute;
		inset: 0;
		background: linear-gradient(0deg, rgba(15, 15, 22, 0.95) 0%, rgba(15, 15, 22, 0) 60%);
	}

	.upnext-cap {
		position: absolute;
		left: 14px;
		right: 14px;
		bottom: 12px;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.upnext-code {
		font: 500 11px var(--font-mono);
		letter-spacing: 0.06em;
		color: var(--player-accent-text);
	}

	.upnext-title {
		font-size: 16px;
		font-weight: 600;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.upnext-timer {
		position: absolute;
		inset: auto 0 0 0;
		height: 3px;
		background: rgba(255, 255, 255, 0.16);
	}

	.upnext-timer > div {
		height: 100%;
		background: var(--player-accent);
		transform-origin: 0 50%;
		transition: transform 250ms linear;
		will-change: transform;
	}

	.upnext-row {
		display: flex;
		gap: 8px;
		padding: 12px 14px 14px;
	}

	.btn-ink {
		flex: 1;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 9px;
		height: 40px;
		border: 0;
		border-radius: var(--radius-md);
		background: var(--player-ink);
		color: #0b0b10;
		font-size: 14px;
		font-weight: 600;
		cursor: pointer;
		outline: none;
	}

	.btn-ink:focus {
		box-shadow: 0 0 0 3px var(--player-accent);
	}

	.btn-outline {
		height: 40px;
		padding: 0 14px;
		border: 1px solid rgba(255, 255, 255, 0.14);
		border-radius: var(--radius-md);
		background: transparent;
		color: var(--player-ink);
		font-size: 14px;
		font-weight: 500;
		cursor: pointer;
		outline: none;
	}

	.btn-outline:focus {
		background: var(--player-ink);
		color: var(--fg-on-accent);
	}

	.sl-stats {
		position: absolute;
		right: 70px;
		zoom: 1.75;
		width: 440px;
		padding: 14px 16px;
		border: 1px solid rgba(255, 255, 255, 0.1);
		border-radius: var(--radius-player-controls);
		background: rgba(5, 5, 8, 0.78);
		backdrop-filter: blur(14px);
		font: 400 11.5px/1.65 var(--font-mono);
		color: var(--player-ink-muted);
		transform-origin: 100% 100%;
		transition: bottom 240ms var(--ease);
	}

	.stats-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 8px;
		font-size: 10.5px;
		font-weight: 600;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--player-ink);
	}

	.stats-close {
		width: 26px;
		height: 26px;
		display: grid;
		place-items: center;
		border: 0;
		border-radius: var(--radius-sm);
		background: transparent;
		color: var(--fg-subtle);
		cursor: pointer;
		outline: none;
	}

	.stats-close:focus {
		background: var(--player-ink);
		color: var(--fg-on-accent);
	}

	.stats-row {
		display: grid;
		grid-template-columns: 96px minmax(0, 1fr);
		gap: 10px;
	}

	.stats-row > span:first-child {
		color: #5a5a68;
	}

	.stats-row > span:last-child {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.stats-row > span.why {
		white-space: normal;
		text-wrap: pretty;
	}

	.end {
		position: absolute;
		inset: 0;
		overflow: hidden;
		background: var(--bg-deep);
		animation: slp-fade 320ms ease-out;
	}

	.halt-scrim.end-scrim {
		background: linear-gradient(90deg, var(--hero-scrim) 0%, color-mix(in srgb, var(--bg-deep) 72%, transparent) 50%, color-mix(in srgb, var(--bg-deep) 40%, transparent) 100%);
	}

	.frame {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		object-fit: cover;
		filter: blur(32px) saturate(1.2);
		transform: scale(1.15);
		opacity: 0.5;
	}

	.end-next {
		display: flex;
		align-items: center;
		gap: 48px;
		animation: slp-rise 360ms var(--ease);
	}

	.end-next .end-col {
		flex: 1 1 0;
		min-width: 0;
		max-width: 520px;
		animation: none;
	}

	.end-syn {
		font-size: 14px;
		line-height: 1.55;
		color: var(--player-ink-muted);
		text-wrap: pretty;
	}

	.end-still {
		flex: none;
		width: 400px;
		aspect-ratio: 16 / 9;
		border-radius: 14px;
		overflow: hidden;
		box-shadow: 0 24px 64px rgba(0, 0, 0, 0.55), 0 0 0 1px var(--border);
	}

	.end-still img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		display: block;
	}

	.end-done-col {
		display: flex;
		flex-direction: column;
		gap: 36px;
	}

	.end-more {
		display: flex;
		flex-direction: column;
		gap: 12px;
		animation: slp-rise 440ms var(--ease);
	}

	.more-title {
		font-size: 15px;
		font-weight: 600;
	}

	.more-row {
		display: flex;
		gap: 14px;
	}

	.more-card {
		flex: none;
		width: 130px;
		padding: 0;
		border: 0;
		border-radius: var(--radius-md);
		background: transparent;
		cursor: pointer;
		outline: none;
		transition: transform var(--dur-fast), box-shadow var(--dur-fast);
	}

	.more-card:focus {
		transform: scale(1.06);
		box-shadow: 0 0 0 3px var(--player-accent);
	}

	.more-card img {
		width: 100%;
		aspect-ratio: 2 / 3;
		border-radius: var(--radius-md);
		object-fit: cover;
		display: block;
		box-shadow: 0 0 0 1px var(--border);
	}

	.halt-col.end-col {
		gap: 12px;
		animation: slp-rise 360ms var(--ease);
	}

	.end-col .actions {
		margin-top: 10px;
	}

	.primary.end-done {
		padding: 0 26px;
	}

	.secondary.end-again {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 0 20px 0 16px;
	}

	@keyframes slp-fade {
		from {
			opacity: 0;
		}
	}

	@keyframes slp-rise {
		from {
			opacity: 0;
			transform: translateY(10px);
		}
	}

	.halt {
		position: absolute;
		inset: 0;
		overflow: hidden;
		background: var(--bg-deep);
		animation: slp-fade 260ms var(--ease);
	}

	.halt-scrim {
		position: absolute;
		inset: 0;
		background: linear-gradient(90deg, var(--hero-scrim) 0%, color-mix(in srgb, var(--bg-deep) 74%, transparent) 55%, color-mix(in srgb, var(--bg-deep) 45%, transparent) 100%);
	}

	.halt-zoom {
		position: absolute;
		inset: 0;
		zoom: 1.75;
		display: flex;
		flex-direction: column;
		justify-content: center;
		padding: 0 70px;
	}

	.halt-col {
		display: flex;
		flex-direction: column;
		gap: 14px;
		max-width: 560px;
		animation: halt-rise 320ms var(--ease);
	}

	@keyframes halt-rise {
		from {
			transform: translateY(10px);
		}
	}

	.halt-note {
		font-size: 14px;
		line-height: 1.5;
		color: var(--player-ink-muted);
		text-wrap: pretty;
	}

	.halt-wait {
		display: flex;
		align-items: center;
		gap: 10px;
		margin-top: 2px;
		font: 400 12px var(--font-mono);
		color: var(--player-ink-muted);
	}

	.wait-dot {
		flex: none;
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--status-warning);
		animation: slp-pulse 1.6s ease-in-out infinite;
	}

	@keyframes slp-pulse {
		50% {
			opacity: 0.3;
		}
	}

	.eyebrow {
		display: flex;
		align-items: center;
		gap: 8px;
		font: 500 11px var(--font-mono);
		letter-spacing: 0.14em;
		text-transform: uppercase;
	}

	.halt-title {
		font-size: 36px;
		line-height: 1.1;
		font-weight: 700;
		letter-spacing: -0.02em;
		text-wrap: balance;
	}

	.halt-body {
		font-size: 16px;
		line-height: 1.5;
		color: var(--player-ink-muted);
		text-wrap: pretty;
	}

	.halt-mono {
		padding: 10px 12px;
		border-radius: var(--radius-md);
		background: color-mix(in srgb, var(--bg-elevated) 72%, transparent);
		border: 1px solid var(--border);
		font: 400 12px var(--font-mono);
		line-height: 1.5;
		color: var(--player-ink-muted);
		overflow-wrap: anywhere;
	}

	.count {
		display: flex;
		flex-direction: column;
		gap: 8px;
		max-width: 420px;
		margin-top: 4px;
	}

	.count-track {
		height: 4px;
		border-radius: var(--radius-full);
		background: var(--player-control-hover);
		overflow: hidden;
	}

	.count-fill {
		height: 100%;
		border-radius: var(--radius-full);
		transition: width 1s linear;
	}

	.count-text {
		font: 400 12px var(--font-mono);
		font-variant-numeric: tabular-nums;
		color: var(--player-ink-muted);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 10px;
		margin-top: 8px;
	}

	.primary {
		height: 48px;
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 0 22px 0 18px;
		border: 0;
		border-radius: var(--radius-player-controls);
		background: var(--player-ink);
		color: var(--fg-on-accent);
		font-size: 15px;
		font-weight: 600;
		cursor: pointer;
		outline: none;
		white-space: nowrap;
		transition: transform var(--dur-fast);
	}

	.primary:focus {
		box-shadow: 0 0 0 3px var(--player-accent);
		transform: scale(1.04);
	}

	.secondary {
		height: 48px;
		padding: 0 20px;
		border-radius: var(--radius-player-controls);
		border: 1px solid rgba(255, 255, 255, 0.18);
		background: transparent;
		color: var(--player-ink);
		font-size: 15px;
		font-weight: 500;
		cursor: pointer;
		outline: none;
		white-space: nowrap;
		transition: transform var(--dur-fast);
	}

	.secondary:focus {
		background: var(--player-ink);
		color: var(--fg-on-accent);
		transform: scale(1.04);
	}
</style>
