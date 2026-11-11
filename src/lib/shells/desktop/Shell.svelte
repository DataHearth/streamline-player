<script lang="ts">
import { goto } from "#lib/ui/nav.ts";
import { page } from "$app/state";
import { account, clearMe, initialsOf, setMe } from "#lib/core/me.svelte.ts";
import { activeToast, toast } from "#lib/core/toast.svelte.ts";
import { m as i18n } from "#lib/paraglide/messages.js";
import { IS_WEB, TARGET } from "#lib/platform/target.ts";
import { keyAction } from "#lib/shells/desktop/shortcuts.ts";
import WindowButtons from "#lib/shells/desktop/WindowButtons.svelte";
import AppBar from "#lib/ui/AppBar.svelte";
import Dialog from "#lib/ui/Dialog.svelte";
import DesktopToast from "#lib/ui/DesktopToast.svelte";
import Kbd from "#lib/ui/Kbd.svelte";
import Menu from "#lib/ui/Menu.svelte";
import ShortcutList from "#lib/ui/ShortcutList.svelte";
import { presence } from "#lib/ui/presence.svelte.ts";
import { provideScroll } from "#lib/ui/scroll.ts";
import { me, myPicture, signOut } from "#platform";
import type { Snippet } from "svelte";
import { onMount } from "svelte";

const MIN_WIDTH = 900;
const STAGE_WIDTH = 1920;
const G_WINDOW_MS = 900;
const FULLSCREEN_HINT_MS = 2600;
const AVATAR_COLOR = "var(--kind-album)";

let { web, children }: { web: boolean; children: Snippet } = $props();

const mac = TARGET === "darwin" || (IS_WEB && /Mac/.test(navigator.platform));
let width = $state(window.innerWidth);
let scrolledDown = $state(false);
let menuOpen = $state(false);
let helpOpen = $state(false);
let fullscreenHint = $state(false);
let gAt = 0;
let content = $state<HTMLElement>();
provideScroll(() => content);

const menu = presence(() => menuOpen, 140);
const help = presence(() => helpOpen, 240);

const route = $derived(page.route.id ?? "");
const kind = $derived(page.params.kind);
const onboarding = $derived(route === "/welcome" || route.startsWith("/signin"));
const inPlayer = $derived(route.startsWith("/player"));
const showBar = $derived(!onboarding && !inPlayer);
const narrow = $derived(web && width < MIN_WIDTH);
const solidBar = $derived(scrolledDown || !(route === "/" || route.startsWith("/detail")));
const padLeft = $derived(!web && mac ? 92 : 28);
const padRight = $derived(
	!web && TARGET === "windows" ? 150 : !web && TARGET === "linux" ? 128 : 24,
);
const toastTop = $derived((showBar ? 64 : 0) + 16);

const name = $derived(account.me?.name ?? "");
const initials = $derived(initialsOf(name));
const picture = $derived(account.me?.picture ? myPicture(account.version) : undefined);
const modifier = mac ? "⌘" : "Ctrl";

const tabs = $derived([
	{ id: "home", label: i18n.nav_home(), on: route === "/" },
	{ id: "movies", label: i18n.nav_movies(), on: route === "/library/[kind]" && kind !== "series" },
	{ id: "shows", label: i18n.nav_shows(), on: route === "/library/[kind]" && kind === "series" },
]);
const PATHS: Record<string, string> = {
	home: "/",
	movies: "/library/movies",
	shows: "/library/series",
	search: "/search",
	settings: "/settings",
};

const kb = (keys: string[], label: string) => ({ keys, label });
const groups = $derived([
	{
		title: i18n.shell_keys_anywhere(),
		items: [
			kb([modifier, "K"], i18n.nav_search()),
			kb(["G", "H"], i18n.shell_key_home()),
			kb(["G", "M"], i18n.shell_key_movies()),
			kb(["G", "S"], i18n.shell_key_shows()),
			kb(["G", ","], i18n.nav_settings()),
			kb(["?"], i18n.shell_key_help()),
			kb(["Esc"], i18n.shell_key_close()),
		],
	},
	{
		title: i18n.shell_keys_player(),
		items: [
			kb(["Space"], i18n.shell_key_play()),
			kb(["←", "→"], i18n.shell_key_seek()),
			kb(["↑", "↓"], i18n.shell_key_volume()),
			kb(["M"], i18n.shell_key_mute()),
			kb(["C"], i18n.shell_key_subtitles()),
			kb(["F"], i18n.shell_key_fullscreen()),
			kb(["I"], i18n.shell_key_stats()),
			kb(["N"], i18n.shell_key_next()),
			kb(["P"], i18n.shell_key_previous()),
		],
	},
]);

const go = (id: string) => goto(PATHS[id] ?? "/");

function search() {
	const field = document.querySelector<HTMLInputElement>("[data-search-field]");
	if (!field) return go("search");
	field.focus();
	field.select();
}

async function leave() {
	menuOpen = false;
	await signOut();
	toast(web ? i18n.shell_signed_out() : i18n.shell_signed_out_key_removed());
	await (web ? goto("/signin", { replace: true }) : goto("/welcome", { replace: true }));
}

const items = $derived([
	{
		icon: "settings" as const,
		label: i18n.nav_settings(),
		keys: ["G", ","],
		onclick: () => {
			menuOpen = false;
			go("settings");
		},
	},
	{
		icon: "keyboard" as const,
		label: i18n.shell_keyboard_shortcuts(),
		keys: ["?"],
		onclick: () => {
			menuOpen = false;
			helpOpen = true;
		},
	},
	{ icon: "log-out" as const, label: i18n.shell_sign_out(), separator: true, onclick: leave },
]);

function onKey(e: KeyboardEvent) {
	const pendingG = gAt > 0 && Date.now() - gAt < G_WINDOW_MS;
	gAt = 0;
	const act = keyAction(
		{ key: e.key, ctrlKey: e.ctrlKey, metaKey: e.metaKey, target: e.target as Element | null },
		mac,
		pendingG,
	);
	if (act === "g") {
		gAt = Date.now();
		return;
	}
	if ((helpOpen || menuOpen) && (act === "close" || (helpOpen && act === "help"))) {
		e.preventDefault();
		e.stopPropagation();
		helpOpen = false;
		menuOpen = false;
		return;
	}
	if (!act || act === "close") return;
	e.preventDefault();
	if (act === "help") {
		menuOpen = false;
		helpOpen = true;
	} else if (act === "search") search();
	else go(act);
}

$effect(() => {
	if (onboarding) clearMe();
});

$effect(() => {
	if (onboarding || account.me) return;
	me()
		.then((v) => setMe(v))
		.catch(() => {});
});

onMount(() => {
	const zoom = () =>
		document.documentElement.style.setProperty(
			"--tv-zoom",
			String(window.innerWidth / STAGE_WIDTH),
		);
	zoom();
	let hint: ReturnType<typeof setTimeout>;
	const onFullscreen = () => {
		clearTimeout(hint);
		fullscreenHint = web && !!document.fullscreenElement;
		if (fullscreenHint) hint = setTimeout(() => (fullscreenHint = false), FULLSCREEN_HINT_MS);
	};
	window.addEventListener("resize", zoom);
	window.addEventListener("keydown", onKey, true);
	document.addEventListener("fullscreenchange", onFullscreen);
	return () => {
		window.removeEventListener("resize", zoom);
		window.removeEventListener("keydown", onKey, true);
		document.removeEventListener("fullscreenchange", onFullscreen);
		clearTimeout(hint);
	};
});

const toastNow = $derived(activeToast());
</script>

<svelte:window bind:innerWidth={width} />

<div class="shell">
	{#if narrow}
		<p class="narrow">{i18n.shell_narrow()}</p>
	{:else}
		<div
			class="content"
			bind:this={content}
			onscroll={(e) => (scrolledDown = e.currentTarget.scrollTop > 24)}
		>
			{@render children()}
		</div>

		{#if showBar}
			<AppBar
				{tabs}
				{padLeft}
				{padRight}
				scrolled={solidBar}
				searchLabel={i18n.nav_search()}
				searchKbd={mac ? "⌘K" : "Ctrl K"}
				activityLabel={i18n.shell_activity()}
				downloadsLabel={i18n.shell_downloads()}
				showDownloads={!web}
				account={{ name, email: account.me?.email ?? "", initials, color: AVATAR_COLOR, picture }}
				accountLabel={i18n.shell_account_menu()}
				{menuOpen}
				onhome={() => go("home")}
				ontab={go}
				onsearch={search}
				onactivity={() => {}}
				ondownloads={() => {}}
				onmenu={() => (menuOpen = !menuOpen)}
			/>
		{/if}

		{#if showBar && menu.mounted}
			<button
				type="button"
				class="scrim"
				tabindex="-1"
				aria-label={i18n.shell_close()}
				style:pointer-events={menuOpen ? "auto" : "none"}
				onclick={() => (menuOpen = false)}
			></button>
			<div
				class="menu"
				class:shown={menu.shown}
				class:closing={!menuOpen}
				style:right="{Math.max(12, padRight - 4)}px"
				style:pointer-events={menuOpen ? "auto" : "none"}
			>
				<Menu
					{name}
					email={account.me?.email}
					{initials}
					color={AVATAR_COLOR}
					{picture}
					{items}
					label={i18n.shell_account_menu()}
				/>
			</div>
		{/if}

		{#if help.mounted}
			<Dialog
				title={i18n.shell_keyboard_shortcuts()}
				closeLabel={i18n.shell_close()}
				shown={help.shown}
				onclose={() => (helpOpen = false)}
			>
				<ShortcutList {groups} />
			</Dialog>
		{/if}

		{#if toastNow}
			<div class="toasts" style:top="{toastTop}px">
				{#key toastNow.title}
					<DesktopToast
						title={toastNow.title}
						text={toastNow.text}
						icon={toastNow.icon}
						warning={toastNow.warning}
						leaving={toastNow.leaving}
					/>
				{/key}
			</div>
		{/if}

		{#if !web}
			{#if !showBar}
				<div class="drag" data-tauri-drag-region style:right="{padRight}px"></div>
			{/if}
			<WindowButtons hidden={inPlayer} />
		{/if}
	{/if}

	{#if fullscreenHint}
		<div class="hint">
			<div class="hint-box">
				{i18n.shell_press()}<Kbd>Esc</Kbd>{i18n.shell_exit_fullscreen()}
			</div>
		</div>
	{/if}
</div>

<style>
.shell {
	position: fixed;
	inset: 0;
	overflow: hidden;
	background: var(--bg-deep);
	color: var(--fg);
}

.content {
	position: absolute;
	inset: 0;
	overflow-y: auto;
}

.narrow {
	position: absolute;
	inset: 0;
	display: grid;
	place-items: center;
	margin: 0;
	padding: 0 32px;
	text-align: center;
	font-size: 14px;
	color: var(--fg-muted);
}

.drag {
	position: absolute;
	left: 0;
	top: 0;
	height: 36px;
	z-index: 10;
}

.scrim {
	position: absolute;
	inset: 0;
	z-index: 80;
	border: 0;
	padding: 0;
	background: transparent;
}

.menu {
	position: absolute;
	top: 60px;
	z-index: 81;
	transform-origin: top right;
	opacity: 0;
	transform: translateY(10px);
	transition:
		opacity 130ms var(--ease-in),
		transform 130ms var(--ease-in);
}

.menu.closing {
	transform: translateY(6px);
}

.menu.shown {
	opacity: 1;
	transform: translateY(0);
	transition:
		opacity 180ms var(--ease),
		transform 180ms var(--ease);
}

.toasts {
	position: absolute;
	right: 24px;
	z-index: 60;
	pointer-events: none;
}

.hint {
	position: absolute;
	left: 0;
	right: 0;
	top: 22px;
	z-index: 90;
	display: flex;
	justify-content: center;
	pointer-events: none;
}

.hint-box {
	display: flex;
	align-items: center;
	gap: 8px;
	padding: 10px 16px;
	border-radius: var(--radius-md);
	background: color-mix(in srgb, var(--bg-elevated) 92%, transparent);
	border: 1px solid var(--border);
	box-shadow: var(--shadow-3);
	font-size: 13px;
	color: var(--fg);
	white-space: nowrap;
	animation: sl-rise 220ms var(--ease);
}
</style>
