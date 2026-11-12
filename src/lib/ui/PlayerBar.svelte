<script lang="ts">
import { Pause, Play, RotateCcw, RotateCw, SkipForward } from "@lucide/svelte";
import VolumeControl from "#lib/ui/VolumeControl.svelte";
import type { Component } from "svelte";

type Button = {
	id: string;
	icon: Component<{ size?: number; strokeWidth?: number }>;
	label: string;
	title: string;
	active: boolean;
	text?: string;
	disabled?: boolean;
	onclick: () => void;
};

let {
	playing,
	volume,
	muted,
	current,
	total,
	chapter,
	hasNext,
	buttons,
	labels,
	ontoggle,
	onseek,
	onnext,
	onvolume,
	onmute,
}: {
	playing: boolean;
	volume: number;
	muted: boolean;
	current: string;
	total: string;
	chapter: string;
	hasNext: boolean;
	buttons: Button[];
	labels: {
		play: string;
		pause: string;
		back: string;
		forward: string;
		next: string;
		mute: string;
		volume: string;
	};
	ontoggle: () => void;
	onseek: (delta: number) => void;
	onnext: () => void;
	onvolume: (v: number) => void;
	onmute: () => void;
} = $props();
</script>

<div class="bar">
	<button
		type="button"
		class="big"
		aria-label={playing ? labels.pause : labels.play}
		title={`${playing ? labels.pause : labels.play} (space)`}
		onclick={ontoggle}
	>
		{#if playing}
			<Pause size={26} fill="currentColor" strokeWidth={0} />
		{:else}
			<Play size={26} fill="currentColor" strokeWidth={0} />
		{/if}
	</button>
	<button
		type="button"
		class="skip"
		aria-label={labels.back}
		title={labels.back}
		onclick={() => onseek(-10)}
	>
		<RotateCcw size={22} strokeWidth={1.75} /><span>10</span>
	</button>
	<button
		type="button"
		class="skip"
		aria-label={labels.forward}
		title={labels.forward}
		onclick={() => onseek(10)}
	>
		<RotateCw size={22} strokeWidth={1.75} /><span>10</span>
	</button>
	{#if hasNext}
		<button
			type="button"
			class="plain"
			aria-label={labels.next}
			title={`${labels.next} (n)`}
			onclick={onnext}
		>
			<SkipForward size={22} fill="currentColor" />
		</button>
	{/if}
	<VolumeControl
		{volume}
		{muted}
		muteLabel={labels.mute}
		sliderLabel={labels.volume}
		{onvolume}
		{onmute}
	/>
	<span class="time">{current} <span class="of">/ {total}</span></span>
	<span class="chapter">{chapter}</span>
	<div class="grow"></div>
	{#each buttons as b (b.id)}
		<button
			type="button"
			class="menu"
			class:on={b.active}
			aria-label={b.label}
			title={b.title}
			disabled={b.disabled}
			onclick={b.onclick}
		>
			<b.icon size={21} strokeWidth={1.75} />
			{#if b.text}
				<span>{b.text}</span>
			{/if}
		</button>
	{/each}
</div>

<style>
.bar {
	display: flex;
	align-items: center;
	gap: 4px;
}

button {
	display: grid;
	place-items: center;
	border: 0;
	border-radius: var(--radius-player-controls);
	background: transparent;
	color: var(--player-ink);
	cursor: pointer;
}

button:hover:not(:disabled) {
	background: var(--player-control-hover);
}

.big {
	width: 48px;
	height: 48px;
}

.skip,
.plain {
	width: 44px;
	height: 44px;
	position: relative;
}

.skip > :global(svg) {
	grid-area: 1 / 1;
}

.skip span {
	grid-area: 1 / 1;
	font: 700 7.5px / 1 var(--font-mono);
	font-variant-numeric: tabular-nums;
}

.time {
	margin-left: 14px;
	font: 500 13px var(--font-mono);
	color: var(--player-ink);
	white-space: nowrap;
}

.of {
	color: var(--fg-subtle);
}

.chapter {
	margin-left: 12px;
	min-width: 0;
	font-size: 13px;
	color: var(--player-ink-muted);
	white-space: nowrap;
	overflow: hidden;
	text-overflow: ellipsis;
}

.grow {
	flex: 1;
}

.menu {
	height: 44px;
	min-width: 44px;
	display: flex;
	align-items: center;
	justify-content: center;
	gap: 6px;
	padding: 0 10px;
	font: 500 12px var(--font-mono);
	white-space: nowrap;
}

.menu.on {
	background: color-mix(in srgb, var(--player-accent) 22%, transparent);
}

.menu:disabled {
	opacity: 0.4;
	cursor: default;
}
</style>
