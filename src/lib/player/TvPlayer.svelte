<script lang="ts">
	import "#lib/design/fonts.css";
	import "#lib/design/colors.css";
	import "#lib/design/motion.css";
	import "#lib/design/shape.css";
	import "#lib/design/typography.css";
	import { addPluginListener, invoke } from "@tauri-apps/api/core";
	import { Check, Cpu, Pause, Play, RotateCcw, RotateCw, Server, Settings, TriangleAlert, X } from "@lucide/svelte";
	import { onMount, tick } from "svelte";

	type Reason = { code: string; text: string };
	type Session = {
		session_id: string;
		mode: "direct" | "remux" | "audio_transcode" | "transcode";
		url: string;
		video: { action: string; codec: string; width: number; height: number };
		audio: { codec: string };
		video_reasons: Reason[];
		audio_reasons: Reason[];
	};
	type Quality = "auto" | "original" | number;
	type Halt = {
		eyebrow: string;
		title: string;
		body?: string;
		mono?: string;
		failed: boolean;
		left?: number;
		total?: number;
	};
	type Flash = { ph: "idle" | "in" | "out"; amt: number; rot: number };

	const HIDE_MS = 3200;
	const HEARTBEAT_MS = 10_000;
	const SEEK_STEP = 10;
	const RETRY_SECS = 10;
	const STAGE_WIDTH = 1920;
	const MODE_LABEL = { direct: "Direct play", remux: "Remux", audio_transcode: "Audio conversion", transcode: "Transcode" };

	let media = $state<string[]>([]);
	let mediaIdx = $state(0);
	let session = $state<Session | null>(null);
	let position = $state(0);
	let duration = $state(0);
	let playing = $state(false);
	let buffering = $state(true);
	let controls = $state(true);
	let menu = $state(false);
	let quality = $state<Quality>("auto");
	let halt = $state<Halt | null>(null);
	let zoom = $state(1);
	let skL = $state<Flash>({ ph: "idle", amt: 0, rot: 0 });
	let skR = $state<Flash>({ ph: "idle", amt: 0, rot: 0 });
	let pf = $state<"idle" | "in" | "out">("idle");

	let stage: HTMLElement;
	let capabilities: unknown = null;
	let lastMove = Date.now();
	let retryTimer: ReturnType<typeof setInterval> | undefined;
	let seekTimer: ReturnType<typeof setTimeout> | undefined;
	let pfTimer: ReturnType<typeof setTimeout> | undefined;

	const title = $derived(media[mediaIdx] ?? "");
	const controlsOn = $derived(controls || !playing || menu);
	const pct = $derived(duration > 0 ? (position / duration) * 100 : 0);
	const full = $derived(session?.mode === "direct" || session?.mode === "remux");
	const modeLabel = $derived.by(() => {
		if (!session) return "";
		const h = session.video.height;
		return `${MODE_LABEL[session.mode]} · ${h >= 2160 ? "4K" : `${h}p`}`;
	});
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
				return "Original video and audio, repackaged for this device. Nothing is converted.";
			case "audio_transcode":
				return `Original video. The server converts the audio to ${session.audio.codec} (${why(session.audio_reasons)}).`;
			case "transcode":
				return `The server converts the video to ${session.video.codec} (${why(session.video_reasons)}).`;
		}
	});

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
		const root = stage?.querySelector<HTMLElement>("[data-tv-trap]") ?? stage;
		root?.querySelector<HTMLElement>("[data-tv-default]")?.focus();
	}

	function clearHalt() {
		clearInterval(retryTimer);
		halt = null;
	}

	function haltWithRetry(h: Halt, secs: number) {
		clearInterval(retryTimer);
		halt = { ...h, left: secs, total: secs };
		retryTimer = setInterval(() => {
			if (!halt?.left) return;
			if (halt.left <= 1) start();
			else halt = { ...halt, left: halt.left - 1 };
		}, 1000);
		focusDefault();
	}

	async function start(at = 0) {
		clearHalt();
		buffering = true;
		try {
			capabilities ??= await player("capabilities");
			if (!media.length) {
				const status = await invoke<{ media: string[] }>("playback_status");
				media = status.media;
			}
			session = await invoke<Session>("start_session", { mediaId: title, capabilities, position: at });
			await player("load", { url: session.url });
			await player("play");
			showControls(true);
		} catch (e) {
			buffering = false;
			failedToStart(String(e));
		}
	}

	function failedToStart(raw: string) {
		let error: { code?: string; message?: string; retry_after?: number } | null = null;
		try {
			error = JSON.parse(raw);
		} catch {
			// Not JSON: the request never got an answer from the server.
		}
		if (error?.code === "transcode_limit") {
			haltWithRetry(
				{
					eyebrow: "Server busy",
					title: "Waiting for a free slot",
					body: "All conversions on the server are in use, and this stream needs one.",
					failed: false,
				},
				error.retry_after ?? RETRY_SECS,
			);
		} else if (error) {
			halt = {
				eyebrow: "Playback error",
				title: "This can't play right now",
				mono: error.message ?? error.code ?? raw,
				failed: true,
			};
			focusDefault();
		} else {
			haltWithRetry(
				{
					eyebrow: "Can't reach the server",
					title: "The server isn't answering",
					body: "The server may be off or restarting, or this device is away from your home network without the VPN.",
					failed: true,
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

	// ponytail: no library screen yet, so leaving the player moves on to the mock's next clip.
	async function exit() {
		clearHalt();
		await stop();
		if (media.length) mediaIdx = (mediaIdx + 1) % media.length;
		quality = "auto";
		position = 0;
		duration = 0;
		await start();
	}

	function togglePlay() {
		const wasPlaying = playing;
		player(wasPlaying ? "pause" : "play");
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
		const dir = delta < 0;
		const cur = dir ? skL : skR;
		const next: Flash = {
			ph: "in",
			amt: (cur.ph === "idle" ? 0 : cur.amt) + Math.abs(delta),
			rot: (cur.ph === "idle" ? 0 : cur.rot) + (dir ? -360 : 360),
		};
		if (dir) skL = next;
		else skR = next;
		clearTimeout(seekTimer);
		seekTimer = setTimeout(() => {
			skL = { ...skL, ph: skL.ph === "idle" ? "idle" : "out" };
			skR = { ...skR, ph: skR.ph === "idle" ? "idle" : "out" };
			seekTimer = setTimeout(() => {
				skL = { ph: "idle", amt: 0, rot: 0 };
				skR = { ph: "idle", amt: 0, rot: 0 };
			}, 220);
		}, 720);
	}

	async function pickQuality(q: Quality) {
		menu = false;
		if (q === quality || !session) return;
		quality = q;
		const at = position;
		const maxBitrate = typeof q === "number" ? q * 1_000_000 : null;
		buffering = true;
		try {
			session = await invoke<Session>("patch_session", {
				id: session.session_id,
				changes: { max_bitrate: maxBitrate, position: at },
			});
			await player("load", { url: session.url });
			await player("seek", { seconds: at });
			await player("play");
		} catch (e) {
			buffering = false;
			failedToStart(String(e));
		}
		focusDefault();
	}

	function openMenu() {
		menu = true;
		focusDefault();
	}

	function back() {
		if (halt) return exit();
		if (menu) {
			menu = false;
			return focusDefault();
		}
		if (controls && playing) {
			controls = false;
			return;
		}
		exit();
	}

	function moveFocus(key: string) {
		const root = stage.querySelector<HTMLElement>("[data-tv-trap]") ?? stage;
		const from = document.activeElement as HTMLElement | null;
		const els = [...root.querySelectorAll<HTMLElement>("[data-tv]:not([disabled])")].filter((e) => e !== from);
		if (!from || !root.contains(from)) return els[0]?.focus();
		const a = from.getBoundingClientRect();
		const [ax, ay] = [a.left + a.width / 2, a.top + a.height / 2];
		const horizontal = key === "ArrowLeft" || key === "ArrowRight";
		const sign = key === "ArrowLeft" || key === "ArrowUp" ? -1 : 1;
		let best: HTMLElement | null = null;
		let bestScore = Infinity;
		for (const el of els) {
			const b = el.getBoundingClientRect();
			const [dx, dy] = [b.left + b.width / 2 - ax, b.top + b.height / 2 - ay];
			const along = (horizontal ? dx : dy) * sign;
			if (along <= 1) continue;
			const score = along + 2 * Math.abs(horizontal ? dy : dx);
			if (score < bestScore) [best, bestScore] = [el, score];
		}
		best?.focus();
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
		if (halt || menu) {
			if (arrow) {
				e.preventDefault();
				moveFocus(k);
			}
			return;
		}
		if (!controlsOn) {
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
			moveFocus(k);
		}
	}

	onMount(() => {
		const resize = () => (zoom = window.innerWidth / STAGE_WIDTH);
		resize();
		window.addEventListener("resize", resize);
		const listeners = [
			addPluginListener<{ position: number; duration: number }>("player", "position", (e) => {
				position = e.position;
				duration = e.duration;
			}),
			addPluginListener<{ state: string }>("player", "state", (e) => {
				buffering = e.state === "buffering";
				playing = e.state === "playing";
			}),
			addPluginListener<{ code: string; message: string }>("player", "error", (e) => {
				buffering = false;
				halt = { eyebrow: "Playback error", title: "This stream stopped", mono: `${e.code}: ${e.message}`, failed: true };
				focusDefault();
			}),
			addPluginListener("player", "back", back),
		];
		start();
		const beat = setInterval(() => {
			if (session) invoke("heartbeat", { id: session.session_id, position, paused: !playing }).catch(() => {});
		}, HEARTBEAT_MS);
		const hide = setInterval(() => {
			if (controls && playing && !menu && Date.now() - lastMove > HIDE_MS) controls = false;
		}, 250);
		return () => {
			clearInterval(beat);
			clearInterval(hide);
			clearInterval(retryTimer);
			window.removeEventListener("resize", resize);
			listeners.forEach((l) => l.then((h) => h.unregister()));
			stop();
		};
	});
</script>

<svelte:window onkeydown={onKey} />

{#snippet flash(f: Flash, side: "left" | "right")}
	<div class="sk {side}" aria-hidden="true">
		<div class="sk-disc" class:on={f.ph === "in"} style:--dir={side === "left" ? -14 : 14}>
			<span class="sk-ico" style:transform="rotate({f.rot}deg)" style:transition={f.ph === "idle" ? "none" : "transform 450ms var(--ease)"}>
				{#if side === "left"}<RotateCcw size={46} strokeWidth={1.5} />{:else}<RotateCw size={46} strokeWidth={1.5} />{/if}
			</span>
			<span class="sk-amt">{f.amt}</span>
		</div>
	</div>
{/snippet}

{#snippet option(label: string, q: Quality)}
	<button data-tv="1" data-tv-default={quality === q ? "1" : undefined} class="opt" class:sel={quality === q} onclick={() => pickQuality(q)}>
		<span class="opt-check" style:opacity={quality === q ? 1 : 0}><Check size={16} strokeWidth={2.5} /></span>
		<span class="opt-label" style:font-weight={quality === q ? 600 : 500}>{label}</span>
	</button>
{/snippet}

<div class="stage" bind:this={stage} style:zoom>
	<div class="scrim" style:opacity={controlsOn ? 1 : 0}></div>

	{#if buffering}<div class="sl-spin" role="status" aria-label="Buffering"></div>{/if}

	<div class="pf" aria-hidden="true">
		<div class="pf-disc" class:on={pf === "in"}>
			{#if playing}<Pause size={36} fill="currentColor" />{:else}<Play size={36} fill="currentColor" />{/if}
		</div>
	</div>
	{@render flash(skL, "left")}
	{@render flash(skR, "right")}

	<div class="chrome" style:opacity={controlsOn ? 1 : 0}>
		{#if controlsOn}
			<div class="controls">
				<div class="head">
					<span class="tv-title">{title}</span>
					<span class="mode"><span class="dot" style:background={full ? "var(--status-available)" : "var(--status-downloading)"}></span>{modeLabel}</span>
				</div>
				<button data-tv="1" data-scrub="1" class="scrub" aria-label="Seek">
					<div class="track"><div class="seg"><div class="fill" style:width="{pct}%"></div></div></div>
					<div class="thumb" style:left="{pct}%"></div>
				</button>
				<div class="times"><span class="cur">{fmt(position)}</span><span>-{fmt(duration - position)}</span></div>
				<div class="row">
					<button data-tv="1" data-tv-default="1" class="play" aria-label={playing ? "Pause" : "Play"} onclick={togglePlay}>
						{#if playing}<Pause size={36} fill="currentColor" />{:else}<Play size={36} fill="currentColor" />{/if}
					</button>
					<button data-tv="1" class="pill" onclick={openMenu}><Settings size={28} strokeWidth={1.9} />{qualText}</button>
				</div>
			</div>
		{/if}
	</div>

	{#if menu}
		<div class="menu-scrim"></div>
		<div class="menu" data-tv-trap="1">
			<div class="menu-head">
				<span class="menu-title">Quality</span>
				<div class="grow"></div>
				<button data-tv="1" class="close" aria-label="Close" onclick={() => { menu = false; focusDefault(); }}><X size={16} strokeWidth={2} /></button>
			</div>
			<div class="menu-body">
				<div class="note"><span class="note-dot" style:background={full ? "var(--status-available)" : "var(--status-downloading)"}></span><span>{menuNote}</span></div>
				{@render option("Auto", "auto")}
				{@render option("Original", "original")}
				{#each [20, 8, 4, 2, 1] as n}{@render option(`${n} Mbit/s`, n)}{/each}
			</div>
		</div>
	{/if}

	{#if halt}
		<div class="halt" role="alertdialog" aria-label={halt.eyebrow} data-tv-trap="1">
			<div class="halt-scrim"></div>
			<div class="halt-zoom">
				<div class="halt-col">
					<span class="eyebrow" style:color={halt.failed ? "var(--status-failed)" : "var(--status-warning)"}>
						{#if halt.failed}<Server size={14} strokeWidth={2} />{:else}<Cpu size={14} strokeWidth={2} />{/if}
						{halt.eyebrow}
					</span>
					<span class="halt-title">{halt.title}</span>
					{#if halt.body}<span class="halt-body">{halt.body}</span>{/if}
					{#if halt.mono}<span class="halt-mono"><TriangleAlert size={12} /> {halt.mono}</span>{/if}
					{#if halt.left && halt.total}
						<div class="count">
							<div class="count-track"><div class="count-fill" style:width="{(halt.left / halt.total) * 100}%" style:background={halt.failed ? "var(--player-ink-muted)" : "var(--status-warning)"}></div></div>
							<span class="count-text">Trying again in {halt.left} s</span>
						</div>
					{/if}
					<div class="actions">
						<button data-tv="1" data-tv-default="1" class="primary" onclick={() => start()}><RotateCw size={18} strokeWidth={2} />Try now</button>
						<button data-tv="1" class="secondary" onclick={exit}>Back</button>
					</div>
				</div>
			</div>
		</div>
	{/if}
</div>

<style>
	:global(html),
	:global(body) {
		margin: 0;
		height: 100%;
		overflow: hidden;
		background: transparent;
		color: var(--player-ink);
		font-family: var(--font-sans);
	}

	button {
		font-family: inherit;
	}

	.stage {
		position: fixed;
		left: 0;
		top: 0;
		width: 1920px;
		height: 1080px;
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
		box-sizing: border-box;
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
		transition:
			opacity var(--dur-exit),
			transform var(--dur-exit);
	}

	.pf-disc {
		transform: scale(1.15);
	}

	.pf-disc.on,
	.sk-disc.on {
		opacity: 1;
		transform: none;
		transition:
			opacity var(--dur-fast) var(--ease),
			transform var(--dur-fast) var(--ease);
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

	.sk-disc {
		transform: translateX(calc(var(--dir) * 1px));
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

	.dot {
		width: 12px;
		height: 12px;
		border-radius: 50%;
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
		box-shadow: 0 0 0 0 transparent;
		transition: box-shadow var(--dur-fast);
	}

	.scrub:focus .thumb {
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
		font-size: 14px;
	}

	.halt {
		position: absolute;
		inset: 0;
		overflow: hidden;
		background: var(--bg-deep);
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
