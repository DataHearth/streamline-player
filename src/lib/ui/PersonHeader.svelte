<script lang="ts">
import { hideBroken } from "#lib/ui/img.ts";

export type Known = {
	title: string;
	year: number;
	character?: string;
	poster?: string;
	onopen?: () => void;
};

let {
	name,
	department,
	summary,
	photo,
	initials,
	bg,
	fg,
	known,
	knownTitle,
	onServer,
	notInLibrary,
}: {
	name: string;
	department: string;
	summary: string;
	photo: string;
	initials: string;
	bg: string;
	fg: string;
	known: Known[];
	knownTitle: string;
	onServer: string;
	notInLibrary: string;
} = $props();
</script>

<div class="col">
	<div class="person">
		<span class="img" style:background={bg} style:color={fg}>
			{initials}<img src={photo} alt="" onerror={hideBroken}>
		</span>
		<div>
			<div class="eyebrow">{department}</div>
			<h1>{name}</h1>
			<div class="sum">{summary}</div>
		</div>
	</div>
	{#if known.length}
		<div>
			<h2>{knownTitle}</h2>
			<div class="known">
				{#each known as k, i (i)}
					{#snippet body()}
						<span
							class="p"
							style:background-image={k.poster ? `url(${k.poster})` : undefined}
						></span>
						<span class="text">
							<span class="t">{k.title}</span>
							<span class="s">{[k.year, k.character].filter(Boolean).join(" · ")}</span>
						</span>
						<span class="st" class:on={!!k.onopen}>{k.onopen ? onServer : notInLibrary}</span>
					{/snippet}
					{#if k.onopen}
						<button type="button" class="known-item" onclick={k.onopen}>{@render body()}</button>
					{:else}
						<div class="known-item out">{@render body()}</div>
					{/if}
				{/each}
			</div>
		</div>
	{/if}
</div>

<style>
.col {
	display: flex;
	flex-direction: column;
	gap: 28px;
}

.person {
	display: flex;
	align-items: center;
	gap: 28px;
}

.img {
	position: relative;
	display: grid;
	place-items: center;
	width: 136px;
	height: 136px;
	flex: none;
	overflow: hidden;
	border-radius: 50%;
	font-size: 46px;
	font-weight: 600;
	letter-spacing: -0.02em;
	box-shadow:
		0 0 0 1px rgba(255, 255, 255, 0.1),
		0 20px 48px rgba(0, 0, 0, 0.45);
}

.img img {
	position: absolute;
	inset: 0;
	width: 100%;
	height: 100%;
	object-fit: cover;
}

.eyebrow {
	display: flex;
	align-items: center;
	gap: 8px;
	font: 500 11px / 1 var(--font-mono);
	letter-spacing: 0.14em;
	text-transform: uppercase;
	color: var(--accent-text);
}

.eyebrow::before {
	content: "";
	width: 6px;
	height: 6px;
	border-radius: 50%;
	background: currentColor;
}

h1 {
	margin: 8px 0 6px;
	font-size: 44px;
	line-height: 1.05;
	font-weight: 700;
	letter-spacing: -0.03em;
}

.sum {
	font: 400 12.5px var(--font-mono);
	color: var(--fg-muted);
}

h2 {
	margin: 0 0 12px;
	font-size: 18px;
	font-weight: 600;
}

.known {
	display: grid;
	grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
	gap: 10px;
}

.known-item {
	display: flex;
	align-items: center;
	gap: 12px;
	min-height: 66px;
	padding: 10px 14px 10px 10px;
	border: 1px solid color-mix(in srgb, var(--fg) 10%, transparent);
	border-radius: var(--radius-player-controls);
	background: var(--bg-elevated);
	color: var(--fg);
	font: inherit;
	text-align: left;
}

button.known-item {
	cursor: pointer;
}

.known-item:hover {
	border-color: var(--accent-line);
}

.known-item.out {
	border-style: dashed;
	background: none;
}

.p {
	width: 31px;
	aspect-ratio: 2 / 3;
	flex: none;
	border-radius: 4px;
	background: var(--bg-card) center / cover;
}

.out .p {
	background: var(--bg-elevated);
	box-shadow: inset 0 0 0 1px var(--border);
}

.text {
	display: flex;
	flex-direction: column;
	min-width: 0;
}

.t {
	font-size: 14px;
	font-weight: 600;
}

.out .t {
	font-weight: 500;
	color: var(--fg-muted);
}

.s {
	font: 400 11.5px var(--font-mono);
	color: var(--fg-subtle);
}

.st {
	margin-left: auto;
	display: inline-flex;
	align-items: center;
	gap: 6px;
	font-size: 12px;
	white-space: nowrap;
	color: var(--fg-subtle);
}

.st.on {
	color: var(--status-available);
}

.st.on::before {
	content: "";
	width: 6px;
	height: 6px;
	border-radius: 50%;
	background: currentColor;
}
</style>
