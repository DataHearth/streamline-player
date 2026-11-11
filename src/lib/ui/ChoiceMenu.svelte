<script lang="ts">
type Option = { value: string; label: string };

let {
	label,
	options,
	value,
	select = false,
	onchange,
}: {
	label: string;
	options: Option[];
	value: string;
	select?: boolean;
	onchange: (value: string) => void;
} = $props();

const at = $derived(
	Math.max(
		0,
		options.findIndex((o) => o.value === value),
	),
);
</script>

{#if select}
	<select class="pick" aria-label={label} {value} onchange={(e) => onchange(e.currentTarget.value)}>
		{#each options as o (o.value)}
			<option value={o.value}>{o.label}</option>
		{/each}
	</select>
{:else}
	<fieldset class="group" aria-label={label}>
		<span
			class="pill"
			style:width="calc((100% - {6 + 3 * (options.length - 1)}px) / {options.length})"
			style:transform="translateX(calc({at} * (100% + 3px)))"
		></span>
		{#each options as o (o.value)}
			<button type="button" aria-pressed={o.value === value} onclick={() => onchange(o.value)}>
				{o.label}
			</button>
		{/each}
	</fieldset>
{/if}

<style>
.pick {
	flex: none;
	width: 172px;
	height: 36px;
	padding: 0 10px;
	font-size: 13px;
	cursor: pointer;
	color-scheme: dark;
	border: 1px solid var(--border);
	border-radius: var(--radius-md);
	background: var(--bg-card);
	color: var(--fg);
	font-family: var(--font-sans);
}

.pick:focus-visible {
	outline: none;
	border-color: var(--accent);
	box-shadow: 0 0 0 2px var(--accent-ring);
}

.group {
	position: relative;
	flex: none;
	max-width: 100%;
	box-sizing: border-box;
	display: grid;
	grid-auto-flow: column;
	grid-auto-columns: 1fr;
	gap: 3px;
	margin: 0;
	min-width: 0;
	padding: 3px;
	border-radius: 9px;
	background: var(--bg-deep);
	border: 1px solid var(--border);
}

.pill {
	position: absolute;
	top: 3px;
	bottom: 3px;
	left: 3px;
	border-radius: 7px;
	background: var(--bg-selected);
	transition: transform 240ms var(--ease);
}

button {
	position: relative;
	height: 28px;
	padding: 0 11px;
	border: 0;
	border-radius: 7px;
	background: transparent;
	color: var(--fg-subtle);
	font-size: 12.5px;
	font-weight: 500;
	white-space: nowrap;
	cursor: pointer;
	transition: color 180ms;
}

button:hover {
	color: var(--fg);
}

button[aria-pressed="true"] {
	color: var(--fg);
}

button:focus-visible {
	outline: 2px solid var(--accent-ring);
	outline-offset: -2px;
}
</style>
