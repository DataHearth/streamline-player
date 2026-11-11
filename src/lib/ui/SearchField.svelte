<script lang="ts">
import Icon from "#lib/ui/Icon.svelte";

let {
	value = $bindable(""),
	input = $bindable(),
	placeholder,
	clearLabel,
	oninput,
	onclear,
	onkeydown,
}: {
	value?: string;
	input?: HTMLInputElement;
	placeholder: string;
	clearLabel: string;
	oninput: () => void;
	onclear: () => void;
	onkeydown: (e: KeyboardEvent) => void;
} = $props();
</script>

<div class="field">
	<Icon name="search" />
	<input
		bind:this={input}
		bind:value
		data-search-field
		{placeholder}
		spellcheck="false"
		autocomplete="off"
		{oninput}
		{onkeydown}
	>
	{#if value}
		<button type="button" class="x" aria-label={clearLabel} onclick={onclear}>
			<Icon name="x" size={15} />
		</button>
	{/if}
</div>

<style>
.field {
	display: flex;
	align-items: center;
	gap: 14px;
	height: 60px;
	padding: 0 18px;
	border: 1px solid var(--border);
	border-radius: 14px;
	background: var(--bg-elevated);
	font-size: 18px;
	color: var(--fg);
}

.field:focus-within {
	border-color: var(--accent);
	box-shadow: 0 0 0 3px var(--accent-glow);
}

.field > :global(.sl-ico) {
	font-size: 22px;
	color: var(--fg-subtle);
}

input {
	flex: 1;
	min-width: 0;
	height: 100%;
	padding: 0;
	border: 0;
	background: none;
	color: var(--fg);
	font: inherit;
	outline: none;
}

input::placeholder {
	color: var(--fg-faint);
}

input::-webkit-search-cancel-button {
	appearance: none;
}

.x {
	margin-left: auto;
	display: grid;
	place-items: center;
	width: 32px;
	height: 32px;
	padding: 0;
	border: 0;
	border-radius: 50%;
	background: var(--surface-2);
	color: var(--fg-muted);
	font-size: 15px;
	cursor: pointer;
}
</style>
