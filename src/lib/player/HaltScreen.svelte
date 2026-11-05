<script lang="ts">
	import { ChevronRight, RotateCw } from "@lucide/svelte";
	import { orPoster } from "#lib/tv/api.ts";
	import type { Halt } from "./halt.ts";
	import "./screen.css";

	let {
		halt,
		frame = "",
		poster = "",
		framePos = "",
		backLabel = "",
		onretry,
		onback,
	}: { halt: Halt; frame?: string; poster?: string; framePos?: string; backLabel?: string; onretry: () => void; onback?: () => void } = $props();
</script>

<div class="halt sl-full" role="alertdialog" aria-label={halt.eyebrow} data-tv-trap="1">
	{#if frame}<img class="frame" src={frame} alt="" style:object-position={framePos} onerror={orPoster(poster)} />{/if}
	<div class="halt-scrim"></div>
	<div class="halt-zoom">
		<div class="halt-col rise">
			<span class="eyebrow" style:color={halt.failed ? "var(--status-failed)" : "var(--status-warning)"}>
				<halt.icon size={14} strokeWidth={2} />
				{halt.eyebrow}
			</span>
			<span class="halt-title">{halt.title}</span>
			{#if halt.body}<span class="halt-body">{halt.body}</span>{/if}
			{#if halt.mono}<span class="halt-mono">{halt.mono}</span>{/if}
			{#if halt.count && halt.left && halt.total}
				<div class="count">
					<div class="count-track"><div class="count-fill" style:width="{(halt.left / halt.total) * 100}%" style:background={halt.failed ? "var(--player-ink-muted)" : "var(--status-warning)"}></div></div>
					<span class="count-text">{halt.count(halt.left)}</span>
				</div>
			{/if}
			{#if halt.wait}<span class="halt-wait"><span class="wait-dot"></span>{halt.wait}</span>{/if}
			{#if halt.alts?.length}
				<div class="alts">
					<span class="alts-title">Or start now, without a conversion</span>
					{#each halt.alts as alt}
						<button data-tv="1" class="alt" onclick={alt.run}>
							<alt.icon size={20} strokeWidth={1.75} />
							<span class="alt-text"><span class="alt-label">{alt.label}</span><span class="alt-sub">{alt.sub}</span></span>
							<span class="alt-go"><ChevronRight size={18} strokeWidth={2} /></span>
						</button>
					{/each}
				</div>
			{/if}
			{#if halt.note}<span class="halt-note">{halt.note}</span>{/if}
			<div class="actions">
				{#if halt.retryLabel}<button data-tv="1" data-tv-default="1" class="primary" onclick={onretry}><RotateCw size={16} strokeWidth={2} />{halt.retryLabel}</button>{/if}
				{#if onback}<button data-tv="1" data-tv-default={halt.retryLabel ? undefined : "1"} class="secondary" onclick={onback}>{backLabel}</button>{/if}
			</div>
		</div>
	</div>
</div>

<style>
	.halt {
		position: absolute;
		inset: 0;
		overflow: hidden;
		background: var(--bg-deep);
		animation: halt-fade 260ms var(--ease);
	}

	@keyframes halt-rise {
		from {
			transform: translateY(10px);
		}
	}

	.halt-note {
		font-size: 14px;
		line-height: 1.5;
		color: var(--player-ink-muted);
		text-wrap: pretty;
	}

	.halt-wait {
		display: flex;
		align-items: center;
		gap: 10px;
		margin-top: 2px;
		font: 400 12px var(--font-mono);
		color: var(--player-ink-muted);
	}

	.wait-dot {
		flex: none;
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--status-warning);
		animation: slp-pulse 1.6s ease-in-out infinite;
	}

	@keyframes slp-pulse {
		50% {
			opacity: 0.3;
		}
	}

	.halt-body {
		font-size: 16px;
		line-height: 1.5;
		color: var(--player-ink-muted);
		text-wrap: pretty;
	}

	.halt-mono {
		padding: 10px 12px;
		border-radius: var(--radius-md);
		background: color-mix(in srgb, var(--bg-elevated) 72%, transparent);
		border: 1px solid var(--border);
		font: 400 12px var(--font-mono);
		line-height: 1.5;
		color: var(--player-ink-muted);
		overflow-wrap: anywhere;
	}

	.count {
		display: flex;
		flex-direction: column;
		gap: 8px;
		max-width: 420px;
		margin-top: 4px;
	}

	.count-track {
		height: 4px;
		border-radius: var(--radius-full);
		background: var(--player-control-hover);
		overflow: hidden;
	}

	.count-fill {
		height: 100%;
		border-radius: var(--radius-full);
		transition: width 1s linear;
	}

	.count-text {
		font: 400 12px var(--font-mono);
		font-variant-numeric: tabular-nums;
		color: var(--player-ink-muted);
	}

	.alts {
		display: flex;
		flex-direction: column;
		gap: 8px;
		margin-top: 6px;
	}

	.alts-title {
		font: 500 11px var(--font-mono);
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--fg-subtle);
	}

	.alt {
		display: flex;
		align-items: center;
		gap: 14px;
		min-height: 60px;
		padding: 10px 16px;
		border-radius: 14px;
		border: 1px solid var(--border-strong);
		background: color-mix(in srgb, var(--bg-elevated) 72%, transparent);
		backdrop-filter: blur(12px);
		color: var(--player-ink);
		text-align: left;
		cursor: pointer;
		outline: none;
		transition: transform var(--dur-fast), background var(--dur-fast);
	}

	.alt:focus {
		background: var(--player-ink);
		color: var(--fg-on-accent);
		transform: scale(1.02);
	}

	.alt :global(svg) {
		flex: none;
	}

	.alt-text {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 3px;
	}

	.alt-label {
		font-size: 15px;
		font-weight: 600;
	}

	.alt-sub {
		font: 400 12px var(--font-mono);
		opacity: 0.72;
	}

	.alt-go {
		display: grid;
		opacity: 0.6;
	}

	.rise {
		animation: halt-rise 320ms var(--ease);
	}

	@keyframes halt-fade {
		from {
			opacity: 0;
		}
	}
</style>
