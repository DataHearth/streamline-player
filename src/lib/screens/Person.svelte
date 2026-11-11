<script lang="ts">
import { monogram, seasonSpan } from "#lib/core/format.ts";
import type { Credit, Person } from "#lib/core/types.ts";
import { m as i18n } from "#lib/paraglide/messages.js";
import Art from "#lib/ui/Art.svelte";
import Icon from "#lib/ui/Icon.svelte";
import { goto } from "#lib/ui/nav.ts";
import NotFound from "#lib/ui/NotFound.svelte";
import PersonHeader from "#lib/ui/PersonHeader.svelte";
import PosterCard from "#lib/ui/PosterCard.svelte";
import { artwork, headshot, person as fetchPerson } from "#platform";
import { afterNavigate } from "$app/navigation";
import EmptyState from "#lib/ui/EmptyState.svelte";
import Button from "#lib/ui/Button.svelte";

const PAD_X = 56;
const TOP = 80;
const GLOW = 480;
const MIN_COL = 148;

let { id }: { id: string } = $props();

let p = $state.raw<Person | null>(null);
let missing = $state(false);
let failed = $state<string | null>(null);
let fromApp = false;
let loadId = 0;

afterNavigate(({ from }) => {
	fromApp = from !== null;
});

async function load(i: string) {
	const call = ++loadId;
	p = null;
	missing = false;
	failed = null;
	try {
		const found = await fetchPerson(i);
		if (call === loadId) p = found;
	} catch (e) {
		if (call !== loadId) return;
		if (String(e).includes('"not_found"')) missing = true;
		else failed = String(e);
	}
}

$effect(() => {
	load(id);
});

const back = () => (fromApp ? history.back() : goto("/"));

const credits = $derived([...(p?.credits ?? [])].sort((a, b) => b.year - a.year));
const movies = $derived(credits.filter((c) => c.kind === "movies"));
const series = $derived(credits.filter((c) => c.kind === "series"));
const characterOf = $derived(new Map(credits.map((c) => [`${c.kind}:${c.id}`, c.character])));
const summary = $derived.by(() => {
	const parts = [
		movies.length
			? (movies.length === 1 ? i18n.person_movies_one : i18n.person_movies_other)({
					count: movies.length,
				})
			: "",
		series.length
			? (series.length === 1 ? i18n.person_series_one : i18n.person_series_other)({
					count: series.length,
				})
			: "",
	].filter(Boolean);
	return i18n.person_on_your_server({
		list: parts.length === 2 ? i18n.person_and({ a: parts[0]!, b: parts[1]! }) : (parts[0] ?? ""),
	});
});
const mono = $derived(monogram(p?.name ?? "?"));
const known = $derived(
	(p?.known_for ?? []).map((k) => {
		const onServer = k.kind !== undefined && k.id !== undefined;
		const key = `${k.kind}:${k.id}`;
		return {
			title: k.title,
			year: k.year,
			character: onServer ? characterOf.get(key) : undefined,
			poster: onServer ? artwork(k.kind!, k.id!, "poster") : undefined,
			onopen: onServer ? () => goto(`/detail/${k.kind}/${k.id}`) : undefined,
		};
	}),
);
const backdrop = $derived(credits[0]);
let bgHidden = $state(false);
$effect(() => {
	backdrop;
	bgHidden = false;
});

const open = (c: Credit) =>
	goto(
		`/detail/${c.kind}/${c.id}${c.kind === "series" && c.seasons?.length ? `?s=${c.seasons[0]}` : ""}`,
	);
const subOf = (c: Credit) =>
	c.kind === "series"
		? seasonSpan(c.seasons ?? [], c.total_seasons ?? c.seasons?.length ?? 0)
		: String(c.year);
</script>

{#snippet section(name: string, list: Credit[])}
	<section class="credits">
		<div class="head">
			<h2>{name}</h2>
			<span class="n">{list.length}</span>
		</div>
		<div class="grid" style:--min="{MIN_COL}px">
			{#each list as c (`${c.kind}:${c.id}`)}
				<PosterCard
					title={c.title}
					character={c.character}
					sub={subOf(c)}
					accent={c.kind === "series"}
					src={artwork(c.kind, c.id, "poster")}
					onclick={() => open(c)}
				/>
			{/each}
		</div>
	</section>
{/snippet}

<div class="person">
	{#if missing}
		<NotFound
			title={i18n.person_not_found()}
			label={i18n.detail_back_home()}
			icon="search"
			onback={() => goto("/")}
		/>
	{:else if failed !== null}
		<div class="state">
			<EmptyState icon="film" title={i18n.home_error_title()} text={failed}>
				{#snippet action()}
					<Button variant="secondary" onclick={() => load(id)}>{i18n.halt_try_again()}</Button>
				{/snippet}
			</EmptyState>
		</div>
	{:else if p}
		<div class="glow" style:height="{GLOW}px">
			{#if backdrop && !bgHidden}
				<img
					src={artwork(backdrop.kind, backdrop.id, "backdrop")}
					alt=""
					onerror={() => (bgHidden = true)}
				>
			{/if}
			<div class="fade"></div>
		</div>
		<div class="body" style:padding="{TOP}px {PAD_X}px 64px">
			<div class="top">
				<button type="button" class="back" onclick={back}>
					<Icon name="arrow-left" size={14} stroke={2} />{i18n.common_back()}
				</button>
				<PersonHeader
					name={p.name}
					department={p.department === "Acting" ? i18n.person_acting() : p.department}
					{summary}
					photo={headshot(p.id)}
					initials={mono.initials}
					bg={mono.bg}
					fg={mono.fg}
					{known}
					knownTitle={i18n.person_known_for()}
					onServer={i18n.person_on_server()}
					notInLibrary={i18n.person_not_in_library()}
				/>
			</div>
			{#if series.length}
				{@render section(i18n.person_series_on_server(), series)}
			{/if}
			{#if movies.length}
				{@render section(i18n.person_movies_on_server(), movies)}
			{/if}
		</div>
	{/if}
</div>

<style>
.person {
	position: relative;
	min-height: 100%;
	overflow: hidden;
	background: var(--bg-deep);
	color: var(--fg);
}

.state {
	display: grid;
	place-items: center;
	min-height: 100vh;
}

.glow {
	position: absolute;
	left: 0;
	right: 0;
	top: 0;
	overflow: hidden;
	pointer-events: none;
}

.glow img {
	position: absolute;
	left: -60px;
	top: -60px;
	width: calc(100% + 120px);
	height: calc(100% + 120px);
	object-fit: cover;
	filter: blur(48px) saturate(1.3);
	opacity: 0.38;
}

.fade {
	position: absolute;
	inset: 0;
	background: linear-gradient(
		180deg,
		color-mix(in srgb, var(--bg-deep) 35%, transparent) 0%,
		color-mix(in srgb, var(--bg-deep) 78%, transparent) 55%,
		var(--bg-deep) 100%
	);
}

.body {
	position: relative;
	box-sizing: border-box;
	display: flex;
	flex-direction: column;
	gap: 40px;
}

.top {
	display: flex;
	flex-direction: column;
	gap: 26px;
}

.back {
	align-self: flex-start;
	display: flex;
	align-items: center;
	gap: 6px;
	height: 34px;
	padding: 0 13px 0 10px;
	border: 1px solid var(--border-strong);
	border-radius: var(--radius-full);
	background: rgba(0, 0, 0, 0.4);
	color: var(--fg-muted);
	font-size: 12.5px;
	font-weight: 500;
	cursor: pointer;
}

.back:hover {
	background: rgba(0, 0, 0, 0.6);
	color: var(--fg);
}

.credits {
	display: flex;
	flex-direction: column;
	gap: 14px;
}

.head {
	display: flex;
	align-items: baseline;
	gap: 12px;
}

h2 {
	margin: 0;
	font-size: 18px;
	font-weight: 600;
}

.n {
	font: 400 12px var(--font-mono);
	color: var(--fg-subtle);
}

.grid {
	display: grid;
	grid-template-columns: repeat(auto-fill, minmax(var(--min), 1fr));
	gap: 26px 18px;
}
</style>
