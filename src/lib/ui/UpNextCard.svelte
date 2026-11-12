<script lang="ts">
import { Play } from "@lucide/svelte";
import { hideBroken, orPoster } from "#lib/ui/img.ts";

let {
	code,
	title,
	still,
	stillPos,
	poster,
	bottom,
	progress,
	timer,
	timerLabel,
	playLabel,
	creditsLabel,
	onplay,
	oncredits,
}: {
	code: string;
	title: string;
	still: string;
	stillPos: string;
	poster?: string;
	bottom: number;
	progress: number;
	timer: boolean;
	timerLabel: string;
	playLabel: string;
	creditsLabel: string;
	onplay: () => void;
	oncredits: () => void;
} = $props();
</script>

<div class="card up" style:bottom="{bottom}px">
	<div class="art">
		<img
			src={still}
			alt=""
			style:object-position={stillPos}
			onerror={poster ? orPoster(poster) : hideBroken}
		>
		<div class="fade"></div>
		<div class="cap">
			<span class="code">{code}</span>
			<span class="title">{title}</span>
		</div>
		{#if timer}
			<div
				class="timer"
				role="progressbar"
				aria-label={timerLabel}
				aria-valuemin="0"
				aria-valuemax="100"
				aria-valuenow={Math.round(progress * 100)}
			>
				<div style:transform="scaleX({progress})"></div>
			</div>
		{/if}
	</div>
	<div class="row">
		<button type="button" class="ink" onclick={onplay}>
			<Play size={15} fill="currentColor" /><span>{playLabel}</span>
		</button>
		<button type="button" class="outline" onclick={oncredits}>{creditsLabel}</button>
	</div>
</div>

<style>
.card {
	position: absolute;
	right: 32px;
	width: 340px;
	overflow: hidden;
	border: 1px solid rgba(255, 255, 255, 0.12);
	border-radius: var(--radius-lg);
	background: var(--player-glass-strong);
	backdrop-filter: blur(24px);
	box-shadow: 0 24px 64px rgba(0, 0, 0, 0.6);
	color: var(--player-ink);
	animation: rise var(--dur-slow) var(--ease);
	transition: bottom 250ms var(--ease);
}

.art {
	position: relative;
	aspect-ratio: 16 / 8;
	overflow: hidden;
}

.art img {
	width: 100%;
	height: 100%;
	object-fit: cover;
	display: block;
}

.fade {
	position: absolute;
	inset: 0;
	background: linear-gradient(0deg, rgba(15, 15, 22, 0.95) 0%, rgba(15, 15, 22, 0) 60%);
}

.cap {
	position: absolute;
	left: 14px;
	right: 14px;
	bottom: 12px;
	display: flex;
	flex-direction: column;
	gap: 2px;
}

.code {
	font: 500 11px var(--font-mono);
	letter-spacing: 0.06em;
	color: var(--player-accent-text);
}

.title {
	font-size: 16px;
	font-weight: 600;
	white-space: nowrap;
	overflow: hidden;
	text-overflow: ellipsis;
}

.timer {
	position: absolute;
	inset: auto 0 0 0;
	height: 3px;
	background: rgba(255, 255, 255, 0.16);
}

.timer > div {
	height: 100%;
	background: var(--player-accent);
	transform-origin: 0 50%;
	transition: transform 250ms linear;
	will-change: transform;
}

.row {
	display: flex;
	gap: 8px;
	padding: 12px 14px 14px;
}

.ink {
	flex: 1;
	display: inline-flex;
	align-items: center;
	justify-content: center;
	gap: 9px;
	height: 40px;
	border: 0;
	border-radius: var(--radius-md);
	background: var(--player-ink);
	color: var(--fg-on-accent);
	font-size: 14px;
	font-weight: 600;
	cursor: pointer;
}

.ink:focus-visible {
	outline: none;
	box-shadow: 0 0 0 3px var(--player-accent);
}

.outline {
	height: 40px;
	padding: 0 14px;
	border: 1px solid rgba(255, 255, 255, 0.14);
	border-radius: var(--radius-md);
	background: transparent;
	color: var(--player-ink);
	font-size: 14px;
	font-weight: 500;
	cursor: pointer;
}

.outline:hover {
	background: var(--player-chip);
}

.outline:focus-visible {
	outline: none;
	background: var(--player-ink);
	color: var(--fg-on-accent);
}

@keyframes rise {
	from {
		opacity: 0;
		transform: translateY(10px);
	}
}
</style>
