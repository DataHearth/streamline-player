<script lang="ts">
import Art from "#lib/ui/Art.svelte";
import Chip from "#lib/ui/Chip.svelte";
import Icon from "#lib/ui/Icon.svelte";
import { useScroll } from "#lib/ui/scroll.ts";
import { onMount } from "svelte";

export type Slide = {
	key: string;
	img: string;
	eyebrow: string;
	title: string;
	meta: string;
	chips: string[];
	overview: string;
	primary: string;
	progress?: { pct: number; left: string };
	onprimary: () => void;
	oninfo: () => void;
};

const HERO_MS = 8000;
const TICK_MS = 100;
const SWAP_MS = 260;
const SCROLLED_PAST = 320;
const KEN_BURNS = 1.08;

let {
	slides,
	height,
	padX = 56,
	infoLabel,
	dotLabel,
}: {
	slides: Slide[];
	height: number;
	padX?: number;
	infoLabel: string;
	dotLabel: (title: string) => string;
} = $props();

const scroller = useScroll();
let index = $state(0);
let progress = $state(0);
let phase = $state<"pre" | "in" | "out">("pre");
let hero = $state<HTMLElement>();
let reduceMotion = false;
let swap: ReturnType<typeof setTimeout>;

const at = $derived(slides.length ? index % slides.length : 0);
const slide = $derived(slides[at]);

const settle = () => requestAnimationFrame(() => requestAnimationFrame(() => (phase = "in")));

function go(i: number) {
	if (i === at || phase === "out") return;
	clearTimeout(swap);
	phase = "out";
	swap = setTimeout(() => {
		index = i;
		progress = 0;
		phase = "pre";
		settle();
	}, SWAP_MS);
}

onMount(() => {
	reduceMotion = matchMedia("(prefers-reduced-motion: reduce)").matches;
	settle();
	const timer = setInterval(() => {
		if (hero?.matches(":hover") || phase !== "in" || slides.length < 2) return;
		if ((scroller?.()?.scrollTop ?? 0) > SCROLLED_PAST) return;
		if (document.hidden) return;
		const next = progress + TICK_MS / HERO_MS;
		if (next >= 1) go((at + 1) % slides.length);
		else progress = next;
	}, TICK_MS);
	return () => {
		clearInterval(timer);
		clearTimeout(swap);
	};
});

const text = $derived(
	phase === "out"
		? "opacity:0;transform:translateY(-10px);transition:opacity 240ms var(--ease-in), transform 240ms var(--ease-in)"
		: phase === "pre"
			? "opacity:0;transform:translateY(18px);transition:none"
			: "opacity:1;transform:translateY(0);transition:opacity 600ms var(--ease) 80ms, transform 700ms var(--ease) 80ms",
);
const frameStyle = (i: number) => {
	const on = i === at;
	const zoom = on && !reduceMotion && phase !== "pre";
	return [
		`opacity:${on ? 1 : 0}`,
		`transform:${zoom ? `scale(${KEN_BURNS})` : "scale(1)"}`,
		`transform-origin:${i % 2 ? "30% 40%" : "70% 35%"}`,
		`transition:${on ? `opacity 1100ms var(--ease), transform ${HERO_MS + 1500}ms linear` : "opacity 1100ms var(--ease), transform 0ms linear 1100ms"}`,
	].join(";");
};
</script>

{#if slide}
	<section class="hero" bind:this={hero} style:height="{height}px">
		{#each slides as s, i (s.key)}
			<div class="frame" style={frameStyle(i)}>
				<Art src={s.img} position="50% 30%" />
			</div>
		{/each}
		<div class="side"></div>
		<div class="fade"></div>
		<div class="body" style:left="{padX}px" style="{text}">
			<div class="eyebrow">{slide.eyebrow}</div>
			<h1>{slide.title}</h1>
			<div class="meta">
				<span>{slide.meta}</span>
				{#each slide.chips as c}
					<Chip>{c}</Chip>
				{/each}
			</div>
			<p>{slide.overview}</p>
			<div class="actions">
				<button type="button" class="cta" onclick={slide.onprimary}>
					<Icon name="play" filled size={18} />{slide.primary}
				</button>
				<button type="button" class="glass" onclick={slide.oninfo}>
					<Icon name="info" size={18} />{infoLabel}
				</button>
				{#if slide.progress}
					<div class="resume">
						<i><b style:width="{slide.progress.pct}%"></b></i>{slide.progress.left}
					</div>
				{/if}
			</div>
		</div>
		{#if slides.length > 1}
			<div class="dots" style:right="{padX}px">
				{#each slides as s, i (s.key)}
					<button
						type="button"
						class="dot"
						style:width="{i === at ? 44 : 18}px"
						aria-label={dotLabel(s.title)}
						onclick={() => go(i)}
					>
						<span class="track"
							><span
								class="fill"
								style:width="{i === at ? Math.round(progress * 1000) / 10 : 0}%"
								style:transition={i === at && progress > 0 ? `width ${TICK_MS}ms linear` : "none"}
							></span></span
						>
					</button>
				{/each}
			</div>
		{/if}
	</section>
{/if}

<style>
.hero {
	position: relative;
	overflow: hidden;
	color: var(--player-ink);
}

.frame {
	position: absolute;
	inset: 0;
}

.side {
	position: absolute;
	inset: 0;
	background: linear-gradient(
		90deg,
		var(--hero-scrim) 0%,
		color-mix(in srgb, var(--bg-deep) 62%, transparent) 34%,
		color-mix(in srgb, var(--bg-deep) 5%, transparent) 68%
	);
}

.fade {
	position: absolute;
	inset: 0;
	background: linear-gradient(
		0deg,
		var(--bg-deep) 0%,
		color-mix(in srgb, var(--bg-deep) 60%, transparent) 22%,
		transparent 48%
	);
}

.body {
	position: absolute;
	bottom: 150px;
	width: min(600px, 52%);
	display: flex;
	flex-direction: column;
	gap: 14px;
	color: var(--player-ink);
}

.eyebrow {
	display: flex;
	align-items: center;
	gap: 8px;
	font: 500 11px / 1 var(--font-mono);
	letter-spacing: 0.14em;
	text-transform: uppercase;
	color: var(--player-accent-text);
	white-space: nowrap;
}

.eyebrow::before {
	content: "";
	width: 6px;
	height: 6px;
	border-radius: 50%;
	background: var(--player-accent);
}

h1 {
	margin: 0;
	font-size: 58px;
	line-height: 1.02;
	font-weight: 700;
	letter-spacing: -0.03em;
	text-wrap: balance;
}

.meta {
	display: flex;
	flex-wrap: wrap;
	align-items: center;
	gap: 8px;
	font: 400 12.5px var(--font-mono);
	color: var(--player-ink-muted);
}

p {
	margin: 0;
	max-width: 540px;
	font-size: 15px;
	line-height: 1.55;
	color: var(--player-ink-muted);
	text-wrap: pretty;
}

.actions {
	display: flex;
	align-items: center;
	gap: 10px;
	margin-top: 6px;
}

.cta {
	display: inline-flex;
	align-items: center;
	gap: 10px;
	height: 46px;
	padding: 0 22px 0 18px;
	border: 0;
	border-radius: var(--radius-md);
	background: var(--player-accent);
	color: var(--fg-on-accent);
	font-size: 15px;
	font-weight: 600;
	cursor: pointer;
	transition:
		background 150ms,
		box-shadow 150ms,
		transform 150ms;
}

.cta:hover {
	background: var(--accent-hover);
	box-shadow: 0 0 24px color-mix(in srgb, var(--accent) 35%, transparent);
}

.cta:active {
	transform: scale(0.97);
}

.glass {
	display: inline-flex;
	align-items: center;
	gap: 9px;
	flex: none;
	white-space: nowrap;
	height: 46px;
	padding: 0 18px;
	border: 1px solid rgba(255, 255, 255, 0.16);
	border-radius: var(--radius-md);
	background: var(--player-chip);
	backdrop-filter: blur(12px);
	-webkit-backdrop-filter: blur(12px);
	color: var(--player-ink);
	font-size: 15px;
	font-weight: 500;
	cursor: pointer;
}

.glass:hover {
	background: rgba(255, 255, 255, 0.14);
}

.glass :global(.sl-ico) {
	stroke-width: 1.75;
}

.resume {
	display: flex;
	flex-direction: column;
	gap: 6px;
	width: 140px;
	margin-left: 10px;
	font: 400 11.5px var(--font-mono);
	color: var(--player-ink-muted);
}

.resume > i {
	display: block;
	height: 3px;
	border-radius: 2px;
	background: rgba(255, 255, 255, 0.18);
	overflow: hidden;
}

.resume > i > b {
	display: block;
	height: 100%;
	background: var(--player-ink);
}

.dots {
	position: absolute;
	bottom: 156px;
	display: flex;
	gap: 6px;
}

.dot {
	height: 24px;
	padding: 0;
	border: 0;
	background: transparent;
	cursor: pointer;
	display: flex;
	align-items: center;
	transition: width 400ms var(--ease);
}

.track {
	position: relative;
	display: block;
	width: 100%;
	height: 4px;
	border-radius: 2px;
	background: color-mix(in srgb, var(--fg) 30%, transparent);
	overflow: hidden;
}

.fill {
	position: absolute;
	left: 0;
	top: 0;
	bottom: 0;
	background: var(--player-ink);
}
</style>
