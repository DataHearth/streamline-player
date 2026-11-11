<script lang="ts">
import { PRESETS, rememberedThroughput } from "#lib/core/playback/auto.ts";
import type { Choice } from "#lib/core/playback/choice.ts";
import {
	audioName,
	audioOutName,
	clock,
	digits,
	fileMbit,
	hdrName,
	languageLabel,
	modeName,
	originalName,
	rangeName,
	subInfo,
	subName,
	videoName,
} from "#lib/core/playback/names.ts";
import { Playback, UP_NEXT_SECS } from "#lib/core/playback/playback.svelte.ts";
import type { Reason, Session, SubKind } from "#lib/core/playback/port.ts";
import { DEFAULT_SIZE_PX, SIZE_PX } from "#lib/core/playback/rules.ts";
import { prefs, setPref as savePref } from "#lib/core/prefs.svelte.ts";
import type { Decision } from "#lib/core/types.ts";
import { onPlayer, PLAYER_NAME, playbackPort } from "#platform";
import { focusDefault as focusStageDefault, move } from "#lib/tv/focus.ts";
import HaltScreen from "#lib/tv/HaltScreen.svelte";
import { hideBroken, orPoster } from "#lib/ui/img.ts";
import "#lib/tv/screen.css";
import {
	Activity,
	AudioLines,
	Captions,
	CaptionsOff,
	Check,
	ChevronDown,
	ChevronsRight,
	Gauge,
	LoaderCircle,
	Minus,
	Moon,
	Pause,
	Play,
	Plus,
	RotateCcw,
	RotateCw,
	Settings,
	SkipForward,
	X,
} from "@lucide/svelte";
import { onMount, tick } from "svelte";
import { m as i18n } from "#lib/paraglide/messages.js";
import { getLocale } from "#lib/paraglide/runtime.js";

type Next = {
	code: string;
	title: string;
	overview: string;
	still: string;
	stillPos: string;
	label: string;
	href: string;
};
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
	anime,
	choice,
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
	anime: boolean;
	choice: Choice | null;
	onexit: () => void;
	onnext: () => void;
	onopen: (href: string) => void;
} = $props();

type Tab = "subs" | "audio" | "quality" | "speed";
type StatRow = { k: string; v: string; c?: string; why?: boolean };
type Tone = "ok" | "conv" | "warn";
type Flash = { ph: "idle" | "in" | "out"; amt: number; rot: number };

const HIDE_MS = 3200;
// Seconds the bar may run ahead of the last report, should reports stop.
const MAX_EXTRAPOLATION = 1;
const SEEK_STEP = 10;
const TAB_NAMES: Record<Tab, () => string> = {
	audio: i18n.player_audio,
	subs: i18n.player_subtitles,
	quality: i18n.player_quality,
	speed: i18n.player_speed,
};
const TV_CUE_SCALE = 1.7;
const OSD_MS = 900;
const SPEEDS = [0.5, 0.75, 1, 1.25, 1.5, 2];
const SLOW_THROUGHPUT = 4_000_000;
const MENU_EXIT_MS = 240;
const SEEK_RUN_MS = 900;
// Prefs hold the English values; only the label shown to the viewer follows the locale.
const STYLE_LABEL: Record<string, () => string> = {
	None: i18n.player_style_bg_none,
	Shadow: i18n.player_style_bg_shadow,
	Box: i18n.player_style_bg_box,
	White: i18n.player_style_color_white,
	Yellow: i18n.player_style_color_yellow,
};
const styleText = (v: string) => STYLE_LABEL[v]?.() ?? v;
const times = (x: number) =>
	`${new Intl.NumberFormat(getLocale(), { maximumFractionDigits: 2 }).format(x)}×`;

let controls = $state(true);
let menu = $state(false);
let menuMounted = $state(false);
let menuVis = $state(false);
let pfPause = $state(false);
let tab = $state<Tab>("quality");
let styleOpen = $state(false);
let osd = $state<string | null>(null);
let skL = $state<Flash>({ ph: "idle", amt: 0, rot: 0 });
let skR = $state<Flash>({ ph: "idle", amt: 0, rot: 0 });
let pf = $state<"idle" | "in" | "out">("idle");

let stage: HTMLElement;
let lastMove = Date.now();
let seekTimer: ReturnType<typeof setTimeout> | undefined;
let pfTimer: ReturnType<typeof setTimeout> | undefined;
let osdTimer: ReturnType<typeof setTimeout> | undefined;
let menuTimer: ReturnType<typeof setTimeout> | undefined;
let seekLeft: boolean | null = null;
let seekAt = 0;

const playNext = () => pb.leave().then(onnext);
// svelte-ignore state_referenced_locally
const pb = new Playback(
	playbackPort,
	{ mediaId, titleName, anime, choice, start: startAt, onEnd: next ? playNext : undefined },
	{
		halted: () => {
			if (menu) closeMenu();
			focusDefault();
		},
		ended: () => {
			if (menu) closeMenu();
			focusDefault();
		},
		started: () => showControls(true),
		settled: () => {
			if (!menu) focusDefault();
		},
		subtitlesUnavailable: () => flashOsd(i18n.player_subtitles_unavailable()),
	},
);

const halted = $derived(!!pb.halt && !pb.haltLeaving);
const skipOn = $derived(((pb.introOn && !prefs.autoSkip) || pb.creditsOn) && !pb.ended && !pb.halt);
const upNextFrac = $derived(
	pb.creditsAt === null ? 0 : Math.min(1, Math.max(0, (pb.position - pb.creditsAt) / UP_NEXT_SECS)),
);
// The player reports every 500 ms; between reports the bar moves on each frame at the playback speed.
let frameAt = $state(0);
const shownPosition = $derived(
	pb.playing
		? Math.min(
				pb.duration || Infinity,
				pb.position +
					Math.min(MAX_EXTRAPOLATION, Math.max(0, ((frameAt - pb.positionAt) / 1000) * pb.speed)),
			)
		: pb.position,
);
const segs = $derived.by(() => {
	const list = pb.tracks?.chapters ?? [];
	if (list.length < 2 || pb.duration <= 0)
		return [{ flex: 1, fill: pct, buf: pb.duration > 0 ? (pb.buffered / pb.duration) * 100 : 0 }];
	return list.map((c, i) => {
		const end = list[i + 1]?.start ?? pb.duration;
		const len = Math.max(1, end - c.start);
		const within = (at: number) => Math.max(0, Math.min(100, ((at - c.start) / len) * 100));
		return { flex: len, fill: within(shownPosition), buf: within(pb.buffered) };
	});
});
const isEpisode = $derived(/ S\d+ E\d+$/.test(eyebrow));
const controlsOn = $derived(controls || (!pb.playing && !pb.buffering) || menu);
const cardBottom = $derived(controlsOn ? 220 : 50);
const pct = $derived(pb.duration > 0 ? (shownPosition / pb.duration) * 100 : 0);
$effect(() => {
	if (!pb.playing || !controlsOn) return;
	let raf = requestAnimationFrame(function tick(t) {
		frameAt = t;
		raf = requestAnimationFrame(tick);
	});
	return () => cancelAnimationFrame(raf);
});
const modeLabel = $derived.by(() => {
	if (!pb.session) return "";
	const { width, height, hdr, dv_profile } = pb.session.video;
	const range = dv_profile !== undefined ? " · Dolby Vision" : hdr === "sdr" ? "" : " · HDR";
	// Wide films are cropped (3840×1608, 3816×1592), so height alone would call them 1608p.
	const size =
		width >= 7000 || height >= 4000 ? "8K" : width >= 3400 || height >= 2000 ? "4K" : `${height}p`;
	return `${size}${range}`;
});
const fileRate = $derived(pb.preview ? fileMbit(pb.preview.bitRate) : "");
const qualText = $derived(
	pb.qSel === "auto"
		? i18n.player_auto()
		: pb.qSel === "original"
			? i18n.player_original()
			: i18n.player_mbit({ value: pb.qSel }),
);
const menuNote = $derived.by(() => {
	const session = pb.session;
	if (!session) return "";
	const why = (r: Reason[]) => r.map((x) => x.text).join(", ");
	switch (session.mode) {
		case "direct":
			return i18n.player_note_direct();
		case "remux":
			return i18n.player_note_remux({
				player: PLAYER_NAME,
				container: pb.tracks?.container.toUpperCase() ?? "",
			});
		case "audio_transcode":
			return i18n.player_note_audio_transcode({
				audio: audioOutName(session),
				why: why(session.audio_reasons),
			});
		case "transcode":
			return i18n.player_note_transcode({
				video: videoOutName(session),
				why: why(session.video_reasons),
			});
	}
});

const selectedSub = $derived(pb.tracks?.subtitles.find((t) => t.index === pb.subPick) ?? null);
const subKind = $derived<SubKind | "burn">(
	pb.burn ? "burn" : selectedSub ? subInfo(selectedSub).kind : "text",
);
const cueSize = $derived(
	Math.round(
		(subKind === "text" || subKind === "styled"
			? (SIZE_PX[prefs.subSize] ?? DEFAULT_SIZE_PX)
			: DEFAULT_SIZE_PX) * TV_CUE_SCALE,
	),
);

function setPref(key: "subSize" | "subBg" | "subColor", value: string) {
	savePref(key, value);
	pb.syncTiming();
}

const audioNote = $derived.by(() => {
	if (!pb.selectedAudio || !pb.session) return "";
	const name = audioName(pb.selectedAudio);
	return pb.session.audio.action === "copy"
		? i18n.player_audio_note_as_is({ player: PLAYER_NAME, name })
		: i18n.player_audio_note_convert({
				player: PLAYER_NAME,
				name,
				out: audioOutName(pb.session),
			});
});

const styleNote = $derived(
	subKind === "styled"
		? i18n.player_style_note_styled()
		: subKind === "image"
			? i18n.player_style_note_image()
			: subKind === "burn"
				? i18n.player_style_note_burn()
				: "",
);
const styleSummary = $derived(
	subKind === "text"
		? `${prefs.subSize} · ${styleText(prefs.subBg)} · ${styleText(prefs.subColor)}`
		: subKind === "styled"
			? i18n.player_style_own_look({ size: prefs.subSize })
			: i18n.player_style_drawn(),
);
const styleOk = (key: string) => subKind === "text" || (subKind === "styled" && key === "subSize");

const fmtOffset = (v: number, seconds: boolean) => {
	const sign = v > 0 ? "+" : v < 0 ? "−" : "";
	return (
		sign +
		(seconds
			? i18n.player_unit_seconds({ value: digits(Math.abs(v), 1) })
			: i18n.player_unit_ms({ value: Math.abs(v) }))
	);
};

function flashOsd(text: string) {
	clearTimeout(osdTimer);
	osd = text;
	osdTimer = setTimeout(() => (osd = null), OSD_MS);
}

function nudgeSub(step: number) {
	if (step === 0) pb.resetTiming("sub");
	else pb.nudgeSub(step);
	lastMove = Date.now();
	flashOsd(i18n.player_osd_subtitles({ offset: fmtOffset(pb.subOffset, true) }));
}

function nudgeAudio(step: number) {
	if (step === 0) pb.resetTiming("audio");
	else pb.nudgeAudio(step);
	lastMove = Date.now();
	flashOsd(i18n.player_osd_audio({ offset: fmtOffset(pb.audioDelay, false) }));
}

const mbit = (bits: number) =>
	i18n.player_mbit({ value: digits(bits / 1_000_000, bits < 1_000_000 ? 2 : 1) });
function videoOutName(s: Session) {
	const copy = s.video.action === "copy";
	const name = videoName(
		s.video.codec,
		s.video.width,
		s.video.height,
		copy ? pb.tracks?.video?.profile : undefined,
	);
	if (copy) return `${name}${rangeName(s.video.hdr, s.video.dv_profile !== undefined)}`;
	return `${name} ${hdrName(s.video.hdr) ?? "SDR"}`;
}
const originalSub = $derived(originalName(pb.tracks?.video ?? null));
const modeBadge = (d?: Decision) =>
	d && !d.error ? modeName(d.mode).toLocaleLowerCase(getLocale()) : "";
const modeTone = (d?: Decision): Tone => (d && !d.error && !d.needs_slot ? "ok" : "conv");

function qualitySub(d: Decision | undefined) {
	if (!d || d.error) return d?.error.message ?? "";
	return d.video.action === "copy"
		? i18n.player_quality_same_as_original({ size: fileRate })
		: videoName(d.video.codec, d.video.width, d.video.height);
}

const autoDecision = $derived(
	pb.autoPick ? pb.preview?.presets[pb.autoPick] : pb.preview?.original,
);
const autoSub = $derived.by(() => {
	const d = pb.preview?.original;
	const why =
		d && !d.error && !pb.originalPlays
			? (d.video_reasons[0]?.text ?? "")
			: i18n.player_auto_based_on_connection();
	if (pb.autoPick)
		return i18n.player_auto_now({ quality: i18n.player_mbit({ value: pb.autoPick }), why });
	if (!d || d.error || pb.originalPlays)
		return pb.measuredOnce
			? i18n.player_auto_now({
					quality: i18n.player_original(),
					why: i18n.player_auto_connection_allows(),
				})
			: i18n.player_auto_now_only({ quality: i18n.player_original() });
	return i18n.player_auto_now({
		quality: videoName(d.video.codec, d.video.width, d.video.height),
		why,
	});
});

const statRows = $derived.by(() => {
	const { session, tracks, live, selectedAudio, encodeSpeed } = pb;
	if (!session) return [];
	const copy = session.video.action === "copy";
	const direct = session.mode === "direct";
	const box = tracks?.container.toUpperCase() ?? "";
	const inName = tracks?.video
		? `${videoName(tracks.video.codec, tracks.video.width, tracks.video.height, tracks.video.profile)}${rangeName(tracks.video.hdr, !!tracks.video.dv)}`
		: "";
	const outName = videoOutName(session);
	const audioIn = selectedAudio ? audioName(selectedAudio) : "";
	const audioOut = audioOutName(session);
	const why = (list: Reason[]): StatRow[] =>
		list.length
			? [{ k: i18n.player_stat_why(), v: list.map((r) => r.text).join(" · "), why: true }]
			: [];
	const known = [
		live &&
			live.videoBitrate > 0 &&
			i18n.player_stat_bitrate_video({ rate: mbit(live.videoBitrate) }),
		live &&
			live.audioBitrate > 0 &&
			i18n.player_stat_bitrate_audio({ rate: mbit(live.audioBitrate) }),
	].filter(Boolean);
	const rows: StatRow[] = [
		{
			k: i18n.player_stat_mode(),
			v: `${modeName(session.mode)}${direct ? ` · ${i18n.player_stat_original_file()}` : session.mode === "remux" ? ` · ${i18n.player_stat_nothing_converted()}` : ""}`,
			c: pb.full ? "var(--status-available)" : "var(--status-downloading)",
		},
		{ k: i18n.player_stat_player(), v: live?.player ?? PLAYER_NAME },
		{
			k: i18n.player_stat_video(),
			v: copy ? `${outName} · ${i18n.player_stat_as_is()}` : `${inName} → ${outName}`,
			c: "var(--player-ink)",
		},
		...(copy ? [] : why(session.video_reasons)),
		{
			k: i18n.player_audio(),
			v:
				session.audio.action === "copy"
					? `${audioIn} · ${i18n.player_stat_as_is()}`
					: `${audioIn} → ${audioOut}`,
			c: "var(--player-ink)",
		},
		...(session.audio.action === "copy" ? [] : why(session.audio_reasons)),
		{
			k: i18n.player_stat_container(),
			v: direct ? `${box} · ${i18n.player_stat_file_itself()}` : `${box} → fMP4 · HLS`,
		},
		...(session.mode === "remux"
			? [
					{
						k: i18n.player_stat_why(),
						v: i18n.player_stat_not_playable({ container: box, player: PLAYER_NAME }),
						why: true,
					},
				]
			: []),
		...(encodeSpeed === null
			? []
			: [
					{
						k: i18n.player_stat_encode(),
						v: `${i18n.player_stat_realtime({ speed: digits(encodeSpeed, 1) })} · ${copy ? i18n.player_stat_audio_only() : session.hw.toUpperCase()}`,
						c:
							encodeSpeed >= 1.2
								? "var(--status-available)"
								: encodeSpeed >= 1
									? "var(--status-warning)"
									: "var(--status-failed)",
					},
				]),
		{
			k: i18n.player_subtitles(),
			v: tracks?.preparing
				? i18n.player_preparing()
				: selectedSub
					? `${subName(selectedSub)} · ${subInfo(selectedSub).fmt} · ${pb.burn ? i18n.player_stat_burned_in() : subRenderer(subInfo(selectedSub).kind)}`
					: i18n.player_subtitles_off(),
		},
		...(pb.subOffset || pb.audioDelay
			? [
					{
						k: i18n.player_stat_offsets(),
						v: i18n.player_stat_offsets_value({
							subtitles: fmtOffset(pb.subOffset, true),
							audio: fmtOffset(pb.audioDelay, false),
						}),
					},
				]
			: []),
		...(known.length ? [{ k: i18n.player_stat_bitrate(), v: known.join(" · ") }] : []),
	];
	if (live) {
		const speed = pb.measured ?? (pb.network && rememberedThroughput(pb.network));
		rows.push(
			{
				k: i18n.player_stat_throughput(),
				v: speed === null ? i18n.player_measuring() : mbit(speed),
				c: speed !== null && speed < SLOW_THROUGHPUT ? "var(--status-warning)" : undefined,
			},
			{
				k: i18n.player_stat_buffer(),
				v: i18n.player_stat_buffer_ahead({ seconds: digits(live.buffer, 1) }),
			},
			{
				k: i18n.player_stat_frames(),
				v: i18n.player_stat_frames_value({
					dropped: live.dropped.toLocaleString(getLocale()),
					rendered: live.rendered.toLocaleString(getLocale()),
				}),
			},
			{
				k: i18n.player_stat_display(),
				v: i18n.player_stat_display_value({
					hz: (Math.round(live.displayHz * 1000) / 1000).toLocaleString(getLocale()),
				}),
			},
		);
	}
	rows.push(
		{ k: i18n.player_speed(), v: times(pb.speed) },
		{
			k: i18n.player_stat_server(),
			v: `${new URL(session.url).host} · ${direct ? "/file" : "/hls"}`,
		},
	);
	return rows;
});

const subRenderer = (kind: SubKind) =>
	kind === "styled"
		? "libass"
		: kind === "image"
			? PLAYER_NAME === "mpv"
				? "mpv"
				: "Media3"
			: i18n.player_renderer_web_view();

const pop = (_: Element, { y = 6, scale = 0.98 } = {}) => ({
	duration: 220,
	css: (t: number) =>
		`opacity:${t};transform:translateY(${(1 - t) * y}px) scale(${scale + (1 - scale) * t})`,
});

// With the controls hidden, nothing else holds focus: OK must land on the card.
$effect(() => {
	if ((skipOn || pb.upNextOn) && !controlsOn)
		tick().then(() =>
			stage?.querySelector<HTMLElement>(".sl-skip, .sl-upnext [data-tv-default]")?.focus(),
		);
});

function showControls(focus = false) {
	lastMove = Date.now();
	controls = true;
	if (focus) focusDefault();
}

async function focusDefault() {
	await tick();
	if (stage) focusStageDefault(stage);
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

async function exit() {
	await pb.exit();
	onexit();
}

function togglePlay() {
	const wasPlaying = pb.playing;
	pb.togglePlay();
	pfPause = wasPlaying;
	clearTimeout(pfTimer);
	pf = "in";
	pfTimer = setTimeout(() => {
		pf = "out";
		pfTimer = setTimeout(() => (pf = "idle"), 220);
	}, 520);
}

function seek(delta: number) {
	pb.seek(delta);
	const left = delta < 0;
	const cur = left ? skL : skR;
	const now = Date.now();
	const run = seekLeft === left && now - seekAt < SEEK_RUN_MS && cur.ph !== "idle";
	seekLeft = left;
	seekAt = now;
	const flash: Flash = {
		ph: "in",
		amt: (run ? cur.amt : 0) + Math.abs(delta),
		rot: (run ? cur.rot : 0) + (left ? -360 : 360),
	};
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
	pb.refreshTracks();
	focusDefault();
}

function back() {
	if (halted || pb.ended) return exit();
	if (menu) return closeMenu();
	if (pb.stats) {
		pb.setStats(false);
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
	// A keyboard's Space, as in every desktop player; a remote has none.
	if (k === " " && !halted && !pb.ended) {
		e.preventDefault();
		togglePlay();
		return;
	}
	const arrow = k.startsWith("Arrow");
	if (!arrow && k !== "Enter") return;
	if (halted || menu || pb.ended) {
		if (arrow) {
			e.preventDefault();
			move(k, stage);
		}
		return;
	}
	if (!controlsOn) {
		const onCard = (document.activeElement as HTMLElement | null)?.closest(".sl-skip, .sl-upnext");
		if (onCard && k === "Enter") return;
		if (onCard && pb.upNextOn && (k === "ArrowLeft" || k === "ArrowRight")) {
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
	const endPlayback = pb.begin();
	const listeners = [
		onPlayer("back", back),
		onPlayer("subtitleFailed", () => flashOsd(i18n.player_subtitles_unavailable())),
	];
	const hide = setInterval(() => {
		if (controls && pb.playing && !menu && Date.now() - lastMove > HIDE_MS) controls = false;
	}, 250);
	return () => {
		clearInterval(hide);
		clearTimeout(menuTimer);
		for (const l of listeners) l.then((off) => off());
		endPlayback();
	};
});
</script>

<svelte:window onkeydown={onKey} />

{#snippet flash(f: Flash, side: "left" | "right")}
	<div class="sk {side}" aria-hidden="true">
		<div class="sk-disc {f.ph}" style:--dir={side === "left" ? -14 : 14}>
			<span
				class="sk-ico"
				style:transform="rotate({f.rot}deg)"
				style:transition={f.ph === "idle" ? "none" : "transform 450ms var(--ease)"}
			>
				{#if side === "left"}
					<RotateCcw size={46} strokeWidth={1.5} />
				{:else}
					<RotateCw size={46} strokeWidth={1.5} />
				{/if}
			</span>
			<span class="sk-amt">{f.amt}</span>
		</div>
	</div>
{/snippet}

{#snippet option(
	label: string,
	selected: boolean,
	onclick: () => void,
	sub = "",
	badge = "",
	tone: Tone = "ok",
	busy = false,
	pill = "",
	off = false,
)}
	<button
		type="button"
		data-tv="1"
		data-tv-default={selected ? "1" : undefined}
		class="opt"
		class:sel={selected}
		disabled={busy || off}
		style:opacity={busy ? 0.62 : off ? 0.5 : 1}
		{onclick}
	>
		<span class="opt-check" style:opacity={selected ? 1 : 0}
			><Check size={16} strokeWidth={2.5} /></span
		>
		<span class="opt-text">
			<span class="opt-label" style:font-weight={selected ? 600 : 500}
				>{label}
				{#if pill}
					<span class="opt-pill">{pill}</span>
				{/if}</span
			>
			{#if sub}
				<span class="opt-sub"
					>{#if busy}
						<span class="opt-spin"><LoaderCircle size={11} strokeWidth={2} /></span>
					{/if}
					{sub}</span
				>
			{/if}
		</span>
		{#if badge}
			<span class="badge {tone}">{badge}</span>
		{/if}
	</button>
{/snippet}

{#snippet stepper(
	label: string,
	desc: string,
	value: string,
	zero: boolean,
	what: string,
	dec: () => void,
	inc: () => void,
	reset: () => void,
	disabled = false,
)}
	<div class="stepper" style:opacity={disabled ? 0.5 : 1}>
		<span class="stepper-text">
			<span class="stepper-label">{label}</span>
			{#if !zero}
				<span class="stepper-desc">{desc}</span>
			{/if}
		</span>
		<div class="stepper-box" style:pointer-events={disabled ? "none" : "auto"}>
			<button
				type="button"
				data-tv="1"
				{disabled}
				class="step"
				aria-label={i18n.player_step_earlier({ what })}
				onclick={dec}
			>
				<Minus size={14} strokeWidth={2.25} />
			</button>
			<span
				class="stepper-value"
				style:color={zero ? "var(--player-ink-muted)" : "var(--player-accent-text)"}
				>{value}</span
			>
			<button
				type="button"
				data-tv="1"
				{disabled}
				class="step"
				aria-label={i18n.player_step_later({ what })}
				onclick={inc}
			>
				<Plus size={14} strokeWidth={2.25} />
			</button>
		</div>
		<button
			type="button"
			data-tv="1"
			disabled={zero}
			class="reset"
			style:opacity={zero ? 0 : 1}
			aria-label={i18n.player_reset_timing({ what: what.toLocaleLowerCase(getLocale()) })}
			onclick={reset}
		>
			{i18n.player_reset()}
		</button>
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
				<button
					type="button"
					data-tv="1"
					disabled={!on}
					class="seg-opt"
					style:color={prefs[key] === v && on ? "var(--fg-on-accent)" : "var(--player-ink-muted)"}
					onclick={() => setPref(key, v)}
				>
					{styleText(v)}
				</button>
			{/each}
		</div>
	</div>
{/snippet}

<div class="stage" bind:this={stage}>
	<div class="scrim" style:opacity={controlsOn ? 1 : 0}></div>

	{#if pb.buffering}
		<div class="sl-spin" role="status" aria-label={i18n.player_buffering()}></div>
	{/if}

	{#if pb.cueText}
		<div class="cue-wrap" style:bottom="{controlsOn ? 400 : 90}px">
			<span
				class="sl-cue"
				style:font-size="{cueSize}px"
				style:color={prefs.subColor === "Yellow" ? "#FDE68A" : "#FFFFFF"}
				style:text-shadow={prefs.subBg === "Shadow"
					? "0 2px 4px rgba(0,0,0,0.9), 0 0 12px rgba(0,0,0,0.6)"
					: prefs.subBg === "None"
						? "0 0 2px rgba(0,0,0,0.9)"
						: "none"}
				style:background={prefs.subBg === "Box" ? "rgba(0,0,0,0.72)" : "transparent"}
				style:padding={prefs.subBg === "Box" ? "4px 12px" : "0"}
				>{pb.cueText}</span
			>
		</div>
	{/if}

	{#if osd}
		<div class="osd-wrap"><div class="sl-osd">{osd}</div></div>
	{/if}

	<div class="pf" aria-hidden="true">
		<div class="pf-disc {pf}">
			{#if pfPause}
				<Pause size={36} fill="currentColor" />
			{:else}
				<Play size={36} fill="currentColor" />
			{/if}
		</div>
	</div>
	{@render flash(skL, "left")}
	{@render flash(skR, "right")}

	<div class="chrome" style:opacity={controlsOn ? 1 : 0}>
		{#if controlsOn && !pb.ended}
			<div class="controls">
				<div class="head">
					<span class="tv-heading"
						><span class="tv-eyebrow">{eyebrow}</span><span class="tv-title">{heading}</span></span
					>
					<span class="mode">{modeLabel}</span>
				</div>
				<button
					type="button"
					data-tv="1"
					data-scrub="1"
					class="scrub"
					aria-label={i18n.player_seek()}
				>
					<div class="track">
						{#each segs as g}
							<div class="seg" style:flex={g.flex}>
								<div class="buf" style:width="{g.buf}%"></div>
								<div class="fill" style:width="{g.fill}%"></div>
							</div>
						{/each}
					</div>
					<div class="thumb" style:left="{pct}%"></div>
				</button>
				<div class="times">
					<span class="cur">{clock(pb.position)}</span><span>{pb.chapterName}</span
					><span>-{clock(pb.duration - pb.position)}</span>
				</div>
				<div class="row">
					<button
						type="button"
						data-tv="1"
						data-tv-default="1"
						class="play"
						aria-label={pb.playing ? i18n.player_pause() : i18n.player_play()}
						onclick={togglePlay}
					>
						{#if pb.playing}
							<Pause size={36} fill="currentColor" />
						{:else}
							<Play size={36} fill="currentColor" />
						{/if}
					</button>
					<button type="button" data-tv="1" class="pill" onclick={() => openMenu("subs")}>
						{#if selectedSub}
							<Captions size={28} strokeWidth={1.9} />{subName(selectedSub)}
						{:else}
							<CaptionsOff size={28} strokeWidth={1.9} />{i18n.player_subtitles()}
						{/if}
					</button>
					<button type="button" data-tv="1" class="pill" onclick={() => openMenu("audio")}>
						<AudioLines size={28} strokeWidth={1.9} />{i18n.player_audio()}
					</button>
					<button type="button" data-tv="1" class="pill" onclick={() => openMenu("quality")}>
						<Settings size={28} strokeWidth={1.9} />{qualText}
					</button>
					<button type="button" data-tv="1" class="pill" onclick={() => openMenu("speed")}>
						<Gauge size={28} strokeWidth={1.9} />{times(pb.speed)}
					</button>
					<button type="button" data-tv="1" class="pill" onclick={() => pb.setStats(!pb.stats)}>
						<Activity size={28} strokeWidth={1.9} />{i18n.player_stats()}
					</button>
					{#if next}
						<button type="button" data-tv="1" class="pill" onclick={() => playNext()}>
							<SkipForward size={28} strokeWidth={1.9} />{i18n.player_next_episode()}
						</button>
					{/if}
				</div>
			</div>
		{/if}
	</div>

	{#if menuMounted}
		<div class="menu-scrim" class:vis={menuVis}></div>
		<div class="menu" class:vis={menuVis} data-tv-trap={menu ? "1" : undefined} inert={!menu}>
			<div class="menu-head">
				<span class="menu-title">{TAB_NAMES[tab]()}</span>
				<div class="grow"></div>
				<button
					type="button"
					data-tv="1"
					class="close"
					aria-label={i18n.player_close()}
					onclick={closeMenu}
				>
					<X size={16} strokeWidth={2} />
				</button>
			</div>
			<div class="menu-body">
				{#if tab === "quality"}
					<div class="note">
						<span
							class="note-dot"
							style:background={pb.full ? "var(--status-available)" : "var(--status-downloading)"}
						></span><span>{menuNote}</span>
					</div>
					{@render option(
						i18n.player_auto(),
						pb.qSel === "auto",
						() => pb.pickQuality("auto"),
						autoSub,
						modeBadge(autoDecision),
						modeTone(autoDecision),
					)}
					{#if pb.originalPlays}
						{@render option(
							i18n.player_original(),
							pb.qSel === "original",
							() => pb.pickQuality("original"),
							`${originalSub}${fileRate ? ` \u00B7 ${fileRate}` : ""}`,
							modeBadge(pb.preview?.original),
							modeTone(pb.preview?.original),
						)}
					{:else}
						{@render option(
							i18n.player_original(),
							false,
							() => {},
							i18n.player_original_unavailable({
								reason:
									pb.preview?.original && !pb.preview.original.error
										? (pb.preview.original.video_reasons[0]?.text ?? "")
										: "",
							}),
							"",
							"ok",
							false,
							"",
							true,
						)}
					{/if}
					{#each PRESETS as n}
						{@const d = pb.preview?.presets[n]}
						{@const converted = !!d && !d.error && d.video.action !== "copy"}
						{@render option(
							i18n.player_mbit({ value: n }),
							pb.qSel === n,
							() => pb.pickQuality(n),
							qualitySub(d),
							modeBadge(d),
							modeTone(d),
							false,
							converted && !d.error && d.video.hdr !== "sdr" ? "HDR" : "",
						)}
					{/each}
				{:else if tab === "speed"}
					{#each SPEEDS as x}
						{@render option(x === 1 ? i18n.player_speed_normal() : times(x), pb.speed === x, () =>
							pb.pickSpeed(x),
						)}
					{/each}
				{:else if tab === "audio"}
					<div class="note">
						<span
							class="note-dot"
							style:background={pb.session?.audio.action === "copy"
								? "var(--status-available)"
								: "var(--status-downloading)"}
						></span><span>{audioNote}</span>
					</div>
					{#each pb.tracks?.audio ?? [] as t, i}
						{@const d = pb.preview?.audio[t.index]}
						{@const out =
							d && !d.error ? d : t.index === pb.selectedAudio?.index ? pb.session : null}
						{@const asIs = out?.audio.action === "copy"}
						{@render option(
							i === 0
								? i18n.player_audio_original({ language: languageLabel(t.language) })
								: languageLabel(t.language),
							t.index === pb.selectedAudio?.index,
							() => pb.pickAudio(t),
							out && !asIs ? `${audioName(t)} \u2192 ${audioOutName(out)}` : audioName(t),
							out ? (asIs ? i18n.player_badge_as_is() : i18n.player_badge_converted()) : "",
							asIs ? "ok" : "conv",
						)}
					{/each}
					{@render stepper(
						i18n.player_audio_delay(),
						pb.audioDelay > 0 ? i18n.player_audio_delay_later() : i18n.player_audio_delay_earlier(),
						fmtOffset(pb.audioDelay, false),
						pb.audioDelay === 0,
						i18n.player_audio(),
						() => nudgeAudio(-10),
						() => nudgeAudio(10),
						() => nudgeAudio(0),
					)}
					<div class="night">
						<Moon size={16} strokeWidth={1.75} />
						<span class="night-text"
							><span class="night-title">{i18n.player_night_mode()}</span
							><span>{i18n.player_night_mode_desc()}</span></span
						>
						<span class="night-later">{i18n.player_night_later()}</span>
					</div>
				{:else}
					{#if pb.tracks?.preparing}
						<div class="note">
							<span class="note-dot" style:background="var(--status-downloading)"></span
							><span>{i18n.player_subtitles_preparing_note()}</span>
						</div>
					{:else if pb.burn && selectedSub}
						<div class="note">
							<span class="note-dot" style:background="var(--status-warning)"></span
							><span
								>{i18n.player_subtitles_burn_note({
									player: PLAYER_NAME,
									format: subInfo(selectedSub).fmt,
									name: subName(selectedSub),
									back: pb.burnBack,
								})}</span
							>
						</div>
					{/if}
					{@render option(i18n.player_subtitles_off(), pb.subPick === null, () => pb.pickSub(null))}
					{#each pb.tracks?.subtitles ?? [] as t}
						{@const info = subInfo(t)}
						{#if pb.tracks?.preparing && !info.burnOnly}
							{@render option(subName(t), false, () => {}, i18n.player_preparing(), "", "ok", true)}
						{:else if info.burnOnly}
							{@render option(
								subName(t),
								pb.subPick === t.index,
								() => pb.pickSub(t),
								i18n.player_sub_burn_in({ label: info.label }),
								i18n.player_badge_converts_video(),
								"warn",
							)}
						{:else}
							{@render option(
								subName(t),
								pb.subPick === t.index,
								() => pb.pickSub(t),
								t.sdh ? i18n.player_sub_sdh({ label: info.label }) : info.label,
							)}
						{/if}
					{/each}
					{@render stepper(
						i18n.player_sync(),
						pb.subOffset > 0 ? i18n.player_sync_later() : i18n.player_sync_earlier(),
						fmtOffset(pb.subOffset, true),
						pb.subOffset === 0,
						i18n.player_subtitles(),
						() => nudgeSub(-0.1),
						() => nudgeSub(0.1),
						() => nudgeSub(0),
						pb.subPick === null || !!pb.tracks?.preparing,
					)}
					<div class="style-box">
						<button
							type="button"
							data-tv="1"
							class="style-head"
							aria-expanded={styleOpen}
							onclick={() => (styleOpen = !styleOpen)}
						>
							<span class="style-eyebrow">{i18n.player_style_title()}</span>
							<span class="style-summary">{styleSummary}</span>
							<span class="style-chevron" style:transform={styleOpen ? "rotate(180deg)" : "none"}
								><ChevronDown size={15} strokeWidth={2} /></span
							>
						</button>
						{#if styleOpen}
							{#if styleNote}
								<div class="style-note">{styleNote}</div>
							{/if}
							{@render styleGroup(i18n.player_style_size(), "subSize", ["S", "M", "L", "XL"])}
							{@render styleGroup(i18n.player_style_background(), "subBg", [
								"None",
								"Shadow",
								"Box",
							])}
							{@render styleGroup(i18n.player_style_color(), "subColor", ["White", "Yellow"])}
						{/if}
					</div>
				{/if}
			</div>
		</div>
	{/if}

	{#if skipOn}
		<div
			class="card-slot skip-slot"
			style:bottom="{cardBottom}px"
			transition:pop={{ y: 10, scale: 0.96 }}
		>
			<button type="button" data-tv="1" class="sl-skip" onclick={() => pb.skip()}>
				<ChevronsRight size={18} strokeWidth={2} />
				{pb.introOn ? i18n.player_skip_intro() : i18n.player_skip_credits()}
			</button>
		</div>
	{/if}

	{#if pb.upNextOn && next}
		<div class="card-slot sl-upnext" style:bottom="{cardBottom}px">
			<div class="upnext-art">
				<img
					src={next.still}
					alt=""
					style:object-position={next.stillPos}
					onerror={orPoster(poster)}
				>
				<div class="upnext-fade"></div>
				<div class="upnext-cap">
					<span class="upnext-code">{i18n.player_up_next_card({ code: next.code })}</span>
					<span class="upnext-title">{next.title}</span>
				</div>
				{#if prefs.autoplay}
					<div
						class="upnext-timer"
						role="progressbar"
						aria-label={i18n.player_next_starts_when_filled()}
						aria-valuemin="0"
						aria-valuemax="100"
						aria-valuenow={Math.round(upNextFrac * 100)}
					>
						<div style:transform="scaleX({upNextFrac})"></div>
					</div>
				{/if}
			</div>
			<div class="upnext-row">
				<button
					type="button"
					data-tv="1"
					data-tv-default="1"
					class="btn-ink"
					onclick={() => playNext()}
				>
					<Play size={15} fill="currentColor" /><span>{i18n.player_play_now()}</span>
				</button>
				<button
					type="button"
					data-tv="1"
					class="btn-outline"
					onclick={() => {
						pb.dismissUpNext();
						focusDefault();
					}}
				>
					{i18n.player_watch_credits()}
				</button>
			</div>
		</div>
	{/if}

	{#if pb.stats}
		<div class="sl-stats" style:bottom="{controlsOn ? 220 : 50}px" transition:pop>
			<div class="stats-head">
				<span>{i18n.player_stats_title()}</span>
				<button
					type="button"
					data-tv="1"
					class="stats-close"
					aria-label={i18n.player_close_stats()}
					onclick={() => {
						pb.setStats(false);
						focusDefault();
					}}
				>
					<X size={14} strokeWidth={2} />
				</button>
			</div>
			{#each statRows as r}
				<div class="stats-row">
					<span>{r.k}</span
					><span
						class:why={r.why}
						style:color={r.why ? "var(--fg-subtle)" : (r.c ?? "var(--player-ink-muted)")}
						>{r.v}</span
					>
				</div>
			{/each}
		</div>
	{/if}

	{#if pb.ended}
		<div class="end sl-full" data-tv-trap="1">
			<img
				class="frame"
				src={frame}
				alt=""
				style:object-position={framePos}
				onerror={orPoster(poster)}
			>
			<div class="halt-scrim end-scrim"></div>
			<div class="halt-zoom">
				{#if next}
					<div class="end-next">
						<div class="halt-col end-col">
							<span class="eyebrow" style:color="var(--player-accent-text)"
								>{i18n.player_up_next({ code: next.code })}</span
							>
							<span class="halt-title">{next.title}</span>
							{#if next.overview}
								<span class="end-syn">{next.overview}</span>
							{/if}
							<div class="actions">
								<button
									type="button"
									data-tv="1"
									data-tv-default="1"
									class="primary"
									onclick={onnext}
								>
									<Play size={16} fill="currentColor" />{next.label}
								</button>
								<button type="button" data-tv="1" class="secondary" onclick={exit}>
									{i18n.player_done()}
								</button>
							</div>
						</div>
						<div class="end-still">
							<img
								src={next.still}
								alt=""
								style:object-position={next.stillPos}
								onerror={orPoster(poster)}
							>
						</div>
					</div>
				{:else}
					<div class="end-done-col">
						<div class="halt-col end-col">
							<span class="eyebrow" style:color="var(--player-accent-text)"
								>{isEpisode
									? i18n.player_finished_episode({
											episode: eyebrow.split(" \u00B7 ").at(-1) ?? "",
										})
									: i18n.player_finished()}</span
							>
							<span class="halt-title">{isEpisode ? eyebrow.split(" \u00B7 ")[0] : heading}</span>
							<div class="actions">
								<button
									type="button"
									data-tv="1"
									data-tv-default="1"
									class="primary end-done"
									onclick={exit}
								>
									{i18n.player_done()}
								</button>
								<button
									type="button"
									data-tv="1"
									class="secondary end-again"
									onclick={() => pb.replay()}
								>
									<RotateCcw size={16} strokeWidth={2} />{i18n.player_watch_again()}
								</button>
							</div>
						</div>
						{#if more.length}
							<div class="end-more">
								<span class="more-title">{i18n.player_more_like_this()}</span>
								<div class="more-row">
									{#each more as it (it.href)}
										<button
											type="button"
											data-tv="1"
											class="more-card"
											aria-label={it.title}
											onclick={() => onopen(it.href)}
										>
											<img src={it.poster} alt="" loading="lazy" onerror={hideBroken}>
										</button>
									{/each}
								</div>
							</div>
						{/if}
					</div>
				{/if}
			</div>
		</div>
	{/if}

	{#if pb.halt}
		<HaltScreen
			halt={pb.halt}
			leaving={pb.haltLeaving}
			{frame}
			{poster}
			{framePos}
			backLabel={i18n.player_back_to({ title: titleName })}
			onretry={() => pb.retry()}
			onback={exit}
		/>
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
	background: linear-gradient(
		180deg,
		rgba(0, 0, 0, 0.62) 0%,
		rgba(0, 0, 0, 0) 22%,
		rgba(0, 0, 0, 0) 62%,
		var(--player-scrim) 100%
	);
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
	transition:
		opacity 180ms var(--ease),
		transform 240ms var(--ease);
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
	transition:
		transform var(--dur-slow) var(--ease),
		background-color var(--dur-base),
		opacity var(--dur-base);
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
	font: 400 11.5px / 1.65 var(--font-mono);
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
	background: linear-gradient(
		90deg,
		var(--hero-scrim) 0%,
		color-mix(in srgb, var(--bg-deep) 72%, transparent) 50%,
		color-mix(in srgb, var(--bg-deep) 40%, transparent) 100%
	);
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
	box-shadow:
		0 24px 64px rgba(0, 0, 0, 0.55),
		0 0 0 1px var(--border);
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
	transition:
		transform var(--dur-fast),
		box-shadow var(--dur-fast);
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
</style>
