<script lang="ts">
	import { goto } from "$app/navigation";
	import { focusDefault } from "#lib/tv/focus.ts";
	import TvScreen from "#lib/tv/TvScreen.svelte";
	import { invoke } from "@tauri-apps/api/core";
	import { Check, CircleCheck } from "@lucide/svelte";
	import { onMount, tick } from "svelte";

	type Step = "server" | "pair" | "paired";
	type Pairing = { code: string; name: string; uri: string; expires_in: number; interval: number; qr: string };

	const FOUND_PAUSE_MS = 750;
	const APPROVED_PAUSE_MS = 700;
	const SLOW_DOWN_SECS = 5;

	const row = (s: string) => s.split(" ").map((k) => ({ label: k, aria: k, type: k, wide: 0 }));
	const KEY_ROWS = [
		row("a b c d e f g h i j"),
		row("k l m n o p q r s t"),
		row("u v w x y z 1 2 3 4"),
		row("5 6 7 8 9 0 . - : /"),
		[
			{ label: "⌫", aria: "Delete", type: "", wide: 212 },
			{ label: "Clear", aria: "Clear", type: "", wide: 212 },
			{ label: ".home.arpa", aria: "home.arpa", type: ".home.arpa", wide: 280 },
		],
	];

	let step = $state<Step | null>(null);
	let url = $state("");
	let checking = $state(false);
	let found = $state(false);
	let error = $state("");

	let pairing = $state<Pairing | null>(null);
	let left = $state(0);
	let status = $state<"waiting" | "approved" | "denied" | "failed">("waiting");
	let failure = $state("");
	let account = $state({ name: "", short: "" });
	let keystore = $state("");

	let stage = $state<HTMLElement>();
	let pollTimer: ReturnType<typeof setTimeout> | undefined;
	let generation = 0;

	const host = $derived(url.replace(/^[a-z]+:\/\//, "").replace(/\/.*$/, ""));
	const pairPage = $derived(pairing?.uri.replace(/^[a-z]+:\/\//, "") ?? `${host}/pair`);
	const chars = $derived(pairing ? [...pairing.code.toUpperCase()] : []);
	const expires = $derived(`${Math.floor(left / 60)}:${String(left % 60).padStart(2, "0")}`);
	const STATUS = {
		waiting: { text: "Waiting for approval…", color: "var(--accent)", pulse: true },
		approved: { text: "Approved", color: "var(--status-available)", pulse: false },
		denied: { text: "Request denied", color: "var(--status-failed)", pulse: false },
		failed: { text: "", color: "var(--status-failed)", pulse: false },
	};
	const shown = $derived({ ...STATUS[status], text: status === "failed" ? failure : STATUS[status].text });

	const TITLES = { server: "Connect to your server", pair: "Pair this TV", paired: "This TV is paired" };
	const STEPS = { server: "Step 1 of 2", pair: "Step 2 of 2", paired: "Done" };
	const lead = $derived(
		step === "server"
			? "Enter the address you use for the Streamline web app."
			: step === "pair"
				? "On your phone, open the pairing page and enter this code. You only need to do this once."
				: `Signed in as ${account.short}. Nobody typed a password on this TV.`,
	);
	const tvHelp = $derived(error || (found ? "Streamline server found" : checking ? "Checking…" : "Type with the remote, or press Continue."));

	async function go(next: Step) {
		step = next;
		await tick();
		if (stage) focusDefault(stage);
	}

	function type(k: { aria: string; type: string }) {
		if (k.aria === "Delete") url = url.slice(0, -1);
		else if (k.aria === "Clear") url = "";
		else url += k.type;
		found = false;
		error = "";
	}

	async function submit() {
		if (checking || found) return;
		const typed = url.trim();
		if (!typed) return (error = "Enter your server’s address");
		if (!/^([a-z]+:\/\/)?[a-z0-9.-]+(:\d+)?(\/.*)?$/i.test(typed)) return (error = "That doesn’t look like an address");
		checking = true;
		error = "";
		try {
			await invoke("connect", { url: typed });
			found = true;
			setTimeout(() => go("pair").then(newCode), FOUND_PAUSE_MS);
		} catch (e) {
			error = String(e);
		} finally {
			checking = false;
		}
	}

	async function newCode() {
		const id = ++generation;
		clearTimeout(pollTimer);
		status = "waiting";
		try {
			const p = await invoke<Pairing>("pair_start");
			if (id !== generation) return;
			pairing = p;
			left = p.expires_in;
			poll(id, p.interval);
		} catch (e) {
			if (id !== generation) return;
			failure = /"code"/.test(String(e)) ? "The server refused to start pairing" : `Can’t reach ${host}`;
			status = "failed";
		}
	}

	function poll(id: number, interval: number) {
		pollTimer = setTimeout(async () => {
			const answer = await invoke<string>("pair_poll").catch(() => "pending");
			if (id !== generation) return;
			if (answer === "paired") return approved();
			if (answer === "expired") return newCode();
			if (answer === "denied") return (status = "denied");
			poll(id, answer === "slow_down" ? interval + SLOW_DOWN_SECS : interval);
		}, interval * 1000);
	}

	async function approved() {
		status = "approved";
		const [me, state] = await Promise.all([
			invoke<{ name: string }>("me").catch(() => ({ name: "" })),
			invoke<{ keystore: string }>("auth_state"),
		]);
		account = { name: me.name, short: me.name.split(" ")[0] ?? "" };
		keystore = state.keystore;
		setTimeout(() => go("paired"), APPROVED_PAUSE_MS);
	}

	function changeServer() {
		generation++;
		clearTimeout(pollTimer);
		found = false;
		go("server");
	}

	onMount(() => {
		invoke<{ server: string | null; paired: boolean }>("auth_state").then((s) => {
			url = (s.server ?? "").replace(/^https:\/\//, "");
			if (s.paired) goto("/", { replace: true });
			else if (s.server) go("pair").then(newCode);
			else go("server");
		});
		const clock = setInterval(() => {
			if (step !== "pair" || status !== "waiting" || !pairing) return;
			if (left <= 1) newCode();
			else left--;
		}, 1000);
		return () => {
			generation++;
			clearTimeout(pollTimer);
			clearInterval(clock);
		};
	});
</script>

<TvScreen onback={step === "pair" ? changeServer : () => {}}>
	<div class="glow"></div>
	{#if step}
		<div class="page" bind:this={stage}>
			<div class="side">
				<div class="brand">
					<img class="mark" src="/streamline-mark.png" alt="" />
					<span class="eyebrow">{STEPS[step]}</span>
				</div>
				<h1>{TITLES[step]}</h1>
				<p class="lead">{lead}</p>

				{#if step === "server"}
					<div class="field" class:error>
						{#if !url.includes("://")}<span class="faint">https://</span>{/if}<span class="url">{url}</span><span class="caret"></span>
					</div>
					<div class="help" class:error>
						{#if found}<CircleCheck size={28} strokeWidth={2} color="var(--status-available)" />{/if}
						<span>{tvHelp}</span>
					</div>
					<div class="actions">
						<button data-tv="1" data-tv-default="1" class="tvbtn" onclick={submit}>{checking ? "Checking…" : found ? "Connected" : "Continue"}</button>
					</div>
				{:else if step === "pair"}
					<div class="code">
						{#each chars as ch}
							{#if ch === "-"}<span class="dash">–</span>{:else}<span class="ch" class:ok={status === "approved"}>{ch}</span>{/if}
						{/each}
					</div>
					<div class="status">
						<span class="dot" class:pulse={shown.pulse} style:background={shown.color}></span>
						<span>{shown.text}</span>
						{#if pairing && status === "waiting"}<span class="expires">expires in {expires}</span>{/if}
					</div>
					<div class="actions">
						<button data-tv="1" data-tv-default="1" class="tvbtn" onclick={newCode}>Get a new code</button>
						<button data-tv="1" class="tvbtn" onclick={changeServer}>Change server</button>
					</div>
				{:else}
					<div class="facts">
						<div><span>Account</span><span>{account.name}</span></div>
						<div><span>This TV</span><span>{pairing?.name}</span></div>
						<div><span>Key stored in</span><span>{keystore}</span></div>
					</div>
					<div><button data-tv="1" data-tv-default="1" class="tvbtn" onclick={() => goto("/", { replace: true })}>Start watching</button></div>
				{/if}
			</div>

			<div class="aside">
				{#if step === "server"}
					<div class="keyboard">
						{#each KEY_ROWS as keys}
							<div class="keys">
								{#each keys as k}
									<button data-tv="1" class="tvkey" aria-label={k.aria} style:width={k.wide ? `${k.wide}px` : undefined} onclick={() => type(k)}>{k.label}</button>
								{/each}
							</div>
						{/each}
					</div>
				{:else if step === "pair"}
					<div class="qr">
						{#if pairing}<img src="data:image/svg+xml,{encodeURIComponent(pairing.qr)}" alt="QR code for the pairing page" />{/if}
					</div>
					<span class="scan">Scan with your phone, or open<br /><span class="mono">{pairPage}</span></span>
				{:else}
					<span class="done"><Check size={110} strokeWidth={2} /></span>
				{/if}
			</div>
		</div>
	{/if}
</TvScreen>

<style>
	.glow {
		position: absolute;
		left: 50%;
		top: -260px;
		width: 1400px;
		height: 1400px;
		margin-left: -700px;
		border-radius: 50%;
		background: radial-gradient(closest-side, color-mix(in srgb, var(--accent) 22%, transparent), color-mix(in srgb, var(--accent) 6%, transparent) 55%, transparent 100%);
		pointer-events: none;
	}

	.page {
		position: absolute;
		inset: 0;
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
		gap: 96px;
		align-items: center;
		padding: 96px 140px;
		color: var(--fg);
	}

	.side {
		display: flex;
		flex-direction: column;
		gap: 30px;
	}

	.brand {
		display: flex;
		align-items: center;
		gap: 20px;
	}

	.mark {
		width: 72px;
		height: 72px;
		border-radius: 18px;
		display: block;
		flex: none;
	}

	.eyebrow {
		font: 500 22px var(--font-mono);
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--fg-subtle);
	}

	h1 {
		margin: 0;
		font-size: 68px;
		line-height: 1.05;
		font-weight: 700;
		letter-spacing: -0.03em;
	}

	.lead {
		margin: 0;
		font-size: 28px;
		line-height: 1.5;
		color: var(--fg-muted);
	}

	.field {
		height: 92px;
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 0 28px;
		border-radius: 18px;
		background: var(--bg-elevated);
		border: 2px solid color-mix(in srgb, var(--accent) 60%, transparent);
		font: 400 32px var(--font-mono);
		white-space: pre;
		overflow: hidden;
	}

	.field.error {
		border-color: var(--status-failed);
	}

	.faint {
		color: var(--fg-faint);
	}

	.caret {
		flex: none;
		width: 3px;
		height: 40px;
		background: var(--accent);
		animation: pulse 1s steps(1) infinite;
	}

	.help {
		display: flex;
		align-items: center;
		gap: 16px;
		font-size: 24px;
		color: var(--fg-subtle);
	}

	.help.error {
		color: var(--status-failed);
	}

	.actions {
		display: flex;
		gap: 18px;
	}

	.tvbtn {
		height: 72px;
		padding: 0 36px;
		border: 0;
		border-radius: 18px;
		background: rgba(255, 255, 255, 0.1);
		color: var(--player-ink);
		font: 500 26px var(--font-sans);
		cursor: pointer;
		outline: none;
		transition: transform 150ms;
	}

	.tvbtn:focus {
		background: var(--player-ink);
		color: #0b0b10;
		transform: scale(1.06);
	}

	.code {
		display: flex;
		gap: 10px;
	}

	.ch,
	.dash {
		height: 128px;
		display: grid;
		place-items: center;
		font: 500 68px var(--font-mono);
	}

	.ch {
		width: 96px;
		border-radius: var(--radius-xl);
		background: var(--bg-deep);
		border: 2px solid var(--border-strong);
		color: var(--fg);
	}

	.ch.ok {
		background: color-mix(in srgb, var(--status-available) 10%, transparent);
		border-color: color-mix(in srgb, var(--status-available) 35%, transparent);
		color: var(--status-available);
	}

	.dash {
		width: 34px;
		color: var(--fg-faint);
	}

	.status {
		display: flex;
		align-items: center;
		gap: 16px;
		font-size: 26px;
	}

	.dot {
		width: 14px;
		height: 14px;
		border-radius: 50%;
	}

	.dot.pulse {
		animation: pulse 1.4s ease-in-out infinite;
	}

	.expires {
		margin-left: auto;
		font: 400 22px var(--font-mono);
		color: var(--fg-subtle);
	}

	.facts {
		display: flex;
		flex-direction: column;
		border-radius: var(--radius-xl);
		border: 1px solid var(--border);
		background: var(--bg-elevated);
		font-size: 26px;
	}

	.facts > div {
		display: flex;
		justify-content: space-between;
		gap: 24px;
		padding: 22px 28px;
	}

	.facts > div + div {
		border-top: 1px solid var(--border);
	}

	.facts > div > span:first-child {
		color: var(--fg-subtle);
	}

	.aside {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 24px;
	}

	.keyboard {
		display: flex;
		flex-direction: column;
		gap: 12px;
		padding: 28px;
		border-radius: 24px;
		background: color-mix(in srgb, var(--bg-elevated) 85%, transparent);
		border: 1px solid var(--border);
	}

	.keys {
		display: flex;
		gap: 10px;
		justify-content: center;
	}

	.tvkey {
		display: grid;
		place-items: center;
		width: 64px;
		height: 68px;
		padding: 0;
		border: 0;
		border-radius: 14px;
		background: rgba(255, 255, 255, 0.07);
		color: var(--player-ink);
		font: 500 28px var(--font-mono);
		cursor: pointer;
		outline: none;
		transition: transform 150ms;
	}

	.tvkey:focus {
		background: var(--player-ink);
		color: #0b0b10;
		transform: scale(1.1);
	}

	.qr {
		width: 420px;
		height: 420px;
		padding: 28px;
		border-radius: 28px;
		background: var(--player-ink);
		box-shadow: 0 24px 64px rgba(0, 0, 0, 0.5);
	}

	.qr img {
		width: 100%;
		height: 100%;
		display: block;
		image-rendering: pixelated;
	}

	.scan {
		font-size: 26px;
		color: var(--fg-muted);
		text-align: center;
	}

	.mono {
		font-family: var(--font-mono);
		color: var(--fg);
	}

	.done {
		width: 220px;
		height: 220px;
		display: grid;
		place-items: center;
		border-radius: 50%;
		background: color-mix(in srgb, var(--status-available) 12%, transparent);
		color: var(--status-available);
		box-shadow: 0 0 0 18px color-mix(in srgb, var(--status-available) 5%, transparent);
	}

	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.3;
		}
	}
</style>
