<script lang="ts">
import Icon, { type IconName } from "#lib/ui/Icon.svelte";
import type { Snippet } from "svelte";

let {
	label,
	desc,
	icon,
	control,
	below,
}: {
	label: string;
	desc?: string;
	icon?: IconName;
	control?: Snippet;
	below?: Snippet;
} = $props();
</script>

<div class="row">
	<div class="line">
		{#if icon}
			<span class="icon"><Icon name={icon} /></span>
		{/if}
		<span class="text">
			<span class="label">{label}</span>
			{#if desc}
				<span class="desc">{desc}</span>
			{/if}
		</span>
		{@render control?.()}
	</div>
	{@render below?.()}
</div>

<style>
.row {
	display: flex;
	flex-direction: column;
	gap: 12px;
	padding: 15px 20px;
	border-bottom: 1px solid var(--border);
}

.line {
	display: flex;
	align-items: center;
	gap: 16px;
}

.icon {
	flex: none;
	width: 38px;
	height: 38px;
	border-radius: var(--radius-md);
	display: grid;
	place-items: center;
	background: var(--bg-deep);
	border: 1px solid var(--border);
	color: var(--accent-text);
}

.icon :global(.sl-ico) {
	font-size: 18px;
}

.text {
	flex: 1;
	min-width: 0;
	display: flex;
	flex-direction: column;
	gap: 3px;
}

.label {
	font-size: 14px;
	font-weight: 500;
	color: var(--fg);
}

.desc {
	font-size: 12.5px;
	line-height: 1.5;
	color: var(--fg-subtle);
}
</style>
