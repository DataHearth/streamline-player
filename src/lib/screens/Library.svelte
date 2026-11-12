<script lang="ts">
import { isNew, nextEpisode, playUrl, watchedTitle } from "#lib/core/browse.ts";
import { addedText, badges, sizeText } from "#lib/core/format.ts";
import { lib, loadLibrary, loadSeasons, applyWatch } from "#lib/core/library.svelte.ts";
import {
	FLAGS,
	genres,
	progressOf,
	SORTS,
	type Sort,
	toggleFlag,
	type Flag,
	type Layout,
	type View,
	viewFromQuery,
	viewToQuery,
	visible,
} from "#lib/core/library-view.ts";
import {
	audioLanguages,
	type Choice,
	defaultChoice,
	loadChoices,
	saveChoices,
	subtitleLanguages,
} from "#lib/core/playback/choice.ts";
import { languageLabel } from "#lib/core/playback/names.ts";
import type { Tracks } from "#lib/core/playback/port.ts";
import { isAnime, language } from "#lib/core/playback/rules.ts";
import { myLanguage, prefs } from "#lib/core/prefs.svelte.ts";
import { toast } from "#lib/core/toast.svelte.ts";
import type { Kind, Title } from "#lib/core/types.ts";
import { m as i18n } from "#lib/paraglide/messages.js";
import Button from "#lib/ui/Button.svelte";
import Chip from "#lib/ui/Chip.svelte";
import { dismiss } from "#lib/ui/dismiss.ts";
import EmptyState from "#lib/ui/EmptyState.svelte";
import FilterMenu from "#lib/ui/FilterMenu.svelte";
import LanguagePopover from "#lib/ui/LanguagePopover.svelte";
import Icon from "#lib/ui/Icon.svelte";
import { goto } from "#lib/ui/nav.ts";
import PosterCard from "#lib/ui/PosterCard.svelte";
import { presence } from "#lib/ui/presence.svelte.ts";
import SegmentedControl from "#lib/ui/SegmentedControl.svelte";
import SortMenu from "#lib/ui/SortMenu.svelte";
import TitleRow from "#lib/ui/TitleRow.svelte";
import { artwork, home, library, playbackPort, setWatched, title, watchState } from "#platform";
import { page } from "$app/state";
import { onMount, untrack } from "svelte";

const PAD_X = 56;
const TOP = 92;
const MIN_COL = 196;
const ALL = "__all";
const LIST_COLS = "32px minmax(0,2.2fr) 74px 58px 104px minmax(0,1.4fr) 76px 84px";
const port = { library, watchState, home, title };

let { kind }: { kind: Kind } = $props();

const now = new Date();
const view = $derived(viewFromQuery(new URLSearchParams(page.url.search)));
const base = $derived(kind === "movies" ? lib.movies : lib.shows);
const items = $derived(visible(base, kind, lib.watch, view));
const genreList = $derived(genres(base, kind, lib.watch, view.flags));
const flaggedTotal = $derived(visible(base, kind, lib.watch, { ...view, genre: null }).length);

let genreMenu = $state(false);
const genrePanel = presence(() => genreMenu, 170);

const flagLabel: Record<Flag, string> = {
	unwatched: i18n.library_unwatched(),
	progress: i18n.library_in_progress(),
	uhd: "4K",
	hdr: "HDR",
};
const sortLabel = (s: Sort) =>
	({
		added: i18n.library_sort_added(),
		title: i18n.library_sort_title(),
		year: i18n.library_sort_year(),
		rating: i18n.library_sort_rating(),
		size: i18n.library_sort_size(),
	})[s];

function set(patch: Partial<View>) {
	const q = viewToQuery({ ...view, ...patch });
	goto(`/library/${kind}${q ? `?${q}` : ""}`, { replace: true, reset: false });
}

const years = (t: Title) => t.first_aired ?? String(t.year);
const sub = (t: Title) => {
	if (kind === "series")
		return [
			years(t),
			i18n.library_seasons_short({ count: t.total_seasons ?? t.seasons?.length ?? 0 }),
		].join(" · ");
	const q = t.file_summary;
	const tech = prefs.techDetails
		? [q?.resolution, q?.hdr].filter(Boolean).join(" ")
		: badges(q).slice(0, 2).join(" ");
	return [t.year, tech].filter(Boolean).join(" · ");
};

function episodeFor(t: Title) {
	const going = lib.watch.find((w) => w.series_id === t.id && !w.watched && w.position_seconds > 0);
	const files = (t.seasons ?? []).flatMap((s) =>
		s.episodes.filter((e) => e.has_file).map((e) => ({ ...e, season: s.number })),
	);
	return (
		(going && files.find((e) => e.id === going.episode_id)) ||
		nextEpisode(t, lib.watch) ||
		(going?.episode_id ? { id: going.episode_id } : undefined)
	);
}

function play(t: Title) {
	const resuming = progressOf(t, kind, lib.watch) > 0;
	if (kind === "movies") return goto(playUrl("movies", t.id, undefined, resuming));
	return goto(playUrl("series", t.id, episodeFor(t)?.id, resuming));
}

function playLabel(t: Title) {
	if (kind === "movies")
		return progressOf(t, kind, lib.watch) > 0
			? i18n.library_resume_title({ title: t.title })
			: i18n.library_play_title({ title: t.title });
	const ep = episodeFor(t);
	return ep && "number" in ep
		? i18n.library_play_episode({ title: t.title, season: ep.season, episode: ep.number })
		: i18n.library_play_title({ title: t.title });
}

const POPOVER_W = 280;
const POPOVER_H = 420;
const BAR = 64;
let box = $state<HTMLElement>();
let choices = $state(loadChoices());
type Open = { t: Title; tracks: Tracks | null; draft: Choice; x: number; y: number; up: boolean };
let lang = $state<Open | null>(null);
const popover = presence(() => lang !== null, 190);
let lastLang = $state.raw<Open | null>(null);
$effect(() => {
	if (lang) lastLang = lang;
});
const shownLang = $derived(lang ?? lastLang);

const languageNamed = (name: string, from: { language: string }[]) => {
	const hit = from.find((x) => language(x.language) === name);
	return hit ? languageLabel(hit.language) : name;
};
const subsShort = (c: Choice) =>
	c.subs === "off"
		? i18n.detail_subs_none()
		: c.subs === "forced"
			? i18n.detail_subs_forced()
			: i18n.detail_subs_language({ language: c.subs });
const langLabel = (t: Title) => {
	const own = choices[`movies:${t.id}`];
	return own
		? i18n.detail_lang_label({ audio: own.audio, subs: subsShort(own) })
		: i18n.detail_lang_title();
};

async function openLang(t: Title, el: HTMLElement) {
	if (lang?.t.id === t.id) {
		lang = null;
		return;
	}
	let tracks: Tracks | null = null;
	try {
		const media = (await title(kind, t.id)).media_files?.[0]?.id;
		if (media) tracks = await playbackPort.mediaTracks(media);
	} catch {
		// The popover then says the tracks are not ready.
	}
	if (!box || !el.isConnected) return;
	const draft =
		choices[`movies:${t.id}`] ??
		(tracks
			? defaultChoice(tracks.audio, {
					audioLang: prefs.audioLang,
					anime: isAnime(t.genres ?? []),
					animeRule: prefs.animeRule,
					subsMode: prefs.subsMode,
					myLanguage: myLanguage(),
				})
			: { audio: "", subs: "forced" });
	const b = box.getBoundingClientRect();
	const r = el.getBoundingClientRect();
	const host = el.closest(".content")?.getBoundingClientRect();
	const bottom = host?.bottom ?? window.innerHeight;
	const top = (host?.top ?? 0) + BAR + 8;
	const up = r.bottom + POPOVER_H > bottom && r.top - POPOVER_H >= top;
	let y = up ? r.top - b.top - 6 : r.bottom - b.top + 6;
	// No room below nor above: slide it up beside the card, clear of the app bar.
	if (!up && r.bottom + POPOVER_H > bottom)
		y = Math.max(top - b.top, Math.min(y, bottom - b.top - 8 - POPOVER_H));
	lang = {
		t,
		tracks,
		draft: { ...draft },
		x: Math.max(12, Math.min(box.offsetWidth - POPOVER_W - 12, r.right - b.left - POPOVER_W + 8)),
		y,
		up,
	};
}

function applyLang() {
	if (!lang) return;
	choices = { ...choices, [`movies:${lang.t.id}`]: lang.draft };
	saveChoices(choices);
	lang = null;
}

$effect(() => {
	if (!lang) return;
	const onPress = (e: PointerEvent) => {
		const el = e.target as Element | null;
		if (el?.closest('[role="dialog"]') || el?.closest("[data-lang-trigger]")) return;
		lang = null;
	};
	const onKey = (e: KeyboardEvent) => {
		if (e.key !== "Escape") return;
		e.stopPropagation();
		lang = null;
	};
	document.addEventListener("pointerdown", onPress, true);
	window.addEventListener("keydown", onKey, true);
	return () => {
		document.removeEventListener("pointerdown", onPress, true);
		window.removeEventListener("keydown", onKey, true);
	};
});

async function toggleMark(t: Title) {
	const was = watchedTitle(t, kind, lib.watch);
	try {
		const d = await title(kind, t.id);
		const targets =
			kind === "movies"
				? (d.media_files ?? []).slice(0, 1).map((f) => ({ media: f.id, ref: { movie_id: t.id } }))
				: (d.seasons ?? []).flatMap((s) =>
						s.episodes
							.filter((e) => e.has_file)
							.map((e) => ({
								media: e.media_file_id,
								ref: { series_id: t.id, season: s.number, episode_id: e.id },
							})),
					);
		await Promise.all(targets.map((x) => setWatched(x.media, !was)));
		for (const x of targets) applyWatch(x.media, !was, x.ref);
		toast(
			kind === "series"
				? was
					? i18n.detail_marked_series_unwatched()
					: i18n.detail_marked_series_watched()
				: was
					? i18n.detail_marked_unwatched()
					: i18n.detail_marked_watched(),
		);
	} catch {}
}

const open = (t: Title) => goto(`/detail/${kind}/${t.id}`);
const empty = $derived(lib.loaded && base.length === 0);

$effect(() => {
	if (kind === "series" && lib.loaded && lib.shows.some((s) => !s.seasons))
		untrack(() => loadSeasons(port));
});

onMount(() => {
	loadLibrary(port, lib.loaded);
});

const heads: { label: () => string; sort?: Sort; end?: boolean }[] = [
	{ label: () => i18n.library_sort_title(), sort: "title" },
	{ label: () => i18n.library_sort_year(), sort: "year" },
	{ label: () => i18n.library_col_res() },
	{ label: () => "HDR" },
	{ label: () => i18n.library_col_audio() },
	{ label: () => i18n.library_sort_size(), sort: "size", end: true },
	{ label: () => i18n.library_col_added(), sort: "added", end: true },
];
</script>

<div class="library" style:--cols={LIST_COLS} bind:this={box}>
	{#if lib.error !== null}
		<div class="state">
			<EmptyState icon="film" title={i18n.home_error_title()} text={lib.error}>
				{#snippet action()}
					<Button variant="secondary" onclick={() => loadLibrary(port, true)}>
						{i18n.halt_try_again()}
					</Button>
				{/snippet}
			</EmptyState>
		</div>
	{:else if empty}
		<div class="state">
			<EmptyState
				icon="film"
				title={kind === "movies"
					? i18n.library_empty_movies_title()
					: i18n.library_empty_shows_title()}
				text={i18n.home_empty_text()}
			/>
		</div>
	{:else if lib.loaded}
		<div class="page" style:padding="{TOP}px {PAD_X}px 56px">
			<div class="top">
				<div class="heading">
					<h1>{kind === "movies" ? i18n.nav_movies() : i18n.nav_shows()}</h1>
					<span class="count"
						>{kind === "movies"
							? i18n.library_count_movies({ count: items.length, total: base.length })
							: i18n.library_count_shows({ count: items.length, total: base.length })}</span
					>
				</div>
				<div class="tools">
					<div class="genre" use:dismiss={() => (genreMenu = false)}>
						<button
							type="button"
							class="genre-btn"
							class:on={view.genre !== null}
							aria-haspopup="listbox"
							aria-expanded={genreMenu}
							onclick={() => (genreMenu = !genreMenu)}
						>
							{view.genre ?? i18n.library_genre_all()}
							<span class="chev" class:open={genreMenu}
								><Icon name="chevron-down" size={13} stroke={2} /></span
							>
						</button>
						{#if genrePanel.mounted}
							<div class="genre-panel" class:shown={genrePanel.shown && genreMenu}>
								<FilterMenu
									options={[
										{ value: ALL, label: i18n.library_all(), count: flaggedTotal },
										...genreList.map((g) => ({ value: g.name, label: g.name, count: g.count })),
									]}
									value={view.genre ?? ALL}
									label={i18n.library_genre()}
									placeholder={i18n.library_genre_search()}
									emptyLabel={i18n.library_genre_none()}
									onchange={(v) => {
										set({ genre: v === ALL ? null : v });
										genreMenu = false;
									}}
								/>
							</div>
						{/if}
					</div>
					<fieldset class="flags" aria-label={i18n.library_show_only()}>
						{#each FLAGS as f (f)}
							<Chip
								pressed={view.flags.includes(f)}
								onclick={() => set({ flags: toggleFlag(view.flags, f) })}
							>
								{flagLabel[f]}
							</Chip>
						{/each}
					</fieldset>
					<span class="rule"></span>
					<SegmentedControl
						label={i18n.library_layout()}
						value={view.layout}
						options={[
							{ value: "grid", label: i18n.library_layout_grid(), icon: "layout-grid" },
							{ value: "list", label: i18n.library_layout_list(), icon: "list" },
						]}
						onchange={(layout: Layout) => set({ layout })}
					/>
					<SortMenu
						label={i18n.library_sort_by()}
						value={view.sort}
						options={SORTS.map((s) => ({ value: s, label: sortLabel(s) }))}
						onchange={(sort: Sort) => set({ sort })}
					/>
				</div>
			</div>

			{#if items.length && view.layout === "grid"}
				<div class="grid" style:--min="{MIN_COL}px">
					{#each items as t (t.id)}
						{@const watched = watchedTitle(t, kind, lib.watch)}
						<PosterCard
							title={t.title}
							sub={sub(t)}
							src={artwork(kind, t.id, "poster")}
							badge={isNew(t, now) ? i18n.home_new() : undefined}
							{watched}
							progress={progressOf(t, kind, lib.watch)}
							playLabel={playLabel(t)}
							mark={{
								on: watched,
								label: watched
									? i18n.library_mark_unwatched({ title: t.title })
									: i18n.library_mark_watched({ title: t.title }),
								onclick: () => toggleMark(t),
							}}
							lang={kind === "movies"
								? {
										own: !!choices[`movies:${t.id}`],
										label: langLabel(t),
										onclick: (el) => openLang(t, el),
									}
								: undefined}
							onplay={() => play(t)}
							onclick={() => open(t)}
						/>
					{/each}
				</div>
			{:else if items.length}
				<div class="table">
					<div class="head">
						<span></span>
						{#each heads as h, i (i)}
							<button
								type="button"
								class="th"
								class:end={h.end}
								class:sorted={h.sort === view.sort}
								class:inert={!h.sort}
								onclick={() => h.sort && set({ sort: h.sort })}
							>
								{h.label()}<Icon name="chevron-down" size={11} stroke={2.5} />
							</button>
						{/each}
					</div>
					{#each items as t (t.id)}
						{@const q = t.file_summary}
						<TitleRow
							title={t.title}
							poster={artwork(kind, t.id, "poster")}
							watched={watchedTitle(t, kind, lib.watch)}
							progress={progressOf(t, kind, lib.watch)}
							year={kind === "series" ? years(t) : String(t.year)}
							res={q?.resolution ?? ""}
							hdr={q?.hdr || "SDR"}
							hdrOn={!!q?.hdr}
							audio={[q?.audio, q?.channels].filter(Boolean).join(" ")}
							size={sizeText(q?.size)}
							added={addedText(t.added_at, now)}
							onclick={() => open(t)}
						/>
					{/each}
				</div>
			{:else}
				<EmptyState icon="list-filter" title={i18n.library_nothing_matches()}>
					{#snippet action()}
						<Button variant="secondary" onclick={() => set({ flags: [], genre: null })}>
							{i18n.library_clear_filters()}
						</Button>
					{/snippet}
				</EmptyState>
			{/if}
		</div>
	{/if}
	{#if popover.mounted && shownLang}
		{@const audio = audioLanguages(shownLang.tracks?.audio ?? [])}
		<LanguagePopover
			heading={shownLang.t.title}
			title={i18n.detail_lang_title()}
			audioLabel={i18n.player_audio()}
			subsLabel={i18n.player_subtitles()}
			audio={audio.map((name) => ({
				value: name,
				label: languageNamed(name, shownLang.tracks?.audio ?? []),
			}))}
			subs={[
				{ value: "off", label: i18n.player_subtitles_off() },
				{ value: "forced", label: i18n.detail_forced_only() },
				...subtitleLanguages(shownLang.tracks?.subtitles ?? []).map((name) => ({
					value: name,
					label: languageNamed(name, shownLang.tracks?.subtitles ?? []),
				})),
			]}
			draft={shownLang.draft}
			hint=""
			applyLabel={i18n.detail_apply()}
			unavailable={audio.length ? undefined : i18n.detail_lang_unavailable()}
			x={shownLang.x}
			y={shownLang.y}
			up={shownLang.up}
			shown={popover.shown && lang !== null}
			onpick={(patch) => {
				if (lang) lang.draft = { ...lang.draft, ...patch };
			}}
			onapply={applyLang}
		/>
	{/if}
</div>

<style>
.library {
	position: relative;
	min-height: 100%;
	background: var(--bg-deep);
	color: var(--fg);
}

.state {
	display: grid;
	place-items: center;
	min-height: 100vh;
	padding-top: 64px;
	box-sizing: border-box;
}

.page {
	position: relative;
	display: flex;
	flex-direction: column;
	gap: 20px;
	box-sizing: border-box;
}

.top {
	display: flex;
	align-items: flex-end;
	justify-content: space-between;
	flex-wrap: wrap;
	gap: 14px 20px;
}

.heading {
	display: flex;
	align-items: baseline;
	gap: 14px;
}

h1 {
	margin: 0;
	font-size: 30px;
	font-weight: 700;
	letter-spacing: -0.025em;
}

.count {
	font: 400 12.5px var(--font-mono);
	color: var(--fg-subtle);
}

.tools {
	display: flex;
	align-items: center;
	gap: 8px;
}

.genre {
	position: relative;
}

.genre-btn {
	height: 32px;
	display: flex;
	align-items: center;
	gap: 7px;
	padding: 0 10px 0 12px;
	border-radius: var(--radius-full);
	border: 1px solid var(--border-strong);
	background: transparent;
	color: var(--fg-muted);
	font-size: 13px;
	font-weight: 500;
	white-space: nowrap;
	cursor: pointer;
	transition:
		background var(--dur-fast),
		border-color var(--dur-fast),
		color var(--dur-fast);
}

.genre-btn:hover {
	color: var(--fg);
}

.genre-btn.on {
	border-color: color-mix(in srgb, var(--accent) 45%, transparent);
	background: var(--accent-soft);
	color: var(--fg);
}

.chev {
	display: grid;
	transition: transform var(--dur-base) var(--ease);
}

.chev.open {
	transform: rotate(180deg);
}

.genre-panel {
	position: absolute;
	left: 0;
	top: 40px;
	z-index: 6;
	transform-origin: top left;
	opacity: 0;
	transform: translateY(-6px) scale(0.97);
	pointer-events: none;
	transition:
		opacity 130ms var(--ease-in),
		transform 150ms var(--ease-in);
}

.genre-panel.shown {
	opacity: 1;
	transform: translateY(0) scale(1);
	pointer-events: auto;
	transition:
		opacity 160ms var(--ease),
		transform 220ms var(--ease);
}

.flags {
	margin: 0;
	padding: 0;
	border: 0;
	min-width: 0;
	display: flex;
	align-items: center;
	gap: 6px;
}

.rule {
	width: 1px;
	height: 22px;
	background: var(--border-strong);
	margin: 0 6px;
}

.grid {
	display: grid;
	grid-template-columns: repeat(auto-fill, minmax(var(--min), 1fr));
	gap: 32px 24px;
}

.table {
	display: flex;
	flex-direction: column;
}

.head {
	display: grid;
	grid-template-columns: var(--cols);
	gap: 16px;
	align-items: center;
	height: 36px;
	padding: 0 12px;
	border-bottom: 1px solid var(--border);
}

.th {
	display: flex;
	align-items: center;
	gap: 4px;
	height: 36px;
	padding: 0;
	border: 0;
	background: transparent;
	color: var(--fg-faint);
	font: 500 10.5px var(--font-mono);
	letter-spacing: 0.1em;
	text-transform: uppercase;
	white-space: nowrap;
	cursor: pointer;
}

.th.end {
	justify-content: flex-end;
}

.th.sorted {
	color: var(--fg);
}

.th.inert {
	cursor: default;
}

.th :global(.sl-ico) {
	opacity: 0;
}

.th.sorted :global(.sl-ico) {
	opacity: 1;
}
</style>
