<script lang="ts">
import Art from "#lib/ui/Art.svelte";
import Icon from "#lib/ui/Icon.svelte";

let {
	title,
	sub,
	src,
	badge,
	watched = false,
	width = 164,
	onclick,
}: {
	title: string;
	sub: string;
	src?: string;
	badge?: string;
	watched?: boolean;
	width?: number;
	onclick: () => void;
} = $props();
</script>

<button type="button" class="poster" style:width="{width}px" {onclick}>
	<span class="art">
		<Art {src} kind="poster" ratio="2 / 3" />
		{#if badge}
			<span class="badge">{badge}</span>
		{/if}
		{#if watched}
			<span class="seen"><Icon name="tick" size={13} stroke={2.5} /></span>
		{/if}
	</span>
	<span class="text">
		<span class="title">{title}</span>
		<span class="sub">{sub}</span>
	</span>
</button>

<style>
.poster {
	flex: none;
	display: flex;
	flex-direction: column;
	gap: 9px;
	padding: 0;
	border: 0;
	background: transparent;
	color: var(--fg);
	text-align: left;
	cursor: pointer;
	scroll-snap-align: start;
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

.text {
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
</style>
