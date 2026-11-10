<script lang="ts">
import Art from "#lib/ui/Art.svelte";
import Icon from "#lib/ui/Icon.svelte";

type Mark = { on: boolean; label: string; onclick: () => void };

let {
	title,
	sub,
	src,
	badge,
	watched = false,
	progress = 0,
	width,
	playLabel,
	mark,
	onplay,
	onclick,
}: {
	title: string;
	sub: string;
	src?: string;
	badge?: string;
	watched?: boolean;
	/** Percent of the unfinished play, drawn along the poster's foot. */
	progress?: number;
	/** A fixed width in a row; omitted, the card fills its grid cell. */
	width?: number;
	/** With `onplay`, the poster shows a play button on hover. */
	playLabel?: string;
	mark?: Mark;
	onplay?: () => void;
	onclick: () => void;
} = $props();
</script>

<div class="poster" style:width={width ? `${width}px` : undefined}>
	<button type="button" class="hit" aria-label={title} {onclick}></button>
	<span class="art">
		<Art {src} kind="poster" ratio="2 / 3" />
		{#if badge}
			<span class="badge">{badge}</span>
		{/if}
		{#if watched}
			<span class="seen"><Icon name="tick" size={13} stroke={2.5} /></span>
		{/if}
		{#if progress > 0 && !watched}
			<span class="bar"><span style:width="{progress}%"></span></span>
		{/if}
		{#if onplay}
			<span class="shade">
				<button
					type="button"
					class="play"
					aria-label={playLabel}
					title={playLabel}
					onclick={onplay}
				>
					<Icon name="play" filled size={20} />
				</button>
			</span>
		{/if}
	</span>
	<span class="text">
		<span class="lines">
			<span class="title">{title}</span>
			<span class="sub">{sub}</span>
		</span>
		{#if mark}
			<button
				type="button"
				class="mark"
				class:on={mark.on}
				aria-label={mark.label}
				title={mark.label}
				onclick={mark.onclick}
			>
				<Icon name={mark.on ? "check-check" : "eye"} size={16} stroke={1.75} />
			</button>
		{/if}
	</span>
</div>

<style>
.poster {
	position: relative;
	flex: none;
	display: flex;
	flex-direction: column;
	gap: 9px;
	min-width: 0;
	color: var(--fg);
	text-align: left;
	scroll-snap-align: start;
}

.hit {
	position: absolute;
	inset: 0;
	z-index: 0;
	padding: 0;
	border: 0;
	border-radius: var(--radius-md);
	background: transparent;
	cursor: pointer;
}

.art {
	position: relative;
	display: block;
	width: 100%;
	aspect-ratio: 2 / 3;
	border-radius: var(--radius-md);
	overflow: hidden;
	background: var(--bg-card);
	box-shadow: 0 0 0 1px var(--border);
	pointer-events: none;
	transition:
		box-shadow var(--dur-fast),
		transform var(--dur-fast);
}

.poster:hover .art {
	box-shadow: var(--shadow-card-hover);
	transform: scale(1.03);
}

.badge {
	position: absolute;
	left: 8px;
	top: 8px;
	padding: 2px 7px;
	border-radius: var(--radius-sm);
	background: color-mix(in srgb, var(--bg-deep) 78%, transparent);
	backdrop-filter: blur(8px);
	-webkit-backdrop-filter: blur(8px);
	font: 500 10px var(--font-mono);
	letter-spacing: 0.08em;
	text-transform: uppercase;
	color: var(--accent-text);
}

.seen {
	position: absolute;
	right: 8px;
	top: 8px;
	width: 22px;
	height: 22px;
	display: grid;
	place-items: center;
	border-radius: 50%;
	background: color-mix(in srgb, var(--bg-deep) 78%, transparent);
	color: var(--status-available);
}

.bar {
	position: absolute;
	left: 0;
	right: 0;
	bottom: 0;
	height: 3px;
	background: rgba(255, 255, 255, 0.2);
}

.bar > span {
	display: block;
	height: 100%;
	background: var(--accent);
}

.shade {
	position: absolute;
	inset: 0;
	display: grid;
	place-items: center;
	background: linear-gradient(
		180deg,
		transparent 30%,
		color-mix(in srgb, var(--bg-deep) 55%, transparent) 100%
	);
	opacity: 0;
	transition: opacity 180ms var(--ease);
}

.poster:hover .shade,
.poster:focus-within .shade {
	opacity: 1;
}

.play {
	z-index: 1;
	width: 52px;
	height: 52px;
	display: grid;
	place-items: center;
	padding: 0;
	border: 0;
	border-radius: 50%;
	background: color-mix(in srgb, var(--fg) 94%, transparent);
	color: var(--fg-on-accent);
	cursor: pointer;
	pointer-events: auto;
	box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
	transition: transform var(--dur-fast) var(--ease);
}

.play:hover {
	transform: scale(1.08);
}

.text {
	display: flex;
	align-items: center;
	gap: 4px;
	min-width: 0;
	pointer-events: none;
}

.lines {
	flex: 1;
	display: flex;
	flex-direction: column;
	gap: 1px;
	min-width: 0;
}

.title {
	font-size: 13.5px;
	font-weight: 500;
	white-space: nowrap;
	overflow: hidden;
	text-overflow: ellipsis;
}

.sub {
	font: 400 11.5px var(--font-mono);
	color: var(--fg-subtle);
	white-space: nowrap;
	overflow: hidden;
	text-overflow: ellipsis;
}

.mark {
	z-index: 1;
	flex: none;
	width: 34px;
	height: 34px;
	margin-right: -4px;
	display: grid;
	place-items: center;
	padding: 0;
	border: 0;
	border-radius: 50%;
	background: transparent;
	color: var(--fg-subtle);
	cursor: pointer;
	pointer-events: auto;
	transition: color 180ms;
}

.mark:hover {
	background: var(--surface-2);
}

.mark.on {
	color: var(--status-available);
}
</style>
