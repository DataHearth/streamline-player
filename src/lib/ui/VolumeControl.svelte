<script lang="ts">
import { Volume1, Volume2, VolumeX } from "@lucide/svelte";

let {
	volume,
	muted,
	muteLabel,
	sliderLabel,
	onvolume,
	onmute,
}: {
	volume: number;
	muted: boolean;
	muteLabel: string;
	sliderLabel: string;
	onvolume: (v: number) => void;
	onmute: () => void;
} = $props();

const shown = $derived(muted ? 0 : Math.round(volume * 100));

function at(e: PointerEvent) {
	const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
	onvolume(Math.max(0, Math.min(1, (e.clientX - r.left) / r.width)));
}
</script>

<div class="vol">
	<button
		type="button"
		class="mute"
		aria-label={muteLabel}
		title={`${muteLabel} (m)`}
		onclick={onmute}
	>
		{#if muted || volume === 0}
			<VolumeX size={21} strokeWidth={1.75} />
		{:else if volume < 0.5}
			<Volume1 size={21} strokeWidth={1.75} />
		{:else}
			<Volume2 size={21} strokeWidth={1.75} />
		{/if}
	</button>
	<div
		class="slider"
		role="slider"
		tabindex="-1"
		aria-label={sliderLabel}
		aria-valuemin="0"
		aria-valuemax="100"
		aria-valuenow={shown}
		onpointerdown={(e) => {
			(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
			at(e);
		}}
		onpointermove={(e) => {
			if ((e.currentTarget as HTMLElement).hasPointerCapture(e.pointerId)) at(e);
		}}
	>
		<div class="rail"><div class="level" style:width="{shown}%"></div></div>
	</div>
</div>

<style>
.vol {
	display: flex;
	align-items: center;
	gap: 6px;
	margin-left: 4px;
}

.mute {
	width: 40px;
	height: 40px;
	display: grid;
	place-items: center;
	border: 0;
	border-radius: var(--radius-player-controls);
	background: transparent;
	color: var(--player-ink);
	cursor: pointer;
}

.mute:hover {
	background: var(--player-control-hover);
}

.slider {
	width: 92px;
	height: 24px;
	display: flex;
	align-items: center;
	cursor: pointer;
	touch-action: none;
}

.rail {
	position: relative;
	width: 100%;
	height: 4px;
	border-radius: 2px;
	background: var(--player-buffer);
}

.level {
	position: absolute;
	left: 0;
	top: 0;
	bottom: 0;
	border-radius: 2px;
	background: var(--player-ink);
}
</style>
