<script lang="ts">
import {
	Captions,
	CaptionsOff,
	Gauge,
	Maximize,
	Minimize,
	PictureInPicture2,
	Settings,
	AudioLines,
	ArrowLeft,
	Pause,
	Play,
	RotateCcw,
	RotateCw,
} from "@lucide/svelte";
import { digits, languageLabel, modeName } from "#lib/core/playback/names.ts";
import {
	type Launch,
	nowPlaying,
	play,
	setVolume,
	stop,
	toggleMute,
} from "#lib/core/playback/now-playing.svelte.ts";
import { UP_NEXT_SECS } from "#lib/core/playback/playback.svelte.ts";
import { DEFAULT_SIZE_PX, SIZE_PX } from "#lib/core/playback/rules.ts";
import {
	clock,
	endsAt,
	keyFor,
	loadLaunch,
	type PlayerKey,
	type SeekFlash,
	seekFlash,
	segments,
} from "#lib/core/player-view.ts";
import { prefs } from "#lib/core/prefs.svelte.ts";
import type { Kind } from "#lib/core/types.ts";
import { m as i18n } from "#lib/paraglide/messages.js";
import { getLocale } from "#lib/paraglide/runtime.js";
import {
	artwork,
	everything,
	isFullscreen,
	playbackPort,
	setFullscreen,
	title as fetchTitle,
	watchState,
} from "#platform";
import BufferingSpinner from "#lib/ui/BufferingSpinner.svelte";
import Button from "#lib/ui/Button.svelte";
import EmptyState from "#lib/ui/EmptyState.svelte";
import EndScreen from "#lib/ui/EndScreen.svelte";
import { goto } from "#lib/ui/nav.ts";
import Osd from "#lib/ui/Osd.svelte";
import PausedCard from "#lib/ui/PausedCard.svelte";
import PlayerBar from "#lib/ui/PlayerBar.svelte";
import Scrubber from "#lib/ui/Scrubber.svelte";
import SkipButton from "#lib/ui/SkipButton.svelte";
import UpNextCard from "#lib/ui/UpNextCard.svelte";
import { afterNavigate, beforeNavigate } from "$app/navigation";
import { onMount } from "svelte";

type Phase = "idle" | "in" | "out";
type Flash = { ph: Phase; amt: number; rot: number };

const HIDE_MS = 3200;
const OSD_MS = 900;
const MAX_EXTRAPOLATION = 1.5;
const VOLUME_STEP = 0.05;
const CUE_BOTTOM = { shown: 140, hidden: 48 };
const CARD_BOTTOM = { shown: 130, hidden: 40 };

let { kind, id, epId, resume }: { kind: Kind; id: number; epId: number | null; resume: boolean } =
	$props();

let launch = $state.raw<Launch | null>(null);
let failed = $state(false);
let controls = $state(true);
let hovering = $state(false);
let fullscreen = $state(false);
let osd = $state<string | null>(null);
let pf = $state<Phase>("idle");
let pfPause = $state(false);
let skL = $state<Flash>({ ph: "idle", amt: 0, rot: 0 });
let skR = $state<Flash>({ ph: "idle", amt: 0, rot: 0 });
let frameAt = $state(0);

let lastMove = Date.now();
let osdTimer: ReturnType<typeof setTimeout> | undefined;
let pfTimer: ReturnType<typeof setTimeout> | undefined;
let seekTimer: ReturnType<typeof setTimeout> | undefined;
let run: SeekFlash | null = null;
let fromDetail = false;
let leaving = false;

afterNavigate(({ from }) => {
	if (from?.route.id?.startsWith("/detail")) fromDetail = true;
});

// No mini player takes the session over yet, so any way out of the player that is not another player route ends it.
beforeNavigate(({ to }) => {
	if (!to?.route.id?.startsWith("/player")) stop();
});

const pb = $derived(launch && nowPlaying.launch?.key === launch.key ? nowPlaying.pb : null);
const view = $derived(launch?.view ?? null);
const controlsOn = $derived(!!pb && (controls || (!pb.playing && !pb.buffering)));
const showBar = $derived(controlsOn && !pb?.ended);

$effect(() => {
	const [k, i, e, r] = [kind, id, epId, resume];
	let stale = false;
	failed = false;
	leaving = false;
	loadLaunch({ title: fetchTitle, watchState, artwork, everything }, k, i, e, r).then(
		(l) => {
			if (stale) return;
			launch = l;
			startPlayback(l);
		},
		() => {
			if (!stale) failed = true;
		},
	);
	return () => {
		stale = true;
	};
});

function startPlayback(l: Launch) {
	const next = l.view.next;
	// The controller calls onEnd when the file ends, autoplay or not; without autoplay the end screen takes over.
	const onEnd = () => (prefs.autoplay ? goNext() : nowPlaying.pb?.skip());
	play(
		playbackPort,
		{ ...l, item: { ...l.item, onEnd: next ? onEnd : undefined } },
		{
			started: () => show(),
			subtitlesUnavailable: () => flashOsd(i18n.player_subtitles_unavailable()),
		},
	);
	playbackPort.command("viewport", { full: true }).catch(() => {});
}

function show() {
	lastMove = Date.now();
	controls = true;
}

function flashOsd(text: string) {
	clearTimeout(osdTimer);
	osd = text;
	osdTimer = setTimeout(() => (osd = null), OSD_MS);
}

const percent = (v: number) => i18n.player_osd_volume({ percent: Math.round(v * 100) });

function flashPlay(wasPlaying: boolean) {
	pfPause = wasPlaying;
	clearTimeout(pfTimer);
	pf = "in";
	pfTimer = setTimeout(() => {
		pf = "out";
		pfTimer = setTimeout(() => (pf = "idle"), 220);
	}, 520);
}

function togglePlay() {
	if (!pb || pb.ended) return;
	const was = pb.playing;
	pb.togglePlay();
	flashPlay(was);
	show();
}

function seek(delta: number) {
	if (!pb) return;
	pb.seek(delta);
	const left = delta < 0;
	const next = seekFlash(run, left ? "left" : "right", Date.now());
	const cur = left ? skL : skR;
	const continued = run !== null && next.amount > Math.abs(delta) && cur.ph !== "idle";
	run = next;
	const flash: Flash = {
		ph: "in",
		amt: next.amount,
		rot: (continued ? cur.rot : 0) + (left ? -360 : 360),
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
	show();
}

function volume(v: number, silent = false) {
	const level = Math.max(0, Math.min(1, v));
	if (nowPlaying.muted) toggleMute();
	setVolume(level);
	if (!silent) flashOsd(percent(level));
}

function mute() {
	toggleMute();
	flashOsd(nowPlaying.muted ? i18n.player_osd_muted() : percent(nowPlaying.volume));
}

function toggleSubtitles() {
	if (!pb) return;
	if (pb.subPick !== null) {
		pb.pickSub(null);
		flashOsd(i18n.player_osd_subtitles_off());
		return;
	}
	const track = pb.tracks?.subtitles.find((t) => !t.forced) ?? pb.tracks?.subtitles[0];
	if (!track) return flashOsd(i18n.player_subtitles_unavailable());
	pb.pickSub(track);
	flashOsd(i18n.player_osd_subtitles_language({ language: languageLabel(track.language) }));
}

async function toggleFullscreen(on = !fullscreen) {
	await setFullscreen(on).catch(() => {});
	fullscreen = await isFullscreen().catch(() => on);
}

const detail = $derived(`/detail/${kind}/${id}`);

function leaveRoute() {
	stop();
	if (fromDetail) history.back();
	else goto(view?.season ? `${detail}?s=${view.season}` : detail, { replace: true });
}

function exit() {
	if (fullscreen) toggleFullscreen(false);
	leaveRoute();
}

function goNext() {
	const next = view?.next;
	if (!next || leaving) return;
	leaving = true;
	const done = pb?.leave() ?? Promise.resolve();
	done.then(() => goto(next.href, { replace: true }));
}

function goPrevious() {
	const prev = view?.prev;
	if (prev) goto(prev.href, { replace: true });
}

function openMore(href: string) {
	stop();
	goto(href, { replace: true });
}

function onKey(e: KeyboardEvent) {
	if (!pb) return;
	const act = keyFor(
		{
			key: e.key,
			ctrlKey: e.ctrlKey,
			metaKey: e.metaKey,
			altKey: e.altKey,
			target: e.target as Element | null,
		},
		{ ended: pb.ended, menuOpen: false, fullscreen },
	);
	if (!act) return;
	e.preventDefault();
	// A focused button would also click on the key's release.
	if (e.target instanceof HTMLButtonElement) e.target.blur();
	const actions: Record<PlayerKey, () => void> = {
		toggle: togglePlay,
		back10: () => seek(-10),
		forward10: () => seek(10),
		volumeUp: () => {
			volume(Math.round((nowPlaying.volume + VOLUME_STEP) * 20) / 20);
			show();
		},
		volumeDown: () => {
			volume(Math.round((nowPlaying.volume - VOLUME_STEP) * 20) / 20);
			show();
		},
		mute,
		subtitles: toggleSubtitles,
		stats: () => pb?.setStats(!pb.stats),
		next: goNext,
		previous: goPrevious,
		fullscreen: () => toggleFullscreen(),
		closeMenu: () => {},
		exitFullscreen: () => toggleFullscreen(false),
		exit,
	};
	actions[act]();
}

onMount(() => {
	const hide = setInterval(() => {
		if (controls && pb?.playing && !hovering && Date.now() - lastMove > HIDE_MS) controls = false;
	}, 250);
	const sync = () =>
		isFullscreen().then(
			(v) => (fullscreen = v),
			() => {},
		);
	document.addEventListener("fullscreenchange", sync);
	return () => {
		clearInterval(hide);
		clearTimeout(osdTimer);
		clearTimeout(pfTimer);
		clearTimeout(seekTimer);
		document.removeEventListener("fullscreenchange", sync);
		if (fullscreen) setFullscreen(false).catch(() => {});
	};
});

// The player reports every 500 ms; between reports the bar moves on each frame at the playback speed.
$effect(() => {
	if (!pb?.playing || !controlsOn) return;
	let raf = requestAnimationFrame(function tick(t) {
		frameAt = t;
		raf = requestAnimationFrame(tick);
	});
	return () => cancelAnimationFrame(raf);
});

const shown = $derived(
	pb?.playing
		? Math.min(
				pb.duration || Infinity,
				pb.position +
					Math.min(MAX_EXTRAPOLATION, Math.max(0, ((frameAt - pb.positionAt) / 1000) * pb.speed)),
			)
		: (pb?.position ?? 0),
);
const duration = $derived(pb?.duration ?? 0);
const segs = $derived(segments(duration, pb?.tracks?.chapters ?? []));
const paused = $derived(
	!!pb && !pb.playing && !pb.buffering && !pb.ended && !pb.halt && duration > 0,
);
const upNextFrac = $derived(
	!pb || pb.creditsAt === null
		? 0
		: Math.min(1, Math.max(0, (pb.position - pb.creditsAt) / UP_NEXT_SECS)),
);
const skipOn = $derived(
	!!pb && ((pb.introOn && !prefs.autoSkip) || pb.creditsOn) && !pb.ended && !pb.halt,
);
const cardBottom = $derived(controlsOn ? CARD_BOTTOM.shown : CARD_BOTTOM.hidden);
const cueBottom = $derived(controlsOn ? CUE_BOTTOM.shown : CUE_BOTTOM.hidden);

$effect(() => {
	if (!pb) return;
	const px = showBar ? CUE_BOTTOM.shown - CUE_BOTTOM.hidden : 0;
	playbackPort.command("subtitleLift", { px }).catch(() => {});
});

const times = (x: number) =>
	`${new Intl.NumberFormat(getLocale(), { maximumFractionDigits: 2 }).format(x)}×`;
const qualText = $derived(
	!pb || pb.qSel === "auto"
		? i18n.player_auto()
		: pb.qSel === "original"
			? i18n.player_original()
			: i18n.player_mbit({ value: pb.qSel }),
);
const modeLabel = $derived.by(() => {
	const s = pb?.session;
	if (!s) return "";
	const { width, height, hdr, dv_profile } = s.video;
	const size =
		width >= 7000 || height >= 4000 ? "8K" : width >= 3400 || height >= 2000 ? "4K" : `${height}p`;
	if (prefs.techDetails)
		return `${modeName(s.mode)} · ${s.video.action === "copy" ? size : i18n.player_mbit({ value: digits(s.bandwidth / 1_000_000, 1) })}`;
	const range = dv_profile !== undefined ? " · Dolby Vision" : hdr === "sdr" ? "" : " · HDR";
	return `${size}${range}`;
});

const chapterTitles = $derived(pb?.tracks?.chapters ?? []);
const subtitleCue = $derived.by(() => {
	const bg = prefs.subBg;
	return {
		size: SIZE_PX[prefs.subSize] ?? DEFAULT_SIZE_PX,
		color: prefs.subColor === "Yellow" ? "#FDE68A" : "#FFFFFF",
		shadow:
			bg === "Shadow"
				? "0 2px 4px rgba(0,0,0,0.9), 0 0 12px rgba(0,0,0,0.6)"
				: bg === "None"
					? "0 0 2px rgba(0,0,0,0.9)"
					: "none",
		background: bg === "Box" ? "rgba(0,0,0,0.72)" : "transparent",
		padding: bg === "Box" ? "4px 12px" : "0",
	};
});

const menuButtons = $derived.by(() => {
	const label = (name: string, key?: string) => ({
		label: name,
		title: key ? `${name} (${key})` : name,
	});
	return [
		{
			id: "subtitles",
			icon: pb?.subPick === null ? CaptionsOff : Captions,
			...label(i18n.player_subtitles(), "c"),
			active: false,
			onclick: () => {},
		},
		{
			id: "audio",
			icon: AudioLines,
			...label(i18n.player_audio()),
			active: false,
			onclick: () => {},
		},
		{
			id: "quality",
			icon: Settings,
			...label(i18n.player_quality()),
			active: false,
			text: qualText,
			onclick: () => {},
		},
		{
			id: "speed",
			icon: Gauge,
			...label(i18n.player_speed()),
			active: false,
			text: pb && pb.speed !== 1 ? times(pb.speed) : undefined,
			onclick: () => {},
		},
		{
			id: "pip",
			icon: PictureInPicture2,
			...label(i18n.player_pip()),
			active: false,
			disabled: true,
			onclick: () => {},
		},
		{
			id: "fullscreen",
			icon: fullscreen ? Minimize : Maximize,
			label: fullscreen ? i18n.player_fullscreen_exit() : i18n.player_fullscreen(),
			title: `${fullscreen ? i18n.player_fullscreen_exit() : i18n.player_fullscreen()} (f)`,
			active: false,
			onclick: () => toggleFullscreen(),
		},
	];
});

const labels = {
	play: i18n.player_play(),
	pause: i18n.player_pause(),
	back: i18n.player_back_10(),
	forward: i18n.player_forward_10(),
	next: i18n.player_next_episode(),
	mute: i18n.player_mute(),
	volume: i18n.player_volume(),
};
</script>

<svelte:window onkeydown={onKey} onmousemove={show} />

{#if failed}
	<div class="failed">
		<EmptyState icon="info" title={i18n.halt_error_title()}>
			{#snippet action()}
				<Button variant="secondary" onclick={leaveRoute}>{i18n.common_back()}</Button>
			{/snippet}
		</EmptyState>
	</div>
{:else if pb && view && launch}
	<div class="stage" style:cursor={controlsOn || pb.ended ? "default" : "none"}>
		<button
			type="button"
			class="surface"
			tabindex="-1"
			aria-label={pb.playing ? labels.pause : labels.play}
			onclick={togglePlay}
		></button>
		<div class="scrim" style:opacity={showBar ? 1 : 0}></div>

		{#if pb.buffering && !pb.ended}
			<BufferingSpinner label={i18n.player_buffering()} />
		{/if}

		{#if pb.cueText && !paused && !pb.ended}
			<div class="cue-wrap" style:bottom="{cueBottom}px">
				<span
					class="cue"
					style:font-size="{subtitleCue.size}px"
					style:color={subtitleCue.color}
					style:text-shadow={subtitleCue.shadow}
					style:background={subtitleCue.background}
					style:padding={subtitleCue.padding}
					>{pb.cueText}</span
				>
			</div>
		{/if}

		{#if osd}
			<Osd text={osd} />
		{/if}

		<div class="pf" aria-hidden="true">
			<div class="disc {pf}">
				{#if pfPause}
					<Pause size={36} fill="currentColor" strokeWidth={0} />
				{:else}
					<Play size={36} fill="currentColor" strokeWidth={0} />
				{/if}
			</div>
		</div>
		{#each [
			{ f: skL, side: "left" },
			{ f: skR, side: "right" },
		] as s (s.side)}
			<div class="sk {s.side}" aria-hidden="true">
				<div class="disc sdisc {s.f.ph}" style:--dir={s.side === "left" ? -14 : 14}>
					<span
						class="sk-ico"
						style:transform="rotate({s.f.rot}deg)"
						style:transition={s.f.ph === "idle" ? "none" : "transform 450ms var(--ease)"}
					>
						{#if s.side === "left"}
							<RotateCcw size={46} strokeWidth={1.5} />
						{:else}
							<RotateCw size={46} strokeWidth={1.5} />
						{/if}
					</span>
					<span class="sk-amt">{s.f.amt}</span>
				</div>
			</div>
		{/each}

		<div class="chrome" style:opacity={showBar ? 1 : 0}>
			<div class="top" style:pointer-events={showBar ? "auto" : "none"}>
				<button type="button" class="back" aria-label={i18n.common_back()} onclick={exit}>
					<ArrowLeft size={20} strokeWidth={2} />
				</button>
				<div class="heading">
					<span class="name">{launch.item.titleName}</span>
					<span class="sub">{view.sub ?? view.eyebrow}</span>
				</div>
				<div class="grow"></div>
				{#if modeLabel}
					<button
						type="button"
						class="pill"
						class:on={pb.stats}
						aria-label={i18n.player_stats_title()}
						title={`${i18n.player_stats_title()} (i)`}
						onclick={() => pb.setStats(!pb.stats)}
					>
						{#if prefs.techDetails}
							<span
								class="dot"
								style:background={pb.full ? "var(--status-available)" : "var(--status-downloading)"}
							></span>
						{/if}
						{modeLabel}
					</button>
				{/if}
			</div>

			<PausedCard
				on={paused && showBar}
				eyebrow={pb.chapterName
					? i18n.player_paused_in({ chapter: pb.chapterName })
					: i18n.player_paused()}
				title={view.heading}
				text={view.synopsis}
				remaining={i18n.player_paused_left({
					remain: clock(duration - shown, duration),
					time: endsAt(new Date(), duration - shown, pb.speed, getLocale()),
				})}
			/>

			<div class="bottom" style:pointer-events={showBar ? "auto" : "none"}>
				<Scrubber
					{segs}
					chapters={chapterTitles}
					position={shown}
					buffered={pb.buffered}
					{duration}
					still={view.still}
					label={i18n.player_seek()}
					onseek={(at) => pb.seekTo(at)}
					onhovering={(on) => (hovering = on)}
					onactivity={show}
				/>
				<PlayerBar
					playing={pb.playing}
					volume={nowPlaying.volume}
					muted={nowPlaying.muted}
					current={clock(shown, duration)}
					total={clock(duration, duration)}
					chapter={pb.chapterName}
					hasNext={!!view.next}
					buttons={menuButtons}
					{labels}
					ontoggle={togglePlay}
					onseek={seek}
					onnext={goNext}
					onvolume={(v) => volume(v, true)}
					onmute={mute}
				/>
			</div>
		</div>

		<SkipButton
			on={skipOn}
			label={pb.introOn ? i18n.player_skip_intro() : i18n.player_skip_credits()}
			bottom={cardBottom}
			onclick={() => pb.skip()}
		/>

		{#if pb.upNextOn && view.next && !pb.halt}
			<UpNextCard
				code={i18n.player_up_next_card({ code: view.next.code })}
				title={view.next.title}
				still={view.next.still}
				stillPos={view.next.stillPos}
				poster={view.poster}
				bottom={cardBottom}
				progress={upNextFrac}
				timer={prefs.autoplay}
				timerLabel={i18n.player_next_starts_when_filled()}
				playLabel={prefs.autoplay ? i18n.player_play_now() : i18n.player_play_next()}
				creditsLabel={i18n.player_watch_credits()}
				onplay={goNext}
				oncredits={() => pb.dismissUpNext()}
			/>
		{/if}

		{#if pb.ended}
			<EndScreen
				frame={view.still}
				framePos={view.framePos ?? "50% 40%"}
				poster={view.poster}
				next={view.next}
				eyebrow={view.season
					? i18n.player_finished_episode({
							episode: i18n.common_episode_code({
								season: view.season,
								episode: view.episode ?? 0,
							}),
						})
					: i18n.player_finished()}
				title={launch.item.titleName}
				more={view.more}
				labels={{
					upNext: view.next ? i18n.player_up_next({ code: view.next.code }) : "",
					playNext: view.next?.label ?? "",
					done: i18n.player_done(),
					again: i18n.player_watch_again(),
					more: i18n.player_more_like_this(),
				}}
				onnext={goNext}
				ondone={exit}
				onreplay={() => pb.replay()}
				onopen={openMore}
			/>
		{/if}
	</div>
{/if}

<style>
.stage {
	position: fixed;
	inset: 0;
	overflow: hidden;
	color: var(--player-ink);
	user-select: none;
	-webkit-user-select: none;
}

.failed {
	position: fixed;
	inset: 0;
	display: grid;
	place-items: center;
	background: var(--bg-deep);
}

.surface {
	position: absolute;
	inset: 0;
	width: 100%;
	height: 100%;
	padding: 0;
	border: 0;
	background: transparent;
	cursor: inherit;
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

.cue-wrap {
	position: absolute;
	left: 0;
	right: 0;
	display: flex;
	justify-content: center;
	pointer-events: none;
	transition: bottom 250ms var(--ease);
}

.cue {
	display: inline-block;
	max-width: 80%;
	border-radius: var(--radius-sm);
	font-family: var(--font-sans);
	font-weight: 500;
	line-height: 1.3;
	text-align: center;
	white-space: pre-line;
}

.pf {
	position: absolute;
	inset: 0;
	display: grid;
	place-items: center;
	pointer-events: none;
}

.disc {
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

.disc.in {
	opacity: 1;
	transform: none;
	transition:
		opacity 120ms var(--ease),
		transform var(--dur-base) var(--ease);
}

.disc.out {
	transition:
		opacity var(--dur-exit) var(--ease-in),
		transform var(--dur-exit) var(--ease-in);
	transform: scale(1.15);
}

.sdisc.out {
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

.top {
	position: absolute;
	left: 0;
	right: 0;
	top: 0;
	display: flex;
	align-items: center;
	gap: 16px;
	padding: 18px 28px 0;
}

.back {
	width: 44px;
	height: 44px;
	display: grid;
	place-items: center;
	border-radius: 50%;
	border: 0;
	background: var(--player-chip);
	color: var(--player-ink);
	cursor: pointer;
}

.back:hover {
	background: rgba(255, 255, 255, 0.16);
}

.heading {
	display: flex;
	flex-direction: column;
	min-width: 0;
}

.name {
	font-size: 17px;
	font-weight: 600;
	letter-spacing: -0.01em;
}

.sub {
	font-size: 13px;
	color: var(--player-ink-muted);
	white-space: nowrap;
}

.grow {
	flex: 1;
}

.pill {
	height: 34px;
	display: flex;
	align-items: center;
	gap: 7px;
	padding: 0 11px;
	border-radius: var(--radius-full);
	border: 1px solid var(--border-strong);
	background: rgba(0, 0, 0, 0.25);
	color: var(--player-ink);
	font: 500 11.5px var(--font-mono);
	cursor: pointer;
	white-space: nowrap;
}

.pill:hover {
	background: rgba(255, 255, 255, 0.14);
}

.pill.on {
	background: color-mix(in srgb, var(--player-accent) 22%, transparent);
}

.dot {
	width: 7px;
	height: 7px;
	border-radius: 50%;
}

.bottom {
	position: absolute;
	left: 0;
	right: 0;
	bottom: 0;
	padding: 0 28px 14px;
	display: flex;
	flex-direction: column;
	gap: 6px;
}
</style>
