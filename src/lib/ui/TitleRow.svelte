<script lang="ts">
import Icon from "#lib/ui/Icon.svelte";

let {
	title,
	poster,
	watched,
	progress,
	year,
	res,
	hdr,
	hdrOn,
	audio,
	size,
	added,
	onclick,
}: {
	title: string;
	poster?: string;
	watched: boolean;
	progress: number;
	year: string;
	res: string;
	hdr: string;
	hdrOn: boolean;
	audio: string;
	size: string;
	added: string;
	onclick: () => void;
} = $props();

let broken = $state(false);
</script>

<button type="button" class="row" {onclick}>
	<span class="thumb">
		{#if poster && !broken}
			<img src={poster} alt="" loading="lazy" onerror={() => (broken = true)}>
		{/if}
	</span>
	<span class="main">
		<span class="line">
			<span class="title">{title}</span>
			{#if watched}
				<span class="seen"><Icon name="tick" size={13} stroke={2.5} /></span>
			{/if}
		</span>
		{#if progress > 0 && !watched}
			<span class="bar"><span style:width="{progress}%"></span></span>
		{/if}
	</span>
	<span class="cell muted">{year}</span>
	<span class="cell">{res}</span>
	<span class="cell" class:faint={!hdrOn}>{hdr}</span>
	<span class="cell muted clip">{audio}</span>
	<span class="cell muted end">{size}</span>
	<span class="cell subtle end">{added}</span>
</button>

<style>
.row {
	display: grid;
	grid-template-columns: var(--cols);
	gap: 16px;
	align-items: center;
	min-height: 62px;
	padding: 8px 12px;
	border: 0;
	border-bottom: 1px solid var(--border);
	border-radius: 0;
	background: transparent;
	color: var(--fg);
	text-align: left;
	cursor: pointer;
}

.row:hover {
	background: var(--surface);
}

.thumb {
	display: block;
	width: 32px;
	aspect-ratio: 2 / 3;
	border-radius: 4px;
	overflow: hidden;
	background: var(--bg-card);
}

img {
	width: 100%;
	height: 100%;
	object-fit: cover;
	display: block;
}

.main {
	display: flex;
	flex-direction: column;
	gap: 4px;
	min-width: 0;
}

.line {
	display: flex;
	align-items: center;
	gap: 8px;
	min-width: 0;
}

.title {
	font-size: 14px;
	font-weight: 500;
	white-space: nowrap;
	overflow: hidden;
	text-overflow: ellipsis;
}

.seen {
	flex: none;
	display: grid;
	color: var(--status-available);
}

.bar {
	width: 120px;
	height: 3px;
	border-radius: 2px;
	background: rgba(255, 255, 255, 0.12);
	overflow: hidden;
}

.bar > span {
	display: block;
	height: 100%;
	background: var(--accent);
}

.cell {
	font: 400 12px var(--font-mono);
	color: var(--fg);
	white-space: nowrap;
}

.muted {
	color: var(--fg-muted);
}

.subtle {
	color: var(--fg-subtle);
}

.faint {
	color: var(--fg-faint);
}

.clip {
	overflow: hidden;
	text-overflow: ellipsis;
}

.end {
	text-align: right;
}
</style>
