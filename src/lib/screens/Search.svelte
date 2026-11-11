<script lang="ts">
import { lib, loadLibrary } from "#lib/core/library.svelte.ts";
import { runtime } from "#lib/core/format.ts";
import { getLocale } from "#lib/paraglide/runtime.js";
import {
	genreTiles,
	type Hit,
	loadRecent,
	rememberSearch,
	saveRecent,
	searchTitles,
	tagged,
} from "#lib/core/search-view.ts";
import { m as i18n } from "#lib/paraglide/messages.js";
import Button from "#lib/ui/Button.svelte";
import EmptyState from "#lib/ui/EmptyState.svelte";
import Icon from "#lib/ui/Icon.svelte";
import { goto } from "#lib/ui/nav.ts";
import PosterCard from "#lib/ui/PosterCard.svelte";
import SearchField from "#lib/ui/SearchField.svelte";
import { artwork, home, library, title, watchState } from "#platform";
import { afterNavigate } from "$app/navigation";
import { page } from "$app/state";
import { onMount } from "svelte";

const PAD_X = 56;
const TOP = 92;
const TYPE_DELAY_MS = 150;
const port = { library, watchState, home, title };

const urlQ = $derived(page.url.searchParams.get("q") ?? "");
let text = $state(page.url.searchParams.get("q") ?? "");
let input = $state<HTMLInputElement>();
let recent = $state(loadRecent());
let pushed = text;
let timer: ReturnType<typeof setTimeout> | undefined;
let fromApp = false;

afterNavigate(({ from, to }) => {
	if (from && from.url.pathname === to?.url.pathname) return;
	fromApp = from !== null;
});

$effect(() => {
	const external = urlQ;
	if (external !== pushed) {
		pushed = external;
		text = external;
	}
});

function push(replace = true) {
	clearTimeout(timer);
	pushed = text;
	goto(`/search${text ? `?q=${encodeURIComponent(text)}` : ""}`, { replace, reset: false });
}

const ask = (q: string, replace = true) => {
	text = q;
	push(replace);
	input?.focus();
};

function onkeydown(e: KeyboardEvent) {
	if (e.key !== "Escape") return;
	e.preventDefault();
	if (text) return ask("");
	if (fromApp) history.back();
	else goto("/");
}

const all = $derived(tagged(lib.movies, lib.shows));
const q = $derived(urlQ.trim());
const hits = $derived(searchTitles(all, q));
const movies = $derived(hits.filter((t) => t.kind === "movies"));
const shows = $derived(hits.filter((t) => t.kind === "series"));
const groups = $derived(
	[
		{ label: i18n.nav_movies(), items: movies },
		{ label: i18n.nav_shows(), items: shows },
	].filter((g) => g.items.length),
);
const tiles = $derived(genreTiles(all));
const empty = $derived(lib.loaded && all.length === 0);

const rating = (n: number) => n.toLocaleString(getLocale());
const years = (t: Hit) => t.first_aired ?? String(t.year);
const seasons = (t: Hit) => {
	const n = t.total_seasons ?? t.seasons?.length ?? 0;
	return n === 1 ? i18n.common_seasons_one({ count: n }) : i18n.common_seasons_other({ count: n });
};
const sub = (t: Hit) =>
	t.kind === "series"
		? [years(t), seasons(t)].join(" · ")
		: [t.year, t.runtime ? runtime(t.runtime) : ""].filter(Boolean).join(" · ");
const meta = (t: Hit) =>
	(t.kind === "series"
		? [years(t), t.network, `★ ${rating(t.rating)}`]
		: [
				t.year,
				t.runtime ? runtime(t.runtime) : "",
				t.file_summary?.resolution,
				`★ ${rating(t.rating)}`,
			]
	)
		.filter(Boolean)
		.join(" · ");
const kindLabel = (t: Hit) =>
	t.kind === "series" ? i18n.search_kind_show() : i18n.search_kind_movie();

function open(t: Hit) {
	recent = rememberSearch(recent, q);
	saveRecent(recent);
	goto(`/detail/${t.kind}/${t.id}`);
}

onMount(() => {
	loadLibrary(port, lib.loaded);
	input?.focus();
	return () => clearTimeout(timer);
});
</script>

<div class="search" style:padding="{TOP}px {PAD_X}px 56px">
	<SearchField
		bind:value={text}
		bind:input
		placeholder={i18n.search_field_placeholder()}
		clearLabel={i18n.search_clear()}
		oninput={() => {
			clearTimeout(timer);
			timer = setTimeout(push, TYPE_DELAY_MS);
		}}
		onclear={() => ask("")}
		{onkeydown}
	/>

	{#if lib.error !== null}
		<EmptyState icon="film" title={i18n.home_error_title()} text={lib.error}>
			{#snippet action()}
				<Button variant="secondary" onclick={() => loadLibrary(port, true)}>
					{i18n.halt_try_again()}
				</Button>
			{/snippet}
		</EmptyState>
	{:else if empty}
		<EmptyState icon="film" title={i18n.home_empty_title()} text={i18n.home_empty_text()} />
	{:else if lib.loaded && !q}
		<div class="idle" class:solo={!recent.length}>
			{#if recent.length}
				<div class="recent">
					<h2 class="eyebrow">{i18n.search_recent()}</h2>
					{#each recent as r (r)}
						<button type="button" onclick={() => ask(r, false)}>
							<Icon name="history" size={16} stroke={1.75} />{r}
						</button>
					{/each}
				</div>
			{/if}
			<div class="genres">
				<h2 class="eyebrow">{i18n.search_browse_genre()}</h2>
				<div class="tiles">
					{#each tiles as g (g.name)}
						<button type="button" class="genre-tile" onclick={() => ask(g.name, false)}>
							<img
								src={artwork(g.cover.kind, g.cover.id, "poster")}
								alt=""
								loading="lazy"
								onerror={(e) => e.currentTarget.remove()}
							>
							<span class="label">
								<span class="name">{g.name}</span>
								<span class="n"
									>{(g.count === 1 ? i18n.search_titles_one : i18n.search_titles_other)({
										count: g.count,
									})}</span
								>
							</span>
						</button>
					{/each}
				</div>
			</div>
		</div>
	{:else if lib.loaded && hits.length}
		{@const top = hits[0]!}
		<div class="results">
			<div class="found">
				{(hits.length === 1 ? i18n.search_in_library_one : i18n.search_in_library_other)({
					count: hits.length,
				})}
			</div>
			<button type="button" class="top" onclick={() => open(top)}>
				<img
					src={artwork(top.kind, top.id, "poster")}
					alt=""
					onerror={(e) => e.currentTarget.remove()}
				>
				<span class="copy">
					<span class="eyebrow accent">{i18n.search_top_result({ kind: kindLabel(top) })}</span>
					<span class="name">{top.title}</span>
					<span class="meta">{meta(top)}</span>
					<span class="overview">{top.overview}</span>
				</span>
			</button>
			{#each groups as g (g.label)}
				<section>
					<h2>{g.label} <span class="n">{g.items.length}</span></h2>
					<div class="grid">
						{#each g.items as t (`${t.kind}:${t.id}`)}
							<PosterCard
								title={t.title}
								sub={sub(t)}
								src={artwork(t.kind, t.id, "poster")}
								onclick={() => open(t)}
							/>
						{/each}
					</div>
				</section>
			{/each}
		</div>
	{:else if lib.loaded}
		<EmptyState
			icon="search"
			title={i18n.search_nothing({ query: q })}
			text={i18n.search_request_hint()}
		/>
	{/if}
</div>

<style>
.search {
	min-height: 100%;
	box-sizing: border-box;
	display: flex;
	flex-direction: column;
	gap: 28px;
	background: var(--bg-deep);
	color: var(--fg);
}

.eyebrow {
	margin: 0;
	font: 500 10.5px var(--font-mono);
	letter-spacing: 0.14em;
	text-transform: uppercase;
	color: var(--fg-faint);
}

.eyebrow.accent {
	color: var(--accent-text);
}

.idle {
	display: grid;
	grid-template-columns: minmax(0, 1fr) minmax(0, 2fr);
	gap: 40px;
}

.idle.solo {
	grid-template-columns: minmax(0, 1fr);
}

.recent,
.genres {
	display: flex;
	flex-direction: column;
	gap: 12px;
}

.recent button {
	display: flex;
	align-items: center;
	gap: 12px;
	height: 40px;
	padding: 0 10px;
	margin: 0 -10px;
	border: 0;
	border-radius: var(--radius-md);
	background: transparent;
	color: var(--fg-muted);
	font-size: 14.5px;
	text-align: left;
	cursor: pointer;
}

.recent button:hover {
	background: var(--surface);
	color: var(--fg);
}

.recent button :global(.sl-ico) {
	color: var(--fg-faint);
}

.tiles {
	display: grid;
	grid-template-columns: repeat(3, minmax(0, 1fr));
	gap: 10px;
}

.genre-tile {
	position: relative;
	height: 84px;
	display: flex;
	align-items: flex-end;
	padding: 12px 14px;
	border: 0;
	border-radius: 12px;
	overflow: hidden;
	background: var(--bg-elevated);
	color: var(--fg);
	text-align: left;
	cursor: pointer;
	box-shadow: inset 0 0 0 1px var(--border);
}

.genre-tile:hover {
	box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 50%, transparent);
}

.genre-tile img {
	position: absolute;
	right: -6px;
	top: 10px;
	width: 58px;
	aspect-ratio: 2 / 3;
	border-radius: var(--radius-sm);
	object-fit: cover;
	transform: rotate(8deg);
	opacity: 0.85;
	box-shadow: 0 8px 20px rgba(0, 0, 0, 0.5);
}

.label {
	position: relative;
	display: flex;
	flex-direction: column;
}

.label .name {
	font-size: 15px;
	font-weight: 600;
}

.n {
	font: 400 11px var(--font-mono);
	color: var(--fg-subtle);
}

.results {
	display: flex;
	flex-direction: column;
	gap: 28px;
}

.found {
	font: 400 12.5px var(--font-mono);
	color: var(--fg-subtle);
}

.top {
	display: grid;
	grid-template-columns: auto minmax(0, 1fr);
	gap: 22px;
	align-items: center;
	max-width: 760px;
	padding: 14px;
	border: 1px solid var(--border);
	border-radius: var(--radius-lg);
	background: var(--bg-elevated);
	color: var(--fg);
	text-align: left;
	cursor: pointer;
}

.top:hover {
	border-color: color-mix(in srgb, var(--accent) 45%, transparent);
}

.top img {
	width: 110px;
	aspect-ratio: 2 / 3;
	border-radius: var(--radius-md);
	object-fit: cover;
	background: var(--bg-card);
}

.copy {
	display: flex;
	flex-direction: column;
	gap: 8px;
	min-width: 0;
}

.copy .name {
	font-size: 24px;
	font-weight: 700;
	letter-spacing: -0.02em;
}

.meta {
	font: 400 12px var(--font-mono);
	color: var(--fg-subtle);
}

.overview {
	font-size: 13.5px;
	line-height: 1.5;
	color: var(--fg-muted);
	display: -webkit-box;
	-webkit-line-clamp: 2;
	line-clamp: 2;
	-webkit-box-orient: vertical;
	overflow: hidden;
}

section {
	display: flex;
	flex-direction: column;
	gap: 14px;
}

h2 {
	margin: 0;
	font-size: 17px;
	font-weight: 600;
}

section h2 .n {
	font-size: 12px;
}

.grid {
	display: grid;
	grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
	gap: 22px 16px;
}
</style>
