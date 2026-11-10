<script lang="ts">
import {
	arrivalDay,
	continueWatching,
	isNew,
	metaLine,
	recentCount,
	recentlyAdded,
	topGenre,
	watchedTitle,
} from "#lib/core/browse.ts";
import { badges, episodesOf, left, runtime, stillPos } from "#lib/core/format.ts";
import { lib, loadLibrary } from "#lib/core/library.svelte.ts";
import type { Kind, Title } from "#lib/core/types.ts";
import { getLocale } from "#lib/paraglide/runtime.js";
import { m as i18n } from "#lib/paraglide/messages.js";
import Billboard, { type Slide } from "#lib/ui/Billboard.svelte";
import Button from "#lib/ui/Button.svelte";
import EmptyState from "#lib/ui/EmptyState.svelte";
import PosterCard from "#lib/ui/PosterCard.svelte";
import ResumeCard from "#lib/ui/ResumeCard.svelte";
import Row from "#lib/ui/Row.svelte";
import { goto } from "#lib/ui/nav.ts";
import { artwork, home, library, title, watchState } from "#platform";
import { onMount } from "svelte";

const PAD_X = 56;
const HERO_SHARE = 0.8;
const ROW_LIMIT = 12;
const OVERLAP = 112;
const BAR_CLEARANCE = 96;
const port = { library, watchState, home, title };

let innerHeight = $state(window.innerHeight);
const now = new Date();

const resume = $derived(continueWatching(lib.movies, lib.shows, lib.watch));
const byKey = (kind: Kind, id: number) =>
	(kind === "movies" ? lib.movies : lib.shows).find((t) => t.id === id);
const arrivedEpisodes = (id: number) =>
	lib.feed?.arrivals.flatMap((d) => d.items).find((a) => a.kind === "series" && a.id === id)
		?.episodes?.length ?? 0;

const fmt = {
	runtime,
	seasons: (n: number) =>
		n === 1 ? i18n.common_seasons_one({ count: n }) : i18n.common_seasons_other({ count: n }),
	rating: (n: number) => n.toLocaleString(getLocale()),
};
const resLabel = (t: Title) => badges(t.file_summary)[0];
const years = (t: Title) => t.first_aired ?? String(t.year);

const playUrl = (kind: Kind, id: number, episodeId?: number, resuming = false) =>
	`/player/${kind}/${id}${episodeId ? `?ep=${episodeId}${resuming ? "&resume=1" : ""}` : resuming ? "?resume=1" : ""}`;
const playResume = (r: (typeof resume)[number]) => goto(playUrl(r.kind, r.id, r.episodeId, true));
const open = (kind: Kind, id: number, season?: number) =>
	goto(`/detail/${kind}/${id}${season ? `?s=${season}` : ""}`);

const firstEpisode = (t: Title) => {
	const eps = episodesOf(t).filter((e) => e.has_file);
	const unseen = eps.find(
		(e) => !lib.watch.some((w) => w.media_id === e.media_file_id && w.watched),
	);
	return unseen ?? eps[0];
};

function eyebrow(f: NonNullable<typeof lib.feed>["featured"][number], t: Title) {
	if (f.reason === "continue") return i18n.home_continue_watching();
	if (f.reason === "new_episodes" && f.season !== undefined && f.episodes?.length) {
		const first = f.episodes[0];
		const last = f.episodes.at(-1);
		return first === last
			? i18n.home_eyebrow_episode({ season: f.season, episode: first as number })
			: i18n.home_eyebrow_episodes({ season: f.season, from: first as number, to: last as number });
	}
	const day = arrivalDay(t.added_at.slice(0, 10), now);
	return day === "today"
		? i18n.home_added_today()
		: day === "yesterday"
			? i18n.home_added_yesterday()
			: i18n.home_recently_added();
}

const slides = $derived.by(() => {
	const out: Slide[] = [];
	for (const f of lib.feed?.featured ?? []) {
		const t = byKey(f.kind, f.id);
		if (!t) continue;
		const going = resume.find((r) => r.kind === f.kind && r.id === f.id);
		const base = {
			key: `${f.kind}:${f.id}`,
			img: artwork(f.kind, f.id, "backdrop"),
			eyebrow: eyebrow(f, t),
			title: t.title,
			meta: metaLine(t, f.kind, fmt).join(" · "),
			chips: badges(t.file_summary),
			overview: t.overview,
			oninfo: () => open(f.kind, f.id, going?.season ?? f.season),
		};
		if (going) {
			out.push({
				...base,
				primary:
					going.kind === "series"
						? i18n.detail_resume_episode({
								season: going.season as number,
								episode: going.episode as number,
							})
						: i18n.common_resume(),
				progress: {
					pct: going.pct,
					left: going.sub ? `${going.sub} · ${left(going.left)}` : left(going.left),
				},
				onprimary: () => playResume(going),
			});
			continue;
		}
		const ep = f.kind === "series" ? firstEpisode(t) : undefined;
		out.push({
			...base,
			primary: i18n.common_play(),
			onprimary: () => goto(playUrl(f.kind, f.id, ep?.id)),
		});
	}
	return out;
});

const movieSub = (t: Title) => [t.year, resLabel(t)].filter(Boolean).join(" · ");
const showSub = (t: Title) =>
	[fmt.seasons(t.total_seasons ?? t.seasons?.length ?? 0), years(t)].join(" · ");
const badge = (t: Title, kind: Kind) => {
	const episodes = kind === "series" ? arrivedEpisodes(t.id) : 0;
	if (episodes) return i18n.home_new_count({ count: episodes });
	return isNew(t, now) ? i18n.home_new() : undefined;
};

const recentMovies = $derived(recentlyAdded(lib.movies, ROW_LIMIT));
const recentShows = $derived(recentlyAdded(lib.shows, ROW_LIMIT));
const genre = $derived(topGenre([...lib.movies, ...lib.shows]));
const genreTitles = $derived(
	genre
		? [
				...lib.movies
					.filter((t) => t.genres.includes(genre))
					.map((t) => ({ kind: "movies" as Kind, t })),
				...lib.shows
					.filter((t) => t.genres.includes(genre))
					.map((t) => ({ kind: "series" as Kind, t })),
			]
		: [],
);
const empty = $derived(lib.loaded && lib.movies.length + lib.shows.length === 0);

onMount(() => {
	loadLibrary(port, lib.loaded);
});
</script>

<svelte:window bind:innerHeight />

<div class="home">
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
			<EmptyState icon="film" title={i18n.home_empty_title()} text={i18n.home_empty_text()} />
		</div>
	{:else if lib.loaded}
		{#if slides.length}
			<Billboard
				{slides}
				height={Math.round(innerHeight * HERO_SHARE)}
				padX={PAD_X}
				infoLabel={i18n.common_more_info()}
				dotLabel={(name) => i18n.home_show_title({ title: name })}
			/>
		{/if}
		<div class="rows" style:margin-top="{slides.length ? -OVERLAP : BAR_CLEARANCE}px">
			{#if resume.length}
				<Row
					title={i18n.home_continue_watching()}
					note={i18n.home_count_in_progress({ count: resume.length })}
					padX={PAD_X}
					prevLabel={i18n.home_scroll_left()}
					nextLabel={i18n.home_scroll_right()}
				>
					{#each resume as r (r.mediaId)}
						{@const t = byKey(r.kind, r.id)}
						<ResumeCard
							title={r.title}
							code={r.code}
							sub={r.kind === "series"
								? r.sub
								: t
									? `${runtime(t.runtime ?? 0)} · ${badges(t.file_summary).slice(0, 2).join(" ")}`
									: ""}
							still={artwork(r.kind, r.id, "backdrop")}
							position={r.kind === "series"
								? stillPos(r.episode as number, r.season as number)
								: "50% 35%"}
							progress={r.pct}
							left={left(r.left)}
							onclick={() => playResume(r)}
						/>
					{/each}
				</Row>
			{/if}
			{#if recentMovies.length}
				<Row
					title={i18n.home_recent_movies()}
					note={i18n.home_count_month({ count: recentCount(recentMovies, now) })}
					padX={PAD_X}
					prevLabel={i18n.home_scroll_left()}
					nextLabel={i18n.home_scroll_right()}
				>
					{#each recentMovies as t (t.id)}
						<PosterCard
							title={t.title}
							sub={movieSub(t)}
							src={artwork("movies", t.id, "poster")}
							badge={badge(t, "movies")}
							watched={watchedTitle(t, "movies", lib.watch)}
							onclick={() => open("movies", t.id)}
						/>
					{/each}
				</Row>
			{/if}
			{#if recentShows.length}
				<Row
					title={i18n.home_recent_shows()}
					note={i18n.home_count_month({ count: recentCount(recentShows, now) })}
					padX={PAD_X}
					prevLabel={i18n.home_scroll_left()}
					nextLabel={i18n.home_scroll_right()}
				>
					{#each recentShows as t (t.id)}
						<PosterCard
							title={t.title}
							sub={showSub(t)}
							src={artwork("series", t.id, "poster")}
							badge={badge(t, "series")}
							watched={watchedTitle(t, "series", lib.watch)}
							onclick={() => open("series", t.id)}
						/>
					{/each}
				</Row>
			{/if}
			{#if genre && genreTitles.length}
				<Row
					title={genre}
					padX={PAD_X}
					prevLabel={i18n.home_scroll_left()}
					nextLabel={i18n.home_scroll_right()}
				>
					{#each genreTitles as { kind, t } (`${kind}:${t.id}`)}
						<PosterCard
							title={t.title}
							sub={kind === "movies" ? movieSub(t) : showSub(t)}
							src={artwork(kind, t.id, "poster")}
							watched={watchedTitle(t, kind, lib.watch)}
							onclick={() => open(kind, t.id)}
						/>
					{/each}
				</Row>
			{/if}
		</div>
	{/if}
</div>

<style>
.home {
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

.rows {
	position: relative;
	display: flex;
	flex-direction: column;
	gap: 34px;
	padding-bottom: 56px;
}
</style>
