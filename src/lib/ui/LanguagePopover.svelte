<script lang="ts">
import Chip from "#lib/ui/Chip.svelte";
import Icon from "#lib/ui/Icon.svelte";

type Option = { value: string; label: string };

let {
	heading,
	title,
	audioLabel,
	subsLabel,
	audio,
	subs,
	draft,
	hint,
	applyLabel,
	unavailable,
	x,
	y,
	up,
	shown,
	onpick,
	onapply,
}: {
	/** Library's form: the title named on top, audio as a list, no hint beside Apply. */
	heading?: string;
	title: string;
	audioLabel: string;
	subsLabel: string;
	audio: Option[];
	subs: Option[];
	draft: { audio: string; subs: string };
	hint: string;
	applyLabel: string;
	/** Said instead of the lists while the file's tracks are not known. */
	unavailable?: string;
	x: number;
	y: number;
	up: boolean;
	shown: boolean;
	onpick: (patch: Partial<{ audio: string; subs: string }>) => void;
	onapply: () => void;
} = $props();
</script>

<div
	class="pop"
	role="dialog"
	aria-label={title}
	class:shown
	class:up
	class:wide={!!heading}
	style:left="{x}px"
	style:top="{y}px"
>
	{#if heading}
		<span class="heading">{heading}</span>
	{/if}
	{#if unavailable}
		<p class="none">{unavailable}</p>
	{:else}
		<div class="group">
			<div class="cap" class:pad={!!heading}>{audioLabel}</div>
			{#if heading}
				<div class="subs">
					{#each audio as o (o.value)}
						<label class="sub">
							<input
								type="radio"
								name="audio"
								value={o.value}
								checked={draft.audio === o.value}
								onchange={() => onpick({ audio: o.value })}
							>
							<span class="check" class:on={draft.audio === o.value}
								><Icon name="tick" size={14} stroke={2.5} /></span
							>{o.label}
						</label>
					{/each}
				</div>
			{:else}
				<div class="audio" role="radiogroup" aria-label={audioLabel}>
					{#each audio as o (o.value)}
						<Chip pressed={draft.audio === o.value} onclick={() => onpick({ audio: o.value })}>
							{o.label}
						</Chip>
					{/each}
				</div>
			{/if}
		</div>
		<div class="group tight">
			<div class="cap pad">{subsLabel}</div>
			<div class="subs">
				{#each subs as o (o.value)}
					<label class="sub">
						<input
							type="radio"
							name="subs"
							value={o.value}
							checked={draft.subs === o.value}
							onchange={() => onpick({ subs: o.value })}
						>
						<span class="check" class:on={draft.subs === o.value}
							><Icon name="tick" size={14} stroke={2.5} /></span
						>{o.label}
					</label>
				{/each}
			</div>
		</div>
	{/if}
	<div class="foot">
		{#if !heading}
			<span class="hint">{hint}</span>
		{/if}
		<button type="button" class="apply" disabled={!!unavailable} onclick={onapply}>
			{applyLabel}
		</button>
	</div>
</div>

<style>
.pop {
	position: absolute;
	z-index: 6;
	width: 264px;
	box-sizing: border-box;
	padding: 10px;
	display: flex;
	flex-direction: column;
	gap: 10px;
	border: 1px solid var(--border-strong);
	border-radius: 12px;
	background: var(--bg-elevated);
	box-shadow: var(--shadow-4);
	transform-origin: top right;
	opacity: 0;
	transform: translateY(-6px) scale(0.97);
	pointer-events: none;
	transition:
		opacity 130ms var(--ease-in),
		transform 150ms var(--ease-in);
}

.pop.up {
	transform-origin: bottom right;
	transform: translateY(-100%) translateY(6px) scale(0.97);
}

.pop.shown {
	opacity: 1;
	transform: translateY(0) scale(1);
	pointer-events: auto;
	transition:
		opacity 160ms var(--ease),
		transform 220ms var(--ease);
}

.pop.up.shown {
	transform: translateY(-100%) scale(1);
}

.pop.wide {
	width: 280px;
	padding: 14px;
	gap: 14px;
	border-radius: 14px;
}

.heading {
	font-size: 14px;
	font-weight: 600;
	line-height: 1.3;
}

.wide .group {
	gap: 2px;
}

.wide .cap {
	padding: 0 8px 4px;
	font-size: 10.5px;
}

.wide .subs {
	max-height: 170px;
}

.wide .sub {
	height: 34px;
	gap: 10px;
	padding: 0 8px;
	border-radius: 8px;
}

.wide .foot {
	justify-content: flex-end;
	padding: 0;
	border-top: 0;
}

.wide .apply {
	height: 34px;
	padding: 0 16px;
	border-radius: 9px;
}

.group {
	display: flex;
	flex-direction: column;
	gap: 6px;
}

.group.tight {
	gap: 2px;
}

.cap {
	padding: 0 4px;
	font: 500 10px var(--font-mono);
	letter-spacing: 0.14em;
	text-transform: uppercase;
	color: var(--fg-faint);
}

.cap.pad {
	padding-bottom: 4px;
}

.audio {
	display: flex;
	flex-wrap: wrap;
	gap: 6px;
}

.audio :global(.toggle) {
	height: 28px;
}

.subs {
	max-height: 150px;
	overflow-y: auto;
	display: flex;
	flex-direction: column;
}

.sub {
	flex: none;
	width: 100%;
	height: 30px;
	display: flex;
	align-items: center;
	gap: 8px;
	padding: 0 6px;
	border: 0;
	border-radius: 7px;
	background: transparent;
	color: var(--fg);
	font-size: 13px;
	text-align: left;
	cursor: pointer;
}

.sub input {
	position: absolute;
	opacity: 0;
	pointer-events: none;
}

.sub:has(input:focus-visible) {
	outline: 2px solid var(--accent);
}

.sub:hover {
	background: var(--surface-2);
}

.check {
	width: 14px;
	color: var(--accent);
	opacity: 0;
}

.check.on {
	opacity: 1;
}

.none {
	margin: 0;
	padding: 4px;
	font-size: 12.5px;
	line-height: 1.4;
	color: var(--fg-muted);
}

.foot {
	display: flex;
	align-items: center;
	gap: 10px;
	padding: 8px 2px 0;
	border-top: 1px solid var(--border);
}

.hint {
	flex: 1;
	min-width: 0;
	font-size: 11.5px;
	line-height: 1.35;
	color: var(--fg-subtle);
	text-wrap: pretty;
}

.apply {
	height: 30px;
	padding: 0 14px;
	border: 0;
	border-radius: 8px;
	background: var(--accent);
	color: var(--fg-on-accent);
	font-size: 13px;
	font-weight: 600;
	white-space: nowrap;
	cursor: pointer;
}

.apply:hover {
	background: var(--accent-hover);
}

.apply:disabled {
	opacity: 0.5;
	pointer-events: none;
}
</style>
