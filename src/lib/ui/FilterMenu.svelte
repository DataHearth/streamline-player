<script lang="ts">
import Icon from "#lib/ui/Icon.svelte";

type Option = { value: string; label: string; count?: number };

let {
	options,
	value,
	label,
	placeholder,
	emptyLabel,
	width = 248,
	onchange,
}: {
	options: Option[];
	value: string;
	label: string;
	placeholder: string;
	emptyLabel: string;
	width?: number;
	onchange: (value: string) => void;
} = $props();

let query = $state("");
const shown = $derived(
	query.trim()
		? options.filter((o) => o.label.toLowerCase().includes(query.trim().toLowerCase()))
		: options,
);
const focus = (el: HTMLInputElement) => el.focus({ preventScroll: true });
</script>

<div class="fmenu" style:width="{width}px">
	<label class="search">
		<Icon name="search" size={14} stroke={2} />
		<input bind:value={query} {placeholder} aria-label={placeholder} use:focus>
	</label>
	<div class="list" role="listbox" aria-label={label}>
		{#each shown as o (o.value)}
			<button
				type="button"
				role="option"
				aria-selected={value === o.value}
				class="opt"
				disabled={o.count === 0}
				onclick={() => onchange(o.value)}
			>
				<span class="check"><Icon name="tick" size={14} stroke={2.5} /></span>
				<span class="label">{o.label}</span>
				{#if o.count !== undefined}
					<span class="n">{o.count}</span>
				{/if}
			</button>
		{/each}
		{#if !shown.length}
			<span class="none">{emptyLabel}</span>
		{/if}
	</div>
</div>

<style>
.fmenu {
	padding: 6px;
	border: 1px solid var(--border-strong);
	border-radius: 12px;
	background: var(--bg-elevated);
	box-shadow: var(--shadow-4);
	box-sizing: border-box;
}

.search {
	height: 36px;
	display: flex;
	align-items: center;
	gap: 8px;
	padding: 0 10px;
	margin-bottom: 4px;
	border-radius: 8px;
	background: var(--surface);
	color: var(--fg-subtle);
}

input {
	flex: 1;
	min-width: 0;
	height: 100%;
	border: 0;
	outline: none;
	background: none;
	color: var(--fg);
	font: inherit;
	font-size: 13px;
}

input::placeholder {
	color: var(--fg-faint);
}

.list {
	max-height: 300px;
	overflow-y: auto;
	display: flex;
	flex-direction: column;
}

.opt {
	flex: none;
	width: 100%;
	height: 34px;
	display: flex;
	align-items: center;
	gap: 10px;
	padding: 0 10px;
	border: 0;
	border-radius: 8px;
	background: none;
	color: var(--fg);
	font-size: 13px;
	text-align: left;
	cursor: pointer;
}

.opt:hover {
	background: var(--surface-2);
}

.opt:disabled {
	color: var(--fg-faint);
	cursor: default;
	background: none;
}

.check {
	width: 14px;
	flex: none;
	color: var(--accent);
	font-size: 14px;
	opacity: 0;
}

[aria-selected="true"] > .check {
	opacity: 1;
}

.label {
	flex: 1;
	min-width: 0;
	white-space: nowrap;
	overflow: hidden;
	text-overflow: ellipsis;
}

.n {
	font: 400 11px var(--font-mono);
	color: var(--fg-faint);
}

.none {
	padding: 10px;
	font-size: 13px;
	color: var(--fg-faint);
}
</style>
