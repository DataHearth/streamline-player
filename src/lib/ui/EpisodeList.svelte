<script lang="ts">
import Art from "#lib/ui/Art.svelte";
import Icon from "#lib/ui/Icon.svelte";

export type Ep = {
	key: string;
	num: string;
	title: string;
	synopsis: string;
	runtime: string;
	still: string;
	pos: string;
	state: "wanted" | "watched" | "progress" | "unwatched";
	isNew: boolean;
	pct: number;
	pill: { text: string; tone: "warn" | "accent" } | null;
	playLabel: string;
	markLabel: string;
	langLabel: string;
	langOwn: boolean;
};

let {
	seasons,
	season,
	note,
	seasonMark,
	seasonLang,
	eps,
	newLabel,
	minWidth = 252,
	onseason,
	onplay,
	onmark,
	onlang,
}: {
	seasons: { n: number; label: string }[];
	season: number;
	note: string;
	seasonMark: { label: string; watched: boolean; onclick: () => void };
	seasonLang: { label: string; onclick: (e: MouseEvent) => void };
	eps: Ep[];
	newLabel: string;
	minWidth?: number;
	onseason: (n: number) => void;
	onplay: (key: string) => void;
	onmark: (key: string) => void;
	onlang: (key: string, e: MouseEvent) => void;
} = $props();
</script>

<section>
	<div class="bar">
		<div class="seasons">
			{#each seasons as s (s.n)}
				<button
					type="button"
					class="season"
					aria-pressed={s.n === season}
					onclick={() => onseason(s.n)}
				>
					{s.label}
				</button>
			{/each}
		</div>
		<span class="season-note">{note}</span>
		<button
			type="button"
			class="round season-mark"
			class:done={seasonMark.watched}
			aria-label={seasonMark.label}
			title={seasonMark.label}
			onclick={seasonMark.onclick}
		>
			<Icon name={seasonMark.watched ? "check-check" : "eye"} size={17} stroke={1.8} />
		</button>
		<button
			type="button"
			class="round"
			data-lang-trigger="season"
			aria-haspopup="dialog"
			aria-label={seasonLang.label}
			title={seasonLang.label}
			onclick={seasonLang.onclick}
		>
			<Icon name="languages" size={17} stroke={1.8} />
		</button>
	</div>
	<div class="grid" style:--min="{minWidth}px">
		{#each eps as e (e.key)}
			<div class="ep" class:wanted={e.state === "wanted"} data-state={e.state}>
				<span class="still">
					<span class="art" style:filter={e.state === "wanted" ? "grayscale(1)" : undefined}>
						<Art src={e.still} position={e.pos} />
					</span>
					{#if e.state !== "wanted"}
						<button
							type="button"
							class="hit play-ep"
							aria-label={e.playLabel}
							onclick={() => onplay(e.key)}
						></button>
						<span class="disc"><Icon name="play" filled size={13} /></span>
					{/if}
					{#if e.isNew}
						<span class="new">{newLabel}</span>
					{/if}
					{#if e.state === "watched"}
						<span class="seen"><Icon name="tick" size={13} stroke={2.5} /></span>
					{/if}
					{#if e.state === "progress"}
						<span class="bar-p"><span style:width="{e.pct}%"></span></span>
					{/if}
				</span>
				<span class="text">
					<span class="line">
						<span class="num">{e.num}</span>
						<span class="name">{e.title}</span>
					</span>
					<span class="sub">
						<span>{e.runtime}</span>
						{#if e.pill}
							<span class="pill {e.pill.tone}"><span class="pd"></span>{e.pill.text}</span>
						{/if}
						{#if e.state !== "wanted"}
							<button
								type="button"
								class="tiny mark"
								class:done={e.state === "watched"}
								aria-label={e.markLabel}
								title={e.markLabel}
								onclick={() => onmark(e.key)}
							>
								<Icon name={e.state === "watched" ? "check-check" : "eye"} size={17} stroke={1.8} />
							</button>
							<button
								type="button"
								class="tiny lang"
								class:own={e.langOwn}
								data-lang-trigger="ep"
								aria-label={e.langLabel}
								title={e.langLabel}
								onclick={(ev) => onlang(e.key, ev)}
							>
								<Icon name="languages" size={17} stroke={1.8} />
							</button>
						{/if}
					</span>
					{#if e.synopsis}
						<span class="syn">{e.synopsis}</span>
					{/if}
				</span>
			</div>
		{/each}
	</div>
</section>

<style>
section {
	position: relative;
	display: flex;
	flex-direction: column;
	gap: 16px;
}

.bar {
	display: flex;
	align-items: center;
	gap: 8px;
}

.seasons {
	display: flex;
	align-items: center;
	gap: 8px;
}

.season {
	height: 36px;
	padding: 0 15px;
	border: 1px solid var(--border-strong);
	border-radius: var(--radius-full);
	background: transparent;
	color: var(--fg-subtle);
	font-size: 13.5px;
	font-weight: 500;
	white-space: nowrap;
	cursor: pointer;
}

.season:hover {
	color: var(--fg);
}

.season[aria-pressed="true"] {
	border-color: var(--accent-line);
	background: var(--accent-soft);
	color: var(--fg);
	font-weight: 600;
}

.season-note {
	margin-left: auto;
	font: 400 12px var(--font-mono);
	color: var(--fg-subtle);
	white-space: nowrap;
}

.round,
.tiny {
	flex: none;
	display: grid;
	place-items: center;
	width: 40px;
	height: 40px;
	padding: 0;
	border: 0;
	border-radius: 50%;
	background: transparent;
	color: var(--fg-subtle);
	cursor: pointer;
}

.round:hover,
.tiny:hover {
	background: var(--surface-2);
}

.round.season-mark {
	margin-left: 6px;
	color: var(--fg-muted);
	transition: color 180ms;
}

.round:not(.season-mark):hover {
	color: var(--fg);
}

.done {
	color: var(--status-available) !important;
}

.grid {
	display: grid;
	grid-template-columns: repeat(auto-fill, minmax(var(--min), 1fr));
	gap: 26px 18px;
}

.ep {
	position: relative;
	display: flex;
	flex-direction: column;
	gap: 10px;
	min-width: 0;
}

.ep.wanted {
	opacity: 0.55;
}

.still {
	position: relative;
	display: block;
	width: 100%;
	aspect-ratio: 16 / 9;
	border-radius: var(--radius-md);
	overflow: hidden;
	background: var(--bg-card);
	box-shadow: 0 0 0 1px var(--border);
	transition:
		box-shadow var(--dur-fast),
		transform var(--dur-fast);
}

.ep:not(.wanted):hover .still {
	box-shadow:
		0 0 0 2px color-mix(in srgb, var(--accent) 55%, transparent),
		0 16px 32px rgba(0, 0, 0, 0.45);
	transform: scale(1.02);
}

.art {
	position: absolute;
	inset: 0;
	display: block;
}

.hit {
	position: absolute;
	inset: 0;
	z-index: 1;
	padding: 0;
	border: 0;
	background: transparent;
	cursor: pointer;
}

.disc {
	position: absolute;
	left: 10px;
	bottom: 10px;
	display: grid;
	place-items: center;
	width: 32px;
	height: 32px;
	padding-left: 2px;
	box-sizing: border-box;
	border-radius: 50%;
	background: color-mix(in srgb, var(--fg) 92%, transparent);
	color: var(--fg-on-accent);
	pointer-events: none;
}

.new {
	position: absolute;
	left: 8px;
	top: 8px;
	padding: 2px 7px;
	border-radius: var(--radius-sm);
	background: color-mix(in srgb, var(--bg-deep) 80%, transparent);
	font: 500 10px var(--font-mono);
	letter-spacing: 0.08em;
	text-transform: uppercase;
	color: var(--accent-text);
}

.seen {
	position: absolute;
	right: 8px;
	top: 8px;
	display: grid;
	place-items: center;
	width: 22px;
	height: 22px;
	border-radius: 50%;
	background: color-mix(in srgb, var(--bg-deep) 80%, transparent);
	color: var(--status-available);
}

.bar-p {
	position: absolute;
	left: 0;
	right: 0;
	bottom: 0;
	height: 3px;
	background: rgba(255, 255, 255, 0.2);
}

.bar-p > span {
	display: block;
	height: 100%;
	background: var(--accent);
}

.text {
	display: flex;
	flex-direction: column;
	gap: 4px;
	min-width: 0;
}

.line {
	display: flex;
	align-items: baseline;
	gap: 8px;
	min-width: 0;
}

.num {
	flex: none;
	font: 500 11.5px var(--font-mono);
	color: var(--fg-subtle);
}

.name {
	font-size: 14.5px;
	font-weight: 600;
	letter-spacing: -0.01em;
	white-space: nowrap;
	overflow: hidden;
	text-overflow: ellipsis;
}

.sub {
	display: flex;
	align-items: center;
	gap: 8px;
	min-height: 24px;
	font: 400 11.5px var(--font-mono);
	color: var(--fg-subtle);
}

.pill {
	display: inline-flex;
	align-items: center;
	gap: 5px;
	padding: 1px 8px;
	border: 1px solid;
	border-radius: var(--radius-full);
	font: 500 11px var(--font-sans);
}

.pill.warn {
	color: var(--status-warning);
	background: color-mix(in srgb, var(--status-warning) 12%, transparent);
	border-color: color-mix(in srgb, var(--status-warning) 25%, transparent);
}

.pill.accent {
	color: var(--accent-text);
	background: color-mix(in srgb, var(--accent) 12%, transparent);
	border-color: color-mix(in srgb, var(--accent) 30%, transparent);
}

.pd {
	width: 5px;
	height: 5px;
	border-radius: 50%;
	background: currentColor;
}

.tiny {
	margin: -8px -6px -8px 0;
}

.tiny.mark {
	margin-left: auto;
	transition: color 180ms;
}

.tiny.lang.own {
	color: var(--accent);
}

.syn {
	font-size: 13px;
	line-height: 1.5;
	color: var(--fg-muted);
	display: -webkit-box;
	-webkit-line-clamp: 2;
	line-clamp: 2;
	-webkit-box-orient: vertical;
	overflow: hidden;
}
</style>
