<script lang="ts">
	import "#lib/design/fonts.css";
	import "#lib/design/colors.css";
	import "#lib/design/elevation.css";
	import "#lib/design/motion.css";
	import "#lib/design/shape.css";
	import "#lib/design/typography.css";
	import { onMount } from "svelte";

	const STAGE_WIDTH = 1920;
	let { children } = $props();

	onMount(() => {
		const resize = () => document.documentElement.style.setProperty("--tv-zoom", String(window.innerWidth / STAGE_WIDTH));
		resize();
		window.addEventListener("resize", resize);
		return () => window.removeEventListener("resize", resize);
	});
</script>

{@render children()}

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

	:global(button) {
		font-family: inherit;
	}
</style>
