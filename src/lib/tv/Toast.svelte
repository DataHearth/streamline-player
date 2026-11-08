<script lang="ts">
	import { CircleCheck, Cpu, Gauge, Wifi } from "@lucide/svelte";
	import { activeToast } from "./toast.svelte.ts";

	const ICONS = { check: CircleCheck, wifi: Wifi, cpu: Cpu, gauge: Gauge };
	const t = $derived(activeToast());
	const Icon = $derived(t ? ICONS[t.icon] : null);
</script>

{#if t}
	<div class="stage">
		<div class="scale">
			{#key t.title}
				<div class="toast" class:leaving={t.leaving} style:--tone={t.warning ? "var(--status-warning)" : "var(--status-available)"} role="status" aria-live="polite">
					<Icon size={16} strokeWidth={1.5} />
					<div>
						<div class="title">{t.title}</div>
						{#if t.text}<div class="text">{t.text}</div>{/if}
					</div>
				</div>
			{/key}
		</div>
	</div>
{/if}

<style>
	.stage {
		position: fixed;
		left: 0;
		top: 0;
		width: 1920px;
		zoom: var(--tv-zoom, 1);
		z-index: 60;
		display: flex;
		justify-content: flex-end;
		padding: 48px 64px 0 0;
		pointer-events: none;
	}

	:global(body:has([data-tv-nav])) .stage {
		padding-top: 124px;
	}

	.scale {
		transform: scale(1.6);
		transform-origin: 100% 0;
	}

	.toast {
		position: relative;
		display: flex;
		align-items: flex-start;
		gap: 12px;
		width: 360px;
		padding: 12px 16px 12px 19px;
		overflow: hidden;
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--bg-elevated);
		box-shadow: var(--shadow-3);
		color: var(--fg);
		animation: slp-drop 260ms var(--ease);
		transition: opacity 180ms ease-in, transform 180ms ease-in;
	}

	.toast::before {
		content: "";
		position: absolute;
		inset: 0 auto 0 0;
		width: 3px;
		background: var(--tone);
	}

	.toast :global(svg) {
		flex: none;
		margin-top: 2px;
		color: var(--tone);
	}

	.toast.leaving {
		opacity: 0;
		transform: translateY(-8px);
	}

	.title {
		font-size: 14px;
		font-weight: 600;
	}

	.text {
		margin-top: 2px;
		font-size: 13px;
		color: var(--fg-muted);
	}

	@keyframes slp-drop {
		from {
			opacity: 0;
			transform: translateY(-10px);
		}
	}
</style>
