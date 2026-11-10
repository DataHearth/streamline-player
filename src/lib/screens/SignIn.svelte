<script lang="ts">
import { SignInError, type SignInErrorCode } from "#lib/core/auth.ts";
import type { AuthMethods } from "#lib/core/types.ts";
import { m as i18n } from "#lib/paraglide/messages.js";
import Icon from "#lib/ui/Icon.svelte";
import Mark from "#lib/ui/Mark.svelte";
import { authMethods, serverHost, signInEmail, startSso } from "#platform";
import { onMount } from "svelte";

let {
	next,
	failed = null,
	done,
}: { next: string; failed?: string | null; done: () => void } = $props();

let methods = $state<AuthMethods | null>(null);
let host = $state("");
let busy = $state<"sso" | "email" | null>(null);
let emailOpen = $state(false);
let email = $state("");
let password = $state("");
let code = $state<SignInErrorCode | null>(null);

const COPY: Record<SignInErrorCode, () => string> = {
	credentials: () => i18n.signin_error_credentials(),
	denied: () => i18n.signin_error_denied(),
	state: () => i18n.signin_error_expired(),
	invalid_grant: () => i18n.signin_error_expired(),
	network: () => i18n.signin_error_network({ host }),
};
const KNOWN = Object.keys(COPY) as SignInErrorCode[];

const error = $derived(code ? COPY[code]() : "");

const fail = (e: unknown) => {
	busy = null;
	code = e instanceof SignInError ? e.code : "network";
};

async function sso(provider: string) {
	if (busy) return;
	busy = "sso";
	code = null;
	try {
		await startSso(provider, next);
	} catch (e) {
		fail(e);
	}
}

async function submit(e: SubmitEvent) {
	e.preventDefault();
	if (busy) return;
	busy = "email";
	code = null;
	try {
		await signInEmail(email, password);
		done();
	} catch (err) {
		fail(err);
	}
}

onMount(() => {
	if (failed && (KNOWN as string[]).includes(failed)) code = failed as SignInErrorCode;
	serverHost().then((h) => (host = h));
	authMethods()
		.then((v) => (methods = v))
		.catch(() => {
			methods = { sso: [], email: true, register_url: null };
			code = "network";
		});
});
</script>

<div class="page">
	<div class="glow"></div>
	{#if methods}
		<div class="col">
			<div class="card">
				<header>
					<Mark size={48} />
					<span class="brand">Streamline</span>
					<h1>{i18n.signin_title()}</h1>
					<p class="lead">{i18n.signin_lead({ host })}</p>
				</header>
				{#if error}
					<p class="error" role="alert">{error}</p>
				{/if}
				{#each methods.sso as provider (provider.id)}
					<button type="button" class="primary sso" onclick={() => sso(provider.id)}>
						{#if busy === "sso"}
							<span class="spin"></span>
						{:else}
							<Icon name="key-round" size={18} stroke={1.9} />
						{/if}
						{busy === "sso"
							? i18n.signin_sso_busy({ label: provider.label })
							: i18n.signin_sso({ label: provider.label })}
					</button>
				{/each}
				{#if methods.email}
					{#if methods.sso.length}
						<div class="or">
							<span class="rule"></span>
							<span class="or-text">{i18n.signin_or()}</span>
							<span class="rule"></span>
						</div>
					{/if}
					<button
						type="button"
						class="toggle"
						aria-expanded={emailOpen}
						onclick={() => (emailOpen = !emailOpen)}
					>
						<Icon name="mail" size={16} stroke={1.75} />{i18n.signin_email_toggle()}
						<span class="chev" class:open={emailOpen}
							><Icon name="chevron-down" size={16} stroke={2} /></span
						>
					</button>
					{#if emailOpen}
						<form class="form" onsubmit={submit}>
							<label>
								<span>{i18n.signin_email()}</span>
								<input type="email" autocomplete="username" bind:value={email}>
							</label>
							<label>
								<span>{i18n.signin_password()}</span>
								<input type="password" autocomplete="current-password" bind:value={password}>
							</label>
							<button type="submit" class="primary" style:opacity={busy === "email" ? 0.75 : 1}>
								{busy === "email" ? i18n.signin_submitting() : i18n.signin_submit()}
							</button>
						</form>
					{/if}
				{/if}
			</div>
			{#if methods.register_url}
				<p class="register">
					{i18n.signin_no_account()}<a href={methods.register_url}>{i18n.signin_create()}</a>
				</p>
			{/if}
		</div>
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

.col {
	position: relative;
	width: 420px;
	display: flex;
	flex-direction: column;
	gap: 20px;
	animation: sl-rise var(--dur-slow) var(--ease);
}

.card {
	display: flex;
	flex-direction: column;
	padding: 34px 34px 30px;
	border-radius: var(--radius-xl);
	background: color-mix(in srgb, var(--bg-elevated) 95%, transparent);
	border: 1px solid var(--border);
	box-shadow: var(--shadow-4);
}

header {
	display: flex;
	flex-direction: column;
	align-items: center;
	gap: 12px;
	margin-bottom: 24px;
	text-align: center;
}

.brand {
	font: 500 10px var(--font-mono);
	letter-spacing: 0.18em;
	text-transform: uppercase;
	color: var(--fg-faint);
}

h1 {
	margin: 0;
	font-size: 21px;
	font-weight: 600;
	letter-spacing: -0.02em;
}

.lead {
	margin: -4px 0 0;
	font-size: 14px;
	color: var(--fg-muted);
}

.error {
	margin: -8px 0 16px;
	text-align: center;
	font-size: 13px;
	color: var(--status-failed);
}

button {
	height: 44px;
	display: flex;
	align-items: center;
	justify-content: center;
	border: 0;
	cursor: pointer;
}

.primary {
	gap: 10px;
	border-radius: var(--radius-md);
	background: var(--accent);
	color: var(--fg-on-accent);
	font-size: 14px;
	font-weight: 600;
}

.primary:hover {
	background: var(--accent-hover);
}

.sso {
	width: 100%;
}

.spin {
	width: 15px;
	height: 15px;
	border-radius: 50%;
	border: 2px solid color-mix(in srgb, var(--bg-deep) 30%, transparent);
	border-top-color: var(--bg-deep);
	animation: spin 700ms linear infinite;
}

@keyframes spin {
	to {
		transform: rotate(360deg);
	}
}

.or {
	display: flex;
	align-items: center;
	gap: 10px;
	margin: 20px 0 14px;
}

.rule {
	flex: 1;
	height: 1px;
	background: rgba(255, 255, 255, 0.08);
}

.or-text {
	font: 500 11px var(--font-mono);
	letter-spacing: 0.1em;
	text-transform: uppercase;
	color: var(--fg-faint);
}

.toggle {
	width: 100%;
	gap: 8px;
	border-radius: var(--radius-md);
	background: transparent;
	color: var(--fg-muted);
	font-size: 14px;
	font-weight: 500;
}

.toggle:hover {
	color: var(--fg);
}

.chev {
	display: inline-flex;
	transition: transform 180ms;
}

.chev.open {
	transform: rotate(180deg);
}

.form {
	display: flex;
	flex-direction: column;
	gap: 14px;
	padding-top: 12px;
	animation: sl-rise 220ms var(--ease);
}

label {
	display: flex;
	flex-direction: column;
	gap: 7px;
}

label span {
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
	font: 400 14px var(--font-sans);
	outline: none;
}

input:focus {
	border-color: var(--accent);
	box-shadow: 0 0 0 3px var(--accent-glow);
}

.register {
	margin: 0;
	display: flex;
	justify-content: center;
	gap: 6px;
	font-size: 13.5px;
	color: var(--fg-subtle);
}

.register a {
	color: var(--accent-text);
	font-weight: 500;
	text-decoration: none;
}

.register a:hover {
	color: var(--fg);
}
</style>
