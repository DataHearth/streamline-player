<script lang="ts">
import Icon from "#lib/ui/Icon.svelte";

export type Row = {
	label: string;
	value: string;
	mode?: string;
	tone?: "copy" | "convert" | "warn";
};

let {
	label,
	summary,
	open,
	status,
	rows,
	audioTitle,
	audio,
	subsTitle,
	subs,
	ontoggle,
}: {
	label: string;
	summary: string;
	open: boolean;
	status: { label: string; where: string; copy: boolean };
	rows: Row[];
	audioTitle: string;
	audio: string[];
	subsTitle: string;
	/** Only a movie lists its subtitles here. */
	subs?: string;
	ontoggle: () => void;
} = $props();
</script>

<section>
	<button type="button" class="toggle tech-toggle" aria-expanded={open} onclick={ontoggle}>
		<Icon name="sliders" size={18} stroke={1.75} />
		<span class="name">{label}</span>
		<span class="summary">{summary}</span>
		<span class="grow"></span>
		<span class="chev" class:open><Icon name="chevron-down" size={18} stroke={2} /></span>
	</button>
	<div class="reveal" class:open>
		<div class="clip">
			<div class="body tech">
				<div class="card facts">
					<div class="status">
						<span class="dot" class:copy={status.copy}></span>
						<span class="what">{status.label}</span>
						<span class="where">{status.where}</span>
					</div>
					{#each rows as r (r.label)}
						<div class="fact">
							<span class="k">{r.label}</span>
							<span class="v">{r.value}</span>
							<span class="mode {r.tone ?? ""}">{r.mode ?? ""}</span>
						</div>
					{/each}
				</div>
				<div class="side">
					<div class="card pad">
						<div class="cap">{audioTitle}</div>
						<div class="tracks">
							{#each audio as a (a)}
								<div>{a}</div>
							{/each}
						</div>
					</div>
					{#if subs !== undefined}
						<div class="card pad">
							<div class="cap">{subsTitle}</div>
							<div class="subs">{subs}</div>
						</div>
					{/if}
				</div>
			</div>
		</div>
	</div>
</section>

<style>
section {
	display: flex;
	flex-direction: column;
	border-top: 1px solid var(--border);
	border-bottom: 1px solid var(--border);
}

.toggle {
	min-height: 60px;
	display: flex;
	align-items: center;
	gap: 14px;
	padding: 0;
	border: 0;
	background: transparent;
	color: var(--fg);
	text-align: left;
	cursor: pointer;
}

.toggle:hover {
	color: var(--accent-text);
}

.name {
	font-size: 16px;
	font-weight: 600;
	white-space: nowrap;
}

.summary {
	min-width: 0;
	font: 400 12px var(--font-mono);
	color: var(--fg-subtle);
	white-space: nowrap;
	overflow: hidden;
	text-overflow: ellipsis;
}

.grow {
	flex: 1;
}

.chev {
	display: grid;
	color: var(--fg-subtle);
	transition: transform var(--dur-base);
}

.chev.open {
	transform: rotate(180deg);
}

.reveal {
	display: grid;
	grid-template-rows: 0fr;
	visibility: hidden;
	transition:
		grid-template-rows 260ms var(--ease),
		visibility 260ms;
}

.reveal.open {
	grid-template-rows: 1fr;
	visibility: visible;
}

.clip {
	min-height: 0;
	overflow: hidden;
}

.body {
	display: grid;
	grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr);
	gap: 16px;
	padding: 4px 0 24px;
	opacity: 0;
	transform: translateY(8px);
	transition:
		opacity 260ms var(--ease),
		transform 260ms var(--ease);
}

.open .body {
	opacity: 1;
	transform: translateY(0);
}

.card {
	border: 1px solid var(--border);
	border-radius: 14px;
	background: var(--bg-elevated);
	overflow: hidden;
}

.card.pad {
	padding: 14px 16px;
	display: flex;
	flex-direction: column;
	gap: 8px;
}

.side {
	display: flex;
	flex-direction: column;
	gap: 16px;
}

.status {
	display: flex;
	align-items: center;
	gap: 9px;
	padding: 12px 16px;
	border-bottom: 1px solid var(--border);
}

.dot {
	width: 7px;
	height: 7px;
	border-radius: 50%;
	background: var(--status-downloading);
	box-shadow: 0 0 0 3px color-mix(in srgb, var(--status-downloading) 20%, transparent);
}

.dot.copy {
	background: var(--status-available);
	box-shadow: 0 0 0 3px color-mix(in srgb, var(--status-available) 18%, transparent);
}

.what {
	font-size: 13px;
	font-weight: 600;
}

.where {
	margin-left: auto;
	font: 400 11px var(--font-mono);
	color: var(--fg-subtle);
	white-space: nowrap;
}

.fact {
	display: grid;
	grid-template-columns: 84px minmax(0, 1fr) auto;
	gap: 12px;
	align-items: baseline;
	padding: 9px 16px;
	border-bottom: 1px solid var(--border);
}

.k,
.cap {
	font: 500 10.5px var(--font-mono);
	letter-spacing: 0.1em;
	text-transform: uppercase;
	color: var(--fg-faint);
}

.cap {
	letter-spacing: 0.14em;
}

.v {
	font: 400 12px var(--font-mono);
	color: var(--fg);
	overflow-wrap: anywhere;
}

.mode {
	font: 400 11px var(--font-mono);
	color: var(--fg-subtle);
	white-space: nowrap;
}

.mode.copy {
	color: var(--status-available);
}

.mode.convert {
	color: var(--status-downloading);
}

.mode.warn {
	color: var(--status-warning);
}

.tracks {
	display: flex;
	flex-direction: column;
	gap: 8px;
	font-size: 13px;
	color: var(--fg-muted);
}

.subs {
	font-size: 13px;
	line-height: 1.6;
	color: var(--fg-muted);
}
</style>
