<script lang="ts">
import Art from "#lib/ui/Art.svelte";
import Icon from "#lib/ui/Icon.svelte";

let {
	title,
	code,
	sub,
	still,
	position,
	progress,
	left,
	width = 300,
	onclick,
}: {
	title: string;
	code?: string;
	sub?: string;
	still?: string;
	position?: string;
	progress: number;
	left?: string;
	width?: number;
	onclick: () => void;
} = $props();
</script>

<button type="button" class="resume" style:width="{width}px" {onclick}>
	<span class="art">
		<Art src={still} {position} />
		<span class="shade"></span>
		<span class="play"><Icon name="play" filled size={14} /></span>
		{#if left}
			<span class="left">{left}</span>
		{/if}
		<span class="bar"><span style:width="{progress}%"></span></span>
	</span>
	<span class="text">
		<span class="title"
			>{title}
			{#if code}
				<span class="code">{code}</span>
			{/if}</span
		>
		{#if sub}
			<span class="sub">{sub}</span>
		{/if}
	</span>
</button>

<style>
.resume {
	flex: none;
	display: flex;
	flex-direction: column;
	gap: 10px;
	padding: 0;
	border: 0;
	background: none;
	color: var(--fg);
	text-align: left;
	cursor: pointer;
	scroll-snap-align: start;
}

.art {
	position: relative;
	display: block;
	aspect-ratio: var(--card-aspect-backdrop);
	overflow: hidden;
	border-radius: var(--radius-player-controls);
	background: var(--bg-card);
	box-shadow: 0 0 0 1px var(--border);
	transition:
		box-shadow var(--dur-fast),
		transform var(--dur-fast);
}

.resume:hover .art {
	box-shadow: var(--shadow-card-hover);
	transform: scale(1.02);
}

.shade {
	position: absolute;
	inset: 0;
	background: linear-gradient(0deg, rgba(0, 0, 0, 0.55) 0%, rgba(0, 0, 0, 0) 45%);
}

.play {
	position: absolute;
	left: 12px;
	bottom: 12px;
	display: grid;
	place-items: center;
	width: 34px;
	height: 34px;
	border-radius: 50%;
	background: color-mix(in srgb, var(--player-ink) 92%, transparent);
	color: var(--fg-on-accent);
	font-size: 14px;
}

.left {
	position: absolute;
	right: 12px;
	bottom: 14px;
	color: var(--player-ink);
	font: 500 11px var(--font-mono);
	text-shadow: 0 1px 3px rgba(0, 0, 0, 0.8);
}

.bar {
	position: absolute;
	inset: auto 0 0 0;
	height: 3px;
	background: var(--player-track);
}

.bar > span {
	display: block;
	height: 100%;
	background: var(--player-accent);
}

.text {
	display: flex;
	flex-direction: column;
	gap: 2px;
}

.title {
	display: flex;
	align-items: baseline;
	gap: 8px;
	font-size: 14px;
	font-weight: 600;
}

.code {
	font: 500 11px var(--font-mono);
	color: var(--accent-text);
}

.sub {
	font-size: 12.5px;
	color: var(--fg-subtle);
}
</style>
