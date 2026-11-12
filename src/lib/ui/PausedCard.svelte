<script lang="ts">
let {
	on,
	eyebrow,
	title,
	text,
	remaining,
}: { on: boolean; eyebrow: string; title: string; text: string; remaining: string } = $props();

const enter = (_: Element) => ({
	duration: 300,
	css: (t: number) => `opacity:${t};transform:translateY(${(1 - t) * 10}px)`,
});
const leave = (_: Element) => ({
	duration: 210,
	css: (t: number) => `opacity:${t};transform:translateY(${(1 - t) * 10}px)`,
});
</script>

{#if on}
	<div class="card paused" in:enter out:leave>
		<span class="eyebrow">{eyebrow}</span>
		<span class="title">{title}</span>
		<span class="text">{text}</span>
		<span class="left">{remaining}</span>
	</div>
{/if}

<style>
.card {
	position: absolute;
	left: 28px;
	bottom: 128px;
	width: min(460px, 46%);
	display: flex;
	flex-direction: column;
	gap: 8px;
}

.eyebrow {
	font: 500 11px var(--font-mono);
	letter-spacing: 0.14em;
	text-transform: uppercase;
	color: var(--player-accent-text);
}

.title {
	font-size: 30px;
	line-height: 1.1;
	font-weight: 700;
	letter-spacing: -0.02em;
}

.text {
	font-size: 14px;
	line-height: 1.55;
	color: var(--player-ink-muted);
}

.left {
	font: 400 12px var(--font-mono);
	color: var(--fg-subtle);
}
</style>
