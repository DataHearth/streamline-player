<script lang="ts">
let { children }: { children: import("svelte").Snippet } = $props();
</script>

<span class="chip">{@render children()}</span>

<style>
.chip {
	padding: 3px 7px;
	border: 1px solid rgba(255, 255, 255, 0.16);
	border-radius: var(--radius-sm);
	background: color-mix(in srgb, var(--bg-deep) 35%, transparent);
	color: var(--player-ink);
	font: 400 11px var(--font-mono);
}
</style>
