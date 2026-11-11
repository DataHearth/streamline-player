<script lang="ts">
import Button from "#lib/ui/Button.svelte";
import EmptyState from "#lib/ui/EmptyState.svelte";
import type { IconName } from "#lib/ui/Icon.svelte";

let {
	title,
	label,
	icon = "film",
	onback,
}: { title: string; label: string; icon?: IconName; onback: () => void } = $props();
</script>

<div class="wrap">
	<EmptyState {icon} {title}>
		{#snippet action()}
			<Button variant="secondary" onclick={onback}>{label}</Button>
		{/snippet}
	</EmptyState>
</div>

<style>
.wrap {
	display: grid;
	place-items: center;
	min-height: 100vh;
	padding-top: 64px;
	box-sizing: border-box;
}
</style>
