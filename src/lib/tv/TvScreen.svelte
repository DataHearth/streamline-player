<script lang="ts">
	import { afterNavigate, goto } from "$app/navigation";
	import { addPluginListener } from "@tauri-apps/api/core";
	import { Film, House, TvMinimal } from "@lucide/svelte";
	import { onMount, type Snippet } from "svelte";
	import { focus, focusDefault, forgetFocus, move } from "./focus";

	type Tab = "home" | "movies" | "series";
	let { tab, scroll = false, onback, children }: { tab?: Tab; scroll?: boolean; onback?: () => void; children: Snippet } = $props();

	const TABS = [
		{ id: "home", label: "Home", href: "/", icon: House },
		{ id: "movies", label: "Movies", href: "/library/movies", icon: Film },
		{ id: "series", label: "Shows", href: "/library/series", icon: TvMinimal },
	] as const;

	const FOCUS_DELAY_MS = 120;
	let stage: HTMLElement;
	let scroller: HTMLElement;
	let focusTimer: ReturnType<typeof setTimeout> | undefined;
	let clock = $state("");

	const tick = () => (clock = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false }));

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

	onMount(() => {
		tick();
		const timer = setInterval(tick, 10_000);
		const listener = addPluginListener("player", "back", back);
		return () => {
			clearTimeout(focusTimer);
			clearInterval(timer);
			listener.then((h) => h.unregister());
		};
	});
</script>

<svelte:window onkeydown={onKey} />

<div class="stage" bind:this={stage}>
	<div class="scroller" class:scroll bind:this={scroller}>{@render children()}</div>
	{#if tab}
		<div class="nav" data-tv-nav="1">
			<div class="brand">
				<img src="/streamline-mark.png" alt="" width="44" height="44" />
				<span class="name"><span>streamline</span><span class="player">player</span></span>
			</div>
			<div class="tabs" data-tv-row="1">
				{#each TABS as t}
					<button data-tv="1" data-tv-active={t.id === tab ? "1" : undefined} class="pill" class:current={t.id === tab} aria-current={t.id === tab ? "page" : undefined} onclick={() => goto(t.href)}>
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
		background: linear-gradient(180deg, color-mix(in srgb, var(--bg-deep) 90%, transparent) 0%, color-mix(in srgb, var(--bg-deep) 55%, transparent) 55%, transparent 100%);
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
		transition: background var(--dur-fast), color var(--dur-fast), transform 150ms;
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
