<script lang="ts" generics="V extends string">
import Icon, { type IconName } from "#lib/ui/Icon.svelte";

let {
	options,
	value,
	label,
	onchange,
}: {
	options: { value: V; label: string; icon: IconName }[];
	value: V;
	label: string;
	onchange: (value: V) => void;
} = $props();
</script>

<fieldset class="seg" aria-label={label}>
	{#each options as o (o.value)}
		<button
			type="button"
			aria-pressed={value === o.value}
			aria-label={o.label}
			title={o.label}
			onclick={() => onchange(o.value)}
		>
			<Icon name={o.icon} />
		</button>
	{/each}
</fieldset>

<style>
.seg {
	width: fit-content;
	display: inline-flex;
	gap: 3px;
	margin: 0;
	min-width: 0;
	padding: 3px;
	border: 1px solid var(--border);
	border-radius: 9px;
	background: var(--bg-deep);
}

button {
	display: inline-flex;
	align-items: center;
	justify-content: center;
	width: 34px;
	height: 30px;
	padding: 0;
	border: 0;
	border-radius: 7px;
	background: none;
	color: var(--fg-subtle);
	cursor: pointer;
}

button:hover {
	color: var(--fg);
}

button[aria-pressed="true"] {
	background: var(--bg-selected);
	color: var(--fg);
}

button :global(.sl-ico) {
	font-size: 16px;
}
</style>
