<script lang="ts">
import { ChevronsRight } from "@lucide/svelte";

let {
	on,
	label,
	bottom,
	onclick,
}: { on: boolean; label: string; bottom: number; onclick: () => void } = $props();

const pop = (_: Element) => ({
	duration: 240,
	css: (t: number) =>
		`opacity:${t};transform:translateY(${(1 - t) * 10}px) scale(${0.96 + 0.04 * t})`,
});
</script>

{#if on}
	<div class="slot" style:bottom="{bottom}px" transition:pop>
		<button type="button" class="skip" {onclick}>
			<ChevronsRight size={18} strokeWidth={2} />{label}
		</button>
	</div>
{/if}

<style>
.slot {
	position: absolute;
	right: 32px;
	transition: bottom 250ms var(--ease);
}

.skip {
	display: inline-flex;
	align-items: center;
	gap: 8px;
	height: 44px;
	padding: 0 18px;
	border: 1px solid rgba(255, 255, 255, 0.22);
	border-radius: var(--radius-player-controls);
	background: rgba(0, 0, 0, 0.45);
	backdrop-filter: blur(12px);
	color: var(--player-ink);
	font-size: 14px;
	font-weight: 600;
	cursor: pointer;
}

.skip:hover {
	background: var(--player-ink);
	color: var(--fg-on-accent);
}
</style>
