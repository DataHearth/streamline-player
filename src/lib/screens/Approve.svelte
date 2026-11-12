<script lang="ts">
import { normalizeCode, requestMeta } from "#lib/core/approve.ts";
import type { PairRequest } from "#lib/core/types.ts";
import { toast } from "#lib/core/toast.svelte.ts";
import { m as i18n } from "#lib/paraglide/messages.js";
import CodeBoxes from "#lib/ui/CodeBoxes.svelte";
import Icon from "#lib/ui/Icon.svelte";
import Mark from "#lib/ui/Mark.svelte";
import { goto } from "#lib/ui/nav.ts";
import { approvePairing, denyPairing, me, pairRequest } from "#platform";
import { onMount } from "svelte";

const APPROVED_PAUSE_MS = 700;

type Step = "loading" | "entry" | "request" | "done";
type Problem = "bad" | "not_found" | "failed" | null;

let { code: given }: { code: string | null } = $props();

let step = $state<Step>("loading");
let problem = $state<Problem>(null);
let typed = $state("");
let request = $state<PairRequest | null>(null);
let userName = $state("");
let approved = $state(false);
let busy = $state(false);
let pause: ReturnType<typeof setTimeout> | undefined;

const words = {
	justNow: i18n.approve_requested_now(),
	minutes: (n: number) => i18n.approve_requested_min({ n }),
};
const meta = $derived(request ? requestMeta(request, new Date(), words) : "");
const message = $derived(
	problem === "bad"
		? i18n.approve_code_bad()
		: problem === "not_found"
			? i18n.approve_not_found()
			: problem === "failed"
				? i18n.approve_failed()
				: "",
);

const gone = (e: unknown) => String(e).includes("no_pending_pairing");

async function lookup(raw: string) {
	const code = normalizeCode(raw);
	if (!code) {
		problem = "bad";
		step = "entry";
		return;
	}
	step = "loading";
	problem = null;
	try {
		request = await pairRequest(code);
		step = "request";
	} catch (e) {
		request = null;
		problem = gone(e) ? "not_found" : "failed";
		step = "entry";
	}
}

async function approve() {
	if (!request || busy || approved) return;
	busy = true;
	problem = null;
	try {
		await approvePairing(request.user_code);
		approved = true;
		pause = setTimeout(() => (step = "done"), APPROVED_PAUSE_MS);
	} catch (e) {
		if (gone(e)) {
			request = null;
			problem = "not_found";
			step = "entry";
		} else problem = "failed";
	} finally {
		busy = false;
	}
}

async function deny() {
	if (!request || busy || approved) return;
	busy = true;
	problem = null;
	try {
		await denyPairing(request.user_code);
		toast(i18n.welcome_denied());
		await goto("/", { replace: true });
	} catch (e) {
		if (gone(e)) {
			request = null;
			problem = "not_found";
			step = "entry";
		} else problem = "failed";
	} finally {
		busy = false;
	}
}

onMount(() => {
	me()
		.then((v) => (userName = v.name))
		.catch(() => {});
	if (given) lookup(given);
	else step = "entry";
	return () => clearTimeout(pause);
});
</script>

<div class="page">
	<div class="glow"></div>
	{#if step === "request" && request}
		<div class="card">
			<div class="top">
				<Mark size={40} />
				<span class="eyebrow">{i18n.approve_eyebrow()}</span>
			</div>
			<div class="head">
				<h1>{i18n.approve_title()}</h1>
				<p class="lead">{i18n.approve_lead()}</p>
			</div>
			<CodeBoxes code={request.user_code} size="approve" {approved} />
			<div class="device">
				<span class="glyph"><Icon name="tv" size={22} stroke={1.75} /></span>
				<span class="who">
					<span class="name">{request.name}</span>
					<span class="meta">{meta}</span>
				</span>
			</div>
			<p class="consent">{i18n.approve_consent({ name: userName })}</p>
			{#if message}
				<p class="error" role="alert">{message}</p>
			{/if}
			<div class="actions">
				<button type="button" class="approve" class:ok={approved} onclick={approve}>
					{approved ? i18n.welcome_approved() : i18n.approve_approve()}
				</button>
				<button type="button" class="deny" onclick={deny}>{i18n.approve_deny()}</button>
			</div>
		</div>
	{:else if step === "done" && request}
		<div class="card done">
			<span class="tick"><Icon name="tick" size={28} stroke={2.25} /></span>
			<div class="head center">
				<h1>{i18n.approve_done_title()}</h1>
				<p class="lead">{i18n.approve_done_lead({ device: request.name })}</p>
			</div>
			<button type="button" class="approve back" onclick={() => goto("/", { replace: true })}>
				{i18n.approve_back()}
			</button>
		</div>
	{:else if step === "entry"}
		<form
			class="card"
			onsubmit={(e) => {
				e.preventDefault();
				lookup(typed);
			}}
		>
			<div class="top">
				<Mark size={40} />
				<span class="eyebrow">{i18n.approve_eyebrow()}</span>
			</div>
			<div class="head">
				<h1>{i18n.approve_title()}</h1>
				<p class="lead">{i18n.approve_code_lead()}</p>
			</div>
			<label class="field">
				<span class="label">{i18n.approve_code_label()}</span>
				<input
					bind:value={typed}
					class:invalid={!!message}
					spellcheck="false"
					autocomplete="off"
					placeholder="K7F-Q2M"
				>
				{#if message}
					<span class="error" role="alert">{message}</span>
				{/if}
			</label>
			<button type="submit" class="approve">{i18n.approve_code_find()}</button>
		</form>
	{/if}
</div>

<style>
.page {
	position: absolute;
	inset: 0;
	display: flex;
	align-items: center;
	justify-content: center;
	padding: 36px 24px 24px;
	overflow: hidden;
	background: var(--bg-deep);
	color: var(--fg);
}

.glow {
	position: absolute;
	left: 50%;
	top: -180px;
	width: 900px;
	height: 900px;
	margin-left: -450px;
	border-radius: 50%;
	background: radial-gradient(
		closest-side,
		color-mix(in srgb, var(--accent) 22%, transparent),
		color-mix(in srgb, var(--accent) 6%, transparent) 55%,
		transparent 100%
	);
	pointer-events: none;
}

.card {
	position: relative;
	width: 480px;
	display: flex;
	flex-direction: column;
	gap: 22px;
	padding: 32px;
	border-radius: var(--radius-xl);
	background: var(--bg-elevated);
	border: 1px solid var(--border);
	box-shadow: var(--shadow-4);
	animation: sl-rise var(--dur-slow) var(--ease);
}

.top {
	display: flex;
	align-items: center;
	justify-content: space-between;
}

.eyebrow {
	font: 500 10.5px var(--font-mono);
	letter-spacing: 0.14em;
	text-transform: uppercase;
	color: var(--fg-faint);
}

.head {
	display: flex;
	flex-direction: column;
	gap: 6px;
}

h1 {
	margin: 0;
	font-size: 24px;
	font-weight: 600;
	letter-spacing: -0.02em;
}

.lead {
	margin: 0;
	font-size: 14px;
	line-height: 1.55;
	color: var(--fg-muted);
}

.device {
	display: flex;
	align-items: center;
	gap: 14px;
	padding: 14px;
	border-radius: 14px;
	background: var(--bg-base);
	border: 1px solid var(--border);
}

.glyph {
	flex: none;
	width: 42px;
	height: 42px;
	display: grid;
	place-items: center;
	border-radius: var(--radius-md);
	background: color-mix(in srgb, var(--accent) 12%, transparent);
	color: var(--accent-text);
}

.who {
	flex: 1;
	min-width: 0;
	display: flex;
	flex-direction: column;
	gap: 3px;
}

.name {
	font-size: 14.5px;
	font-weight: 600;
}

.meta {
	font: 400 11.5px var(--font-mono);
	color: var(--fg-subtle);
}

.consent {
	margin: -4px 0 0;
	font-size: 13px;
	line-height: 1.55;
	color: var(--fg-subtle);
}

.error {
	margin: 0;
	font-size: 13px;
	color: var(--status-failed);
}

.actions {
	display: flex;
	gap: 10px;
}

button {
	height: 44px;
	display: flex;
	align-items: center;
	justify-content: center;
	cursor: pointer;
}

.approve {
	flex: 1;
	gap: 8px;
	border: 0;
	border-radius: var(--radius-md);
	background: var(--accent);
	color: var(--fg-on-accent);
	font-size: 14.5px;
	font-weight: 600;
	transition: background var(--dur-base);
}

.approve:hover {
	box-shadow: 0 0 24px color-mix(in srgb, var(--accent) 35%, transparent);
}

.approve.ok {
	background: var(--status-available);
}

.deny {
	padding: 0 18px;
	border-radius: var(--radius-md);
	border: 1px solid var(--border-strong);
	background: var(--surface);
	color: var(--fg);
	font-size: 14.5px;
	font-weight: 500;
}

.deny:hover {
	background: rgba(255, 255, 255, 0.1);
}

.field {
	display: flex;
	flex-direction: column;
	gap: 7px;
}

.label {
	font-size: 13px;
	font-weight: 500;
	color: var(--fg-muted);
}

input {
	height: 44px;
	padding: 0 12px;
	border-radius: var(--radius-md);
	border: 1px solid var(--border-strong);
	background: var(--bg-deep);
	color: var(--fg);
	font: 400 14px var(--font-mono);
	outline: none;
}

input:focus {
	border-color: var(--accent);
	box-shadow: 0 0 0 2px var(--accent-ring);
}

input.invalid {
	border-color: var(--status-failed);
	box-shadow: none;
}

.done {
	width: 440px;
	align-items: center;
	gap: 18px;
	padding: 36px 32px 30px;
	text-align: center;
}

.center {
	text-align: center;
}

.tick {
	width: 56px;
	height: 56px;
	display: grid;
	place-items: center;
	border-radius: 50%;
	background: color-mix(in srgb, var(--status-available) 12%, transparent);
	color: var(--status-available);
	box-shadow: 0 0 0 6px color-mix(in srgb, var(--status-available) 6%, transparent);
}

.back {
	flex: none;
	width: 100%;
	font-size: 15px;
}

.back:hover {
	background: var(--accent-hover);
}
</style>
