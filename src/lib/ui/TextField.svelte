<script lang="ts">
let {
	label,
	value,
	error,
	type = "text",
	oninput,
	onenter,
}: {
	label: string;
	value: string;
	error?: string;
	type?: "text" | "email";
	oninput: (value: string) => void;
	onenter?: () => void;
} = $props();
</script>

<span class="field">
	<input
		{type}
		{value}
		aria-label={label}
		aria-invalid={error ? "true" : undefined}
		spellcheck="false"
		autocomplete="off"
		oninput={(e) => oninput(e.currentTarget.value)}
		onkeydown={(e) => e.key === "Enter" && onenter?.()}
	>
	{#if error}
		<span class="error" role="alert">{error}</span>
	{/if}
</span>

<style>
.field {
	flex: none;
	width: 280px;
	max-width: 100%;
	display: flex;
	flex-direction: column;
	gap: 4px;
}

.field input {
	width: 100%;
	box-sizing: border-box;
	height: 40px;
	padding: 8px 12px;
	border: 1px solid var(--border);
	border-radius: var(--radius-md);
	background: var(--bg-card);
	color: var(--fg);
	font: 400 14px var(--font-sans);
	outline: none;
	transition:
		border-color var(--dur-fast) var(--ease),
		box-shadow var(--dur-fast) var(--ease);
}

.field input:focus-visible {
	border-color: var(--accent);
	box-shadow: 0 0 0 2px var(--accent-ring);
}

.field input[aria-invalid="true"] {
	border-color: var(--status-failed);
}

.error {
	font-size: 12px;
	color: var(--status-failed);
}
</style>
