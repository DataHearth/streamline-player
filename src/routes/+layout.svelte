<script lang="ts">
import "#lib/design/fonts.css";
import "#lib/design/colors.css";
import "#lib/design/elevation.css";
import "#lib/design/motion.css";
import "#lib/design/shape.css";
import "#lib/design/typography.css";
import { appPath, goto } from "#lib/ui/nav.ts";
import { toast } from "#lib/core/toast.svelte.ts";
import { m as i18n } from "#lib/paraglide/messages.js";
import { FORM_FACTOR, IS_WEB } from "#lib/platform/target.ts";
import DesktopShell from "#lib/shells/desktop/Shell.svelte";
import TvShell from "#lib/shells/tv/Shell.svelte";
import { authState, init, onSignedOut } from "#platform";
import { onMount } from "svelte";

let { children } = $props();
let ready = $state(false);

const onSignIn = () => appPath().startsWith("/signin");

const toSignIn = () => {
	const next = `${appPath()}${location.search}`;
	return goto(`/signin?next=${encodeURIComponent(next)}`, { replace: true });
};

onMount(() => {
	// A revoked key or browser session is dropped on the first 401, and said so here.
	const signedOut = onSignedOut(() => {
		if (!IS_WEB) return goto("/welcome", { replace: true });
		toast(i18n.shell_signed_out());
		return goto("/signin", { replace: true });
	});
	init()
		.then(authState)
		.then(async (s) => {
			if (s.paired) return;
			if (IS_WEB) {
				if (!onSignIn()) await toSignIn();
			} else if (appPath() !== "/welcome") await goto("/welcome", { replace: true });
		})
		.finally(() => (ready = true));
	return () => {
		signedOut.then((off) => off());
	};
});
</script>

{#snippet content()}
	{#if ready}
		{@render children()}
	{/if}
{/snippet}

{#if FORM_FACTOR === "tv"}
	<TvShell>{@render content()}</TvShell>
{:else}
	<DesktopShell web={IS_WEB}>{@render content()}</DesktopShell>
{/if}

<style>
:global(*),
:global(*::before),
:global(*::after) {
	box-sizing: border-box;
}

:global(html),
:global(body) {
	margin: 0;
	height: 100%;
	overflow: hidden;
	background: transparent;
	color: var(--player-ink);
	font-family: var(--font-sans);
	font-size: 14px;
	line-height: 1.5;
	-webkit-font-smoothing: antialiased;
}

:global(button) {
	font: inherit;
	color: inherit;
}
</style>
