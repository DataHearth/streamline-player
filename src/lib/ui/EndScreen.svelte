<script lang="ts">
import { Play, RotateCcw } from "@lucide/svelte";
import { hideBroken, orPoster } from "#lib/ui/img.ts";
import { onMount } from "svelte";

type Next = { code: string; title: string; overview: string; still: string; stillPos: string };

let {
	frame,
	framePos,
	poster,
	next,
	eyebrow,
	title,
	more,
	labels,
	onnext,
	ondone,
	onreplay,
	onopen,
}: {
	frame: string;
	framePos: string;
	poster?: string;
	next?: Next;
	eyebrow: string;
	title: string;
	more: { title: string; poster: string; href: string }[];
	labels: {
		upNext: string;
		playNext: string;
		done: string;
		again: string;
		more: string;
	};
	onnext: () => void;
	ondone: () => void;
	onreplay: () => void;
	onopen: (href: string) => void;
} = $props();

let primary = $state<HTMLButtonElement>();
onMount(() => primary?.focus());
</script>

<div class="end">
	<img class="blur" src={frame} alt="" style:object-position={framePos} onerror={hideBroken}>
	<div class="shade"></div>
	<div class="body">
		{#if next}
			<div class="next">
				<div class="text">
					<span class="eyebrow">{labels.upNext}</span>
					<span class="head">{next.title}</span>
					{#if next.overview}
						<span class="syn">{next.overview}</span>
					{/if}
					<div class="actions">
						<button type="button" class="ink" bind:this={primary} onclick={onnext}>
							<Play size={16} fill="currentColor" strokeWidth={0} />{labels.playNext}
						</button>
						<button type="button" class="outline" onclick={ondone}>{labels.done}</button>
					</div>
				</div>
				<div class="still">
					<img
						src={next.still}
						alt=""
						style:object-position={next.stillPos}
						onerror={poster ? orPoster(poster) : hideBroken}
					>
				</div>
			</div>
		{:else}
			<div class="finished">
				<div class="text rise">
					<span class="eyebrow">{eyebrow}</span>
					<span class="head">{title}</span>
					<div class="actions">
						<button type="button" class="ink plain" bind:this={primary} onclick={ondone}>
							{labels.done}
						</button>
						<button type="button" class="outline again" onclick={onreplay}>
							<RotateCcw size={16} strokeWidth={2} />{labels.again}
						</button>
					</div>
				</div>
				{#if more.length}
					<div class="more">
						<span class="more-title">{labels.more}</span>
						<div class="posters">
							{#each more as m}
								<button
									type="button"
									class="poster"
									aria-label={m.title}
									onclick={() => onopen(m.href)}
								>
									<img src={m.poster} alt="" loading="lazy" onerror={hideBroken}>
								</button>
							{/each}
						</div>
					</div>
				{/if}
			</div>
		{/if}
	</div>
</div>

<style>
.end {
	position: absolute;
	inset: 0;
	overflow: hidden;
	background: var(--bg-deep);
	animation: fade 320ms ease-out;
}

.blur {
	position: absolute;
	inset: 0;
	width: 100%;
	height: 100%;
	object-fit: cover;
	filter: blur(32px) saturate(1.2);
	transform: scale(1.15);
	opacity: 0.5;
}

.shade {
	position: absolute;
	inset: 0;
	background: linear-gradient(
		90deg,
		var(--hero-scrim) 0%,
		color-mix(in srgb, var(--bg-deep) 72%, transparent) 50%,
		color-mix(in srgb, var(--bg-deep) 40%, transparent) 100%
	);
}

.body {
	position: absolute;
	inset: 0;
	display: flex;
	flex-direction: column;
	justify-content: center;
	padding: 0 64px;
}

.next {
	display: flex;
	align-items: center;
	gap: 48px;
	animation: rise 360ms var(--ease);
}

.text {
	flex: 1 1 0;
	min-width: 0;
	max-width: 520px;
	display: flex;
	flex-direction: column;
	gap: 12px;
}

.finished {
	display: flex;
	flex-direction: column;
	gap: 36px;
}

.finished .text {
	flex: none;
	max-width: 560px;
}

.rise {
	animation: rise 360ms var(--ease);
}

.eyebrow {
	font: 500 11px var(--font-mono);
	letter-spacing: 0.14em;
	text-transform: uppercase;
	color: var(--player-accent-text);
}

.head {
	font-size: 36px;
	line-height: 1.1;
	font-weight: 700;
	letter-spacing: -0.02em;
	text-wrap: balance;
}

.syn {
	font-size: 14px;
	line-height: 1.55;
	color: var(--player-ink-muted);
	text-wrap: pretty;
}

.actions {
	display: flex;
	flex-wrap: wrap;
	gap: 10px;
	margin-top: 10px;
}

.ink {
	height: 48px;
	display: flex;
	align-items: center;
	gap: 10px;
	padding: 0 22px 0 18px;
	border: 0;
	border-radius: var(--radius-player-controls);
	background: var(--player-ink);
	color: var(--fg-on-accent);
	font-size: 15px;
	font-weight: 600;
	cursor: pointer;
	white-space: nowrap;
}

.ink.plain {
	padding: 0 26px;
}

.ink:hover {
	background: white;
}

.ink:focus-visible {
	outline: none;
	box-shadow: 0 0 0 3px var(--player-accent);
}

.outline {
	height: 48px;
	padding: 0 20px;
	border-radius: var(--radius-player-controls);
	border: 1px solid rgba(255, 255, 255, 0.18);
	background: transparent;
	color: var(--player-ink);
	font-size: 15px;
	font-weight: 500;
	cursor: pointer;
	white-space: nowrap;
}

.outline.again {
	display: flex;
	align-items: center;
	gap: 8px;
	padding: 0 20px 0 16px;
}

.outline:hover {
	background: var(--player-chip);
}

.outline:focus-visible {
	outline: none;
	background: var(--player-ink);
	color: var(--fg-on-accent);
}

.still {
	flex: none;
	width: 420px;
	aspect-ratio: 16 / 9;
	border-radius: 14px;
	overflow: hidden;
	box-shadow:
		0 24px 64px rgba(0, 0, 0, 0.55),
		0 0 0 1px var(--border);
}

.still img {
	width: 100%;
	height: 100%;
	object-fit: cover;
	display: block;
}

.more {
	display: flex;
	flex-direction: column;
	gap: 12px;
	animation: rise 440ms var(--ease);
}

.more-title {
	font-size: 15px;
	font-weight: 600;
}

.posters {
	display: flex;
	gap: 14px;
}

.poster {
	flex: none;
	width: 120px;
	padding: 0;
	border: 0;
	border-radius: var(--radius-md);
	background: transparent;
	cursor: pointer;
	transition:
		transform var(--dur-fast),
		box-shadow var(--dur-fast);
}

.poster:hover {
	transform: scale(1.04);
}

.poster:focus-visible {
	outline: none;
	transform: scale(1.06);
	box-shadow: 0 0 0 3px var(--player-accent);
}

.poster img {
	width: 100%;
	aspect-ratio: 2 / 3;
	border-radius: var(--radius-md);
	object-fit: cover;
	display: block;
	box-shadow: 0 0 0 1px var(--border);
}

@keyframes fade {
	from {
		opacity: 0;
	}
}

@keyframes rise {
	from {
		opacity: 0;
		transform: translateY(10px);
	}
}
</style>
