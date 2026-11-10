<script lang="ts" generics="V extends string">
import { dismiss } from "#lib/ui/dismiss.ts";
import Icon from "#lib/ui/Icon.svelte";
import { presence } from "#lib/ui/presence.svelte.ts";

let {
	options,
	value,
	label,
	onchange,
}: {
	options: { value: V; label: string }[];
	value: V;
	label: string;
	onchange: (value: V) => void;
} = $props();

let open = $state(false);
const panel = presence(() => open, 170);
const current = $derived(options.find((o) => o.value === value)?.label ?? "");
</script>

<div class="sort" use:dismiss={() => (open = false)}>
	<button
		type="button"
		class="trigger"
		aria-haspopup="menu"
		aria-expanded={open}
		onclick={() => (open = !open)}
	>
		<Icon name="arrow-up-down" size={15} stroke={1.75} />{current}
		<span class="chev" class:open><Icon name="chevron-down" size={14} stroke={2} /></span>
	</button>
	{#if panel.mounted}
		<div class="menu" class:shown={panel.shown && open} role="menu" aria-label={label}>
			{#each options as o (o.value)}
				<button
					type="button"
					role="menuitemradio"
					aria-checked={value === o.value}
					class="opt"
					onclick={() => {
						onchange(o.value);
						open = false;
					}}
				>
					<span class="check"><Icon name="tick" size={14} stroke={2.5} /></span>{o.label}
				</button>
			{/each}
		</div>
	{/if}
</div>

<style>
.sort {
	position: relative;
}

.trigger {
	height: 38px;
	display: flex;
	align-items: center;
	gap: 8px;
	padding: 0 12px;
	border-radius: var(--radius-md);
	border: 1px solid var(--border-strong);
	background: var(--surface);
	color: var(--fg-muted);
	font-size: 13px;
	cursor: pointer;
	white-space: nowrap;
}

.trigger:hover {
	background: rgba(255, 255, 255, 0.08);
	color: var(--fg);
}

.chev {
	display: grid;
	transition: transform var(--dur-base) var(--ease);
}

.chev.open {
	transform: rotate(180deg);
}

.menu {
	position: absolute;
	right: 0;
	top: 44px;
	z-index: 5;
	width: 200px;
	padding: 6px;
	box-sizing: border-box;
	border-radius: 12px;
	background: var(--bg-elevated);
	border: 1px solid var(--border-strong);
	box-shadow: var(--shadow-4);
	transform-origin: top right;
	opacity: 0;
	transform: translateY(-6px) scale(0.97);
	pointer-events: none;
	transition:
		opacity 130ms var(--ease-in),
		transform 150ms var(--ease-in);
}

.menu.shown {
	opacity: 1;
	transform: translateY(0) scale(1);
	pointer-events: auto;
	transition:
		opacity 160ms var(--ease),
		transform 220ms var(--ease);
}

.opt {
	width: 100%;
	height: 36px;
	display: flex;
	align-items: center;
	gap: 10px;
	padding: 0 10px;
	border: 0;
	border-radius: 8px;
	background: transparent;
	color: var(--fg);
	font-size: 13px;
	text-align: left;
	cursor: pointer;
}

.opt:hover {
	background: var(--surface-2);
}

.check {
	width: 14px;
	color: var(--accent);
	opacity: 0;
}

[aria-checked="true"] > .check {
	opacity: 1;
}
</style>
