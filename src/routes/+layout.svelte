<script lang="ts">
	import "#lib/design/fonts.css";
	import "#lib/design/colors.css";
	import "#lib/design/elevation.css";
	import "#lib/design/motion.css";
	import "#lib/design/shape.css";
	import "#lib/design/typography.css";
	import Toast from "#lib/tv/Toast.svelte";
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
