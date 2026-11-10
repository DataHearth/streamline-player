<script lang="ts">
import Icon, { type IconName } from "#lib/ui/Icon.svelte";
import type { Snippet } from "svelte";

let {
	icon = "film",
	title,
	text,
	action,
}: { icon?: IconName; title: string; text?: string; action?: Snippet } = $props();
</script>

<div class="empty">
	<Icon name={icon} />
	<div class="title">{title}</div>
	{#if text}
		<div class="text">{text}</div>
	{/if}
	{@render action?.()}
</div>

<style>
.empty {
	display: flex;
	flex-direction: column;
	align-items: center;
	gap: 8px;
	max-width: 360px;
	margin: 0 auto;
	padding: 32px 16px;
	text-align: center;
}

.empty > :global(.sl-ico) {
	font-size: 48px;
	color: var(--fg-subtle);
	margin-bottom: 8px;
}

.title {
	font-size: 16px;
	font-weight: 500;
	color: var(--fg);
}

.text {
	font-size: 14px;
	color: var(--fg-muted);
	margin-bottom: 12px;
}
</style>
