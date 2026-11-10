<script lang="ts">
import Button from "#lib/ui/Button.svelte";
import "#lib/ui/ui.css";
import type { Snippet } from "svelte";

let {
	title,
	closeLabel,
	shown,
	onclose,
	children,
}: {
	title: string;
	closeLabel: string;
	shown: boolean;
	onclose: () => void;
	children: Snippet;
} = $props();
</script>

<div class="layer" class:shown>
	<button
		type="button"
		class="shade"
		tabindex="-1"
		aria-label={closeLabel}
		onclick={onclose}
	></button>
	<div class="panel">
		<div class="sl-dialog" role="dialog" aria-modal="true" aria-label={title}>
			<div class="sl-dialog__head">
				<h2 class="sl-dialog__title">{title}</h2>
				<Button variant="icon" icon="x" label={closeLabel} onclick={onclose} />
			</div>
			<div class="sl-dialog__body">{@render children()}</div>
		</div>
	</div>
</div>

<style>
.layer {
	position: absolute;
	inset: 0;
	z-index: 80;
	pointer-events: none;
}

.layer.shown {
	pointer-events: auto;
}

.shade {
	position: absolute;
	inset: 0;
	border: 0;
	padding: 0;
	background: var(--bg-overlay);
	backdrop-filter: blur(8px);
	opacity: 0;
	transition: opacity 160ms var(--ease-in);
}

.panel {
	position: absolute;
	inset: 0;
	display: grid;
	place-items: center;
	padding: 24px;
	pointer-events: none;
	opacity: 0;
	transform: translateY(10px) scale(0.98);
	transition:
		opacity 140ms var(--ease-in),
		transform 140ms var(--ease-in);
}

.shown .shade {
	opacity: 1;
	transition: opacity var(--dur-base) var(--ease);
}

.shown .panel {
	opacity: 1;
	transform: none;
	transition:
		opacity var(--dur-base) var(--ease),
		transform var(--dur-base) var(--ease);
}

.panel > :global(.sl-dialog) {
	pointer-events: auto;
}
</style>
