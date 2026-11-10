<script lang="ts">
import "#lib/design/fonts.css";
import "#lib/design/colors.css";
import "#lib/design/elevation.css";
import "#lib/design/motion.css";
import "#lib/design/shape.css";
import "#lib/design/typography.css";
import { goto } from "$app/navigation";
import Toast from "#lib/ui/Toast.svelte";
import { authState, onSignedOut } from "#platform";
import { onMount } from "svelte";

const STAGE_WIDTH = 1920;
let { children } = $props();
let ready = $state(false);

const toWelcome = () => goto("/welcome", { replace: true });

onMount(() => {
	const resize = () =>
		document.documentElement.style.setProperty(
			"--tv-zoom",
			String(window.innerWidth / STAGE_WIDTH),
		);
	resize();
	window.addEventListener("resize", resize);
	// Rust drops a revoked key on the first 401 and says so here.
	const signedOut = onSignedOut(toWelcome);
	authState()
		.then(async (s) => {
			if (!s.paired && location.pathname !== "/welcome") await toWelcome();
		})
		.finally(() => (ready = true));
	return () => {
		window.removeEventListener("resize", resize);
		signedOut.then((off) => off());
	};
});
</script>

{#if ready}
	{@render children()}
{/if}
<Toast />

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
