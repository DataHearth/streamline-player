<script lang="ts">
import Toast from "#lib/ui/Toast.svelte";
import type { Snippet } from "svelte";
import { onMount } from "svelte";

const STAGE_WIDTH = 1920;
let { children }: { children: Snippet } = $props();

onMount(() => {
	const resize = () =>
		document.documentElement.style.setProperty(
			"--tv-zoom",
			String(window.innerWidth / STAGE_WIDTH),
		);
	resize();
	window.addEventListener("resize", resize);
	return () => window.removeEventListener("resize", resize);
});
</script>

{@render children()}
<Toast />
