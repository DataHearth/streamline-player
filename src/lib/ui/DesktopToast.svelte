<script lang="ts">
import Icon, { type IconName } from "#lib/ui/Icon.svelte";
import "#lib/ui/ui.css";

let {
	title,
	text,
	icon,
	warning = false,
	leaving = false,
}: {
	title: string;
	text?: string;
	icon: IconName;
	warning?: boolean;
	leaving?: boolean;
} = $props();
</script>

<div class="slot" class:leaving>
	<div
		class="sl-toast"
		role="status"
		aria-live="polite"
		style:--c={warning ? "var(--status-warning)" : "var(--status-available)"}
	>
		<Icon name={icon} />
		<div>
			<div class="sl-toast__title">{title}</div>
			{#if text}
				<div class="sl-toast__text">{text}</div>
			{/if}
		</div>
	</div>
</div>

<style>
.slot {
	animation: slp-drop 260ms var(--ease);
	transition:
		opacity 180ms ease-in,
		transform 180ms ease-in;
}

.slot.leaving {
	opacity: 0;
	transform: translateY(-8px);
}

@keyframes slp-drop {
	from {
		opacity: 0;
		transform: translateY(-10px);
	}
}
</style>
