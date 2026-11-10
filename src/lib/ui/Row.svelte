<script lang="ts">
import Icon from "#lib/ui/Icon.svelte";
import type { Snippet } from "svelte";

const PAGE_PX = 660;

let {
	title,
	note,
	padX = 56,
	prevLabel,
	nextLabel,
	children,
}: {
	title: string;
	note?: string;
	padX?: number;
	prevLabel: string;
	nextLabel: string;
	children: Snippet;
} = $props();

let scroller = $state<HTMLElement>();
const page = (dir: -1 | 1) => scroller?.scrollBy({ left: dir * PAGE_PX, behavior: "smooth" });
</script>

<section>
	<div class="head" style:padding="0 {padX}px">
		<h2>{title}</h2>
		<div class="tools">
			{#if note}
				<span class="note">{note}</span>
			{/if}
			<button type="button" class="arrow" aria-label={prevLabel} onclick={() => page(-1)}>
				<Icon name="chevron-left" size={15} stroke={2} />
			</button>
			<button type="button" class="arrow" aria-label={nextLabel} onclick={() => page(1)}>
				<Icon name="chevron-right" size={15} stroke={2} />
			</button>
		</div>
	</div>
	<div
		class="scroller"
		bind:this={scroller}
		style:padding="6px {padX}px 10px"
		style:scroll-padding="0 {padX}px"
	>
		{@render children()}
	</div>
</section>

<style>
section {
	display: flex;
	flex-direction: column;
	gap: 14px;
}

.head {
	display: flex;
	align-items: center;
	justify-content: space-between;
}

h2 {
	margin: 0;
	font-size: 18px;
	font-weight: 600;
	letter-spacing: -0.01em;
}

.tools {
	display: flex;
	align-items: center;
	gap: 6px;
}

.note {
	margin-right: 6px;
	font: 400 12px var(--font-mono);
	color: var(--fg-subtle);
	white-space: nowrap;
}

.arrow {
	width: 30px;
	height: 30px;
	display: grid;
	place-items: center;
	border-radius: 50%;
	border: 1px solid var(--border-strong);
	background: transparent;
	color: var(--fg-muted);
	cursor: pointer;
}

.arrow:hover {
	background: color-mix(in srgb, var(--fg) 8%, transparent);
	color: var(--fg);
}

.scroller {
	display: flex;
	gap: 16px;
	overflow-x: auto;
	scroll-snap-type: x proximity;
}
</style>
