<script lang="ts">
import "#lib/design/fonts.css";
import "#lib/design/colors.css";
import "#lib/design/elevation.css";
import "#lib/design/motion.css";
import "#lib/design/shape.css";
import "#lib/design/typography.css";
import { goto } from "$app/navigation";
import { FORM_FACTOR, IS_WEB } from "#lib/platform/target.ts";
import DesktopShell from "#lib/shells/desktop/Shell.svelte";
import TvShell from "#lib/shells/tv/Shell.svelte";
import { authState, init, onSignedOut } from "#platform";
import { onMount } from "svelte";

let { children } = $props();
let ready = $state(false);

// The web's sign-in screen arrives with its route; until then a signed-out browser stays on Home.
const toWelcome = () => (IS_WEB ? Promise.resolve() : goto("/welcome", { replace: true }));

onMount(() => {
	// Rust drops a revoked key on the first 401 and says so here.
	const signedOut = onSignedOut(toWelcome);
	init()
		.then(authState)
		.then(async (s) => {
			if (!s.paired && location.pathname !== "/welcome") await toWelcome();
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
