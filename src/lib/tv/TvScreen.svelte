<script lang="ts">
import { afterNavigate } from "$app/navigation";
import { appPath, goto, staticUrl } from "#lib/ui/nav.ts";
import { m as i18n } from "#lib/paraglide/messages.js";
import { getLocale } from "#lib/paraglide/runtime.js";
import { search } from "#lib/core/search.svelte.ts";
import { onPlayer, voice as hear } from "#platform";
import { Film, House, Search, Settings, TvMinimal } from "@lucide/svelte";
import { onMount, type Snippet } from "svelte";
import { focus, focusDefault, forgetFocus, move } from "./focus";

type Tab = "search" | "home" | "movies" | "series" | "settings";
let {
	tab,
	scroll = false,
	onback,
	children,
}: { tab?: Tab; scroll?: boolean; onback?: () => void; children: Snippet } = $props();

const TABS = [
	{ id: "search", label: "", name: i18n.nav_search(), href: "/search", icon: Search },
	{ id: "home", label: i18n.nav_home(), name: i18n.nav_home(), href: "/", icon: House },
	{
		id: "movies",
		label: i18n.nav_movies(),
		name: i18n.nav_movies(),
		href: "/library/movies",
		icon: Film,
	},
	{
		id: "series",
		label: i18n.nav_shows(),
		name: i18n.nav_shows(),
		href: "/library/series",
		icon: TvMinimal,
	},
	{ id: "settings", label: "", name: i18n.nav_settings(), href: "/settings", icon: Settings },
] as const;

const FOCUS_DELAY_MS = 120;
let stage: HTMLElement;
let scroller: HTMLElement;
let focusTimer: ReturnType<typeof setTimeout> | undefined;
let clock = $state("");

const tick = () =>
	(clock = new Date().toLocaleTimeString([], {
		hour: "2-digit",
		minute: "2-digit",
		hour12: false,
	}));

function back() {
	if (onback) return onback();
	if (document.activeElement?.closest("[data-tv-nav]")) return;
	focus(stage.querySelector<HTMLElement>("[data-tv-nav] [data-tv-active]"), stage);
}

function onKey(e: KeyboardEvent) {
	if (e.key === "Escape" || e.key === "Backspace" || e.key === "GoBack") {
		e.preventDefault();
		back();
	} else if (e.key.startsWith("Arrow")) {
		e.preventDefault();
		move(e.key, stage);
	} else if (e.key === "Enter" && !document.activeElement?.hasAttribute("data-tv")) {
		e.preventDefault();
		focusDefault(stage);
	}
}

afterNavigate(({ type }) => {
	if (type !== "popstate") forgetFocus();
	scroller.scrollTop = 0;
	clearTimeout(focusTimer);
	focusTimer = setTimeout(() => focusDefault(stage), FOCUS_DELAY_MS);
});

async function listen() {
	if (appPath().startsWith("/player")) return;
	// Recognized in the app's language, in the TV's own variant of it when the TV has one.
	const language =
		navigator.languages.find((l) => l.startsWith(getLocale())) ??
		(getLocale() === "fr" ? "fr-FR" : "en-US");
	const { text } = await hear(language).catch(() => ({ text: "" }));
	if (!text.trim()) return;
	search.q = text.trim();
	if (appPath() !== "/search") goto("/search");
}

onMount(() => {
	// The layout holds the first screen back until it knows the device is paired, so that
	// screen mounts after the navigation that brought it: afterNavigate never fires for it.
	focusTimer = setTimeout(() => focusDefault(stage), FOCUS_DELAY_MS);
	tick();
	const timer = setInterval(tick, 10_000);
	const listener = onPlayer("back", back);
	const voice = onPlayer("voiceKey", listen);
	return () => {
		clearTimeout(focusTimer);
		clearInterval(timer);
		listener.then((off) => off());
		voice.then((off) => off());
	};
});
</script>

<svelte:window onkeydown={onKey} />

<div class="stage" data-tv-screen="1" bind:this={stage}>
	<div class="scroller" class:scroll bind:this={scroller}>{@render children()}</div>
	{#if tab}
		<div class="nav" data-tv-nav="1">
			<div class="brand">
				<img src={staticUrl("streamline-mark.png")} alt="" width="44" height="44">
				<span class="name"><span>streamline</span><span class="player">player</span></span>
			</div>
			<div class="tabs" data-tv-row="1">
				{#each TABS as t}
					<button
						type="button"
						data-tv="1"
						data-tv-active={t.id === tab ? "1" : undefined}
						class="pill"
						class:current={t.id === tab}
						class:icon={!t.label}
						aria-label={t.name}
						aria-current={t.id === tab ? "page" : undefined}
						onclick={() => goto(t.href)}
					>
						<t.icon size={28} strokeWidth={1.75} />{t.label}
					</button>
				{/each}
			</div>
			<div class="clock">{clock}</div>
		</div>
	{/if}
</div>

<style>
.stage {
	position: fixed;
	left: 0;
	top: 0;
	width: 1920px;
	height: 1080px;
	zoom: var(--tv-zoom, 1);
	overflow: hidden;
	user-select: none;
	background: var(--bg-deep);
	color: var(--fg);
}

.scroller {
	position: absolute;
	inset: 0;
	overflow: hidden;
}

.scroller.scroll {
	overflow: hidden auto;
	scrollbar-width: none;
}

.nav {
	position: absolute;
	left: 0;
	top: 0;
	width: 1920px;
	height: 136px;
	z-index: 20;
	display: flex;
	align-items: center;
	gap: 14px;
	padding: 0 96px;
	background: linear-gradient(
		180deg,
		color-mix(in srgb, var(--bg-deep) 90%, transparent) 0%,
		color-mix(in srgb, var(--bg-deep) 55%, transparent) 55%,
		transparent 100%
	);
}

.brand {
	display: flex;
	align-items: center;
	gap: 16px;
	width: 340px;
}

.brand img {
	border-radius: 12px;
}

.name {
	display: flex;
	align-items: baseline;
	gap: 8px;
	font-size: 28px;
	font-weight: 600;
	letter-spacing: -0.02em;
	white-space: nowrap;
}

.player {
	font-weight: 400;
	color: var(--fg-subtle);
}

.tabs {
	flex: 1;
	display: flex;
	justify-content: center;
	gap: 10px;
}

.pill {
	height: 60px;
	display: inline-flex;
	align-items: center;
	gap: 14px;
	padding: 0 28px;
	border: 0;
	border-radius: 30px;
	background: transparent;
	color: var(--player-ink-muted);
	font-size: 25px;
	font-weight: 500;
	cursor: pointer;
	outline: none;
	transition:
		background var(--dur-fast),
		color var(--dur-fast),
		transform 150ms;
}

.pill.icon {
	gap: 0;
}

.pill.current {
	background: rgba(255, 255, 255, 0.1);
	color: var(--player-ink);
}

.pill:focus {
	background: var(--player-ink);
	color: #0b0b10;
	transform: scale(1.06);
}

.clock {
	width: 340px;
	text-align: right;
	font: 500 26px var(--font-mono);
	color: var(--fg-muted);
}
</style>
