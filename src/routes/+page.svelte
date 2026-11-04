<script lang="ts">
	import { addPluginListener, invoke } from "@tauri-apps/api/core";
	import { onMount } from "svelte";

	// Clip ids served by dev/mock-streamline.py.
	const MEDIA = ["demo", "hdr", "anime", "dv"];
	const HEARTBEAT_MS = 10_000;

	type Session = {
		session_id: string;
		mode: string;
		url: string;
		video: Record<string, unknown>;
		audio: Record<string, unknown>;
		video_reasons: { code: string }[];
		audio_reasons: { code: string }[];
	};

	let position = $state(0);
	let duration = $state(0);
	let playerState = $state("idle");
	let error = $state("");
	let log = $state<string[]>([]);
	let panelHidden = $state(false);
	let session = $state<Session | null>(null);
	let capabilities: unknown = null;
	let playButton: HTMLButtonElement;

	const fmt = (s: number) =>
		`${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

	function note(line: string) {
		log = [line, ...log].slice(0, 6);
		console.log("[proto]", line);
	}

	async function call(cmd: string, args?: Record<string, unknown>) {
		try {
			await invoke(`plugin:player|${cmd}`, args);
			note(`${cmd} ok`);
		} catch (e) {
			error = String(e);
			note(`${cmd} failed: ${e}`);
		}
	}

	async function stop() {
		if (!session) return;
		const id = session.session_id;
		session = null;
		await invoke("stop_session", { id }).catch((e) => note(`stop failed: ${e}`));
	}

	async function load(media: string) {
		error = "";
		await stop();
		try {
			capabilities ??= await invoke("plugin:player|capabilities");
			session = await invoke<Session>("start_session", { mediaId: media, capabilities, position: 0 });
			note(`${media}: ${session.mode} ${[...session.video_reasons, ...session.audio_reasons].map((r) => r.code).join(",")}`);
			await call("load", { url: session.url });
			await call("play");
		} catch (e) {
			error = String(e);
			note(`start failed: ${e}`);
		}
	}

	const togglePlay = () => call(playerState === "playing" ? "pause" : "play");
	const seekBy = (delta: number) =>
		call("seek", { seconds: Math.max(0, position + delta) });

	onMount(() => {
		playButton.focus();
		(async () => {
			await addPluginListener<{ position: number; duration: number }>(
				"player",
				"position",
				(e) => {
					position = e.position;
					duration = e.duration;
				},
			);
			await addPluginListener<{ state: string }>("player", "state", (e) => {
				playerState = e.state;
				note(`state ${e.state}`);
			});
			await addPluginListener<{ code: string; message: string }>(
				"player",
				"error",
				(e) => {
					error = `${e.code}: ${e.message}`;
					note(`error ${e.code}`);
				},
			);
			console.log("[proto] capabilities", JSON.stringify(await invoke("plugin:player|capabilities")));
			await load("demo");
		})();
		const beat = setInterval(() => {
			if (session)
				invoke("heartbeat", { id: session.session_id, position, paused: playerState !== "playing" }).catch((e) =>
					note(`heartbeat failed: ${e}`),
				);
		}, HEARTBEAT_MS);
		return () => {
			clearInterval(beat);
			stop();
		};
	});
</script>

<svelte:window
	onkeydown={(e) => {
		note(`key ${e.key}`);
		// Android WebView has no spatial navigation: arrows arrive as plain keydowns.
		const step = { ArrowLeft: -1, ArrowRight: 1 }[e.key];
		if (!step) return;
		const buttons = [...document.querySelectorAll<HTMLButtonElement>(".buttons button")];
		const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
		buttons[Math.min(buttons.length - 1, Math.max(0, i + step))]?.focus();
		e.preventDefault();
	}}
/>

<div class="spinner" aria-hidden="true"></div>

<div class="hud">
	<div>state: <b data-testid="state">{playerState}</b></div>
	<div>
		<b data-testid="position">{fmt(position)}</b> / {fmt(duration)}
	</div>
	{#if session}<div class="mode">{session.mode} · {JSON.stringify(session.video)}</div>{/if}
	{#if error}<div class="error">{error}</div>{/if}
	<ul>
		{#each log as line}<li>{line}</li>{/each}
	</ul>
</div>

<div class="panel" class:hidden={panelHidden}>
	<div class="bar"><div style:width="{duration ? (position / duration) * 100 : 0}%"></div></div>
	<div class="buttons">
		<button onclick={() => seekBy(-10)}>−10 s</button>
		<button bind:this={playButton} onclick={togglePlay}>
			{playerState === "playing" ? "Pause" : "Play"}
		</button>
		<button onclick={() => seekBy(10)}>+10 s</button>
		{#each MEDIA as m}<button onclick={() => load(m)}>{m}</button>{/each}
		<button onclick={() => (panelHidden = !panelHidden)}>Fade panel</button>
	</div>
</div>

<style>
	:global(html),
	:global(body) {
		background: transparent;
		margin: 0;
		height: 100%;
		color: white;
		font-family: sans-serif;
	}

	.spinner {
		position: fixed;
		top: 40px;
		right: 40px;
		width: 80px;
		height: 80px;
		border-radius: 12px;
		background: linear-gradient(45deg, #f0f, #0ff);
		animation: spin 2s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.hud {
		position: fixed;
		top: 24px;
		left: 24px;
		padding: 12px 16px;
		background: rgb(0 0 0 / 0.55);
		border-radius: 8px;
		font-size: 20px;
	}

	.hud ul {
		margin: 8px 0 0;
		padding: 0;
		list-style: none;
		font-size: 14px;
		opacity: 0.8;
	}

	.mode {
		font-size: 14px;
		opacity: 0.8;
	}

	.error {
		color: #ff6b6b;
	}

	.panel {
		position: fixed;
		left: 48px;
		right: 48px;
		bottom: 40px;
		padding: 20px;
		border-radius: 12px;
		background: rgb(0 0 0 / 0.6);
		transition:
			opacity 600ms ease,
			transform 600ms ease;
	}

	.panel.hidden {
		opacity: 0.15;
		transform: translateY(60%);
	}

	.bar {
		height: 6px;
		background: rgb(255 255 255 / 0.25);
		border-radius: 3px;
		margin-bottom: 16px;
	}

	.bar div {
		height: 100%;
		background: #e50914;
		border-radius: 3px;
		transition: width 500ms linear;
	}

	.buttons {
		display: flex;
		gap: 16px;
		justify-content: center;
	}

	button {
		font-size: 22px;
		padding: 12px 28px;
		border-radius: 8px;
		border: 3px solid transparent;
		background: rgb(255 255 255 / 0.15);
		color: white;
		transition:
			transform 150ms ease,
			background 150ms ease;
	}

	button:focus {
		outline: none;
		border-color: white;
		background: rgb(255 255 255 / 0.35);
		transform: scale(1.08);
	}
</style>
