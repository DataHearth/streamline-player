<script lang="ts">
import Icon from "#lib/ui/Icon.svelte";
import type { Snippet } from "svelte";

let { children, pressed, onclick }: { children: Snippet; pressed?: boolean; onclick?: () => void } =
	$props();
</script>

{#if onclick}
	<button type="button" class="toggle" aria-pressed={pressed} {onclick}>
		{#if pressed}
			<Icon name="tick" size={13} stroke={2} />
		{/if}
		{@render children()}
	</button>
{:else}
	<span class="chip">{@render children()}</span>
{/if}

<style>
.chip {
	padding: 3px 7px;
	border: 1px solid rgba(255, 255, 255, 0.16);
	border-radius: var(--radius-sm);
	background: color-mix(in srgb, var(--bg-deep) 35%, transparent);
	color: var(--player-ink);
	font: 400 11px var(--font-mono);
}

.toggle {
	display: inline-flex;
	align-items: center;
	gap: 6px;
	height: 32px;
	padding: 0 12px;
	border: 1px solid var(--border);
	border-radius: var(--radius-full);
	background: transparent;
	color: var(--fg-muted);
	font-size: 13px;
	font-weight: 500;
	white-space: nowrap;
	cursor: pointer;
}

.toggle[aria-pressed="true"] {
	border-color: var(--accent-line);
	background: var(--accent-soft);
	color: var(--accent-text);
}
</style>
