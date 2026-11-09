<script lang="ts">
import { m as i18n } from "#lib/paraglide/messages.js";
import { goto } from "$app/navigation";
import { page } from "$app/state";
import LoadFailed from "#lib/tv/LoadFailed.svelte";
import TvScreen from "#lib/tv/TvScreen.svelte";
import type { Kind, Title, Watch } from "#lib/core/types.ts";
import { episodesOf, inProgress, pct } from "#lib/core/format.ts";
import { artwork, library, title as fetchTitle, watchState } from "#lib/platform/native.ts";
import { hideBroken } from "#lib/ui/img.ts";
import { Check } from "@lucide/svelte";

type Flag = "unwatched" | "progress" | "uhd" | "hdr";
const FLAGS: [Flag, string][] = [
	["unwatched", i18n.library_unwatched()],
	["progress", i18n.library_in_progress()],
	["uhd", "4K"],
	["hdr", "HDR"],
];

const kind = $derived(page.params.kind as Kind);
let titles = $state.raw<Title[]>([]);
let watch = $state.raw<Watch[]>([]);
let loaded = $state(false);
let flags = $state<Flag[]>([]);
let doneShows = $state.raw(new Set<number>());
let failed = $state<string | null>(null);
let loadId = 0;

$effect(() => {
	const current = kind;
	flags = [];
	failed = null;
	load(current).catch((e) => (failed = String(e)));
});

async function load(current: Kind) {
	const id = ++loadId;
	titles = [];
	watch = [];
	doneShows = new Set();
	loaded = false;
	const [t, w] = await Promise.all([library(current), watchState()]).catch((e) => {
		// A failure of the load the viewer already left is not theirs to see.
		if (id === loadId) throw e;
		return [];
	});
	if (id !== loadId) return;
	titles = [...t].sort((a, b) => b.added_at.localeCompare(a.added_at));
	watch = w;
	loaded = true;
	if (current === "series") markDoneShows(w, id).catch(() => {});
}

async function markDoneShows(w: Watch[], id: number) {
	const started = [...new Set(w.filter((x) => x.watched).flatMap((x) => x.series_id ?? []))];
	const shows = await Promise.all(started.map((id) => fetchTitle("series", id)));
	if (id !== loadId) return;
	const seen = new Set(w.filter((x) => x.watched).map((x) => x.media_id));
	doneShows = new Set(
		shows
			.filter((s) => {
				const eps = episodesOf(s).filter((e) => e.has_file);
				return eps.length > 0 && eps.every((e) => seen.has(e.media_file_id));
			})
			.map((s) => s.id),
	);
}

const stateOf = (t: Title) =>
	watch.filter((w) => (kind === "movies" ? w.movie_id === t.id : w.series_id === t.id));
const going = (t: Title) => stateOf(t).find(inProgress);
const watched = (t: Title) =>
	kind === "movies" ? stateOf(t).some((w) => w.watched) : doneShows.has(t.id);

const test: Record<Flag, (t: Title) => boolean> = {
	unwatched: (t) => !watched(t) && !going(t),
	progress: (t) => !!going(t),
	uhd: (t) => t.file_summary?.resolution === "2160p",
	hdr: (t) => !!t.file_summary?.hdr,
};
const items = $derived(titles.filter((t) => flags.every((f) => test[f](t))));

function toggle(f: Flag) {
	const other: Partial<Record<Flag, Flag>> = { unwatched: "progress", progress: "unwatched" };
	flags = flags.includes(f)
		? flags.filter((x) => x !== f)
		: [...flags.filter((x) => x !== other[f]), f];
}
</script>

<TvScreen tab={failed === null ? kind : undefined} scroll>
	{#if failed !== null}
		<LoadFailed
			error={failed}
			retry={() => load(kind)}
			onloaded={() => (failed = null)}
			onback={() => history.back()}
		/>
	{:else}
		{#key kind}
			<div class="page">
				<div class="head">
					<h1>{kind === "series" ? i18n.nav_shows() : i18n.nav_movies()}</h1>
					{#if loaded}
						<span class="count"
							>{(kind === "series" ? i18n.library_count_shows : i18n.library_count_movies)({
								count: items.length,
								total: titles.length,
							})}</span
						>
					{/if}
				</div>
				<div class="filters">
					{#each FLAGS as [f, label]}
						<button
							type="button"
							data-tv="1"
							class="filter"
							class:on={flags.includes(f)}
							onclick={() => toggle(f)}
						>
							{label}
						</button>
					{/each}
				</div>
				<div class="grid">
					{#each items as t (t.id)}
						<div class="item">
							<button
								type="button"
								data-tv="1"
								data-fid={String(t.id)}
								class="card"
								aria-label={t.title}
								onclick={() => goto(`/detail/${kind}/${t.id}`)}
							>
								<img src={artwork(kind, t.id, "poster")} alt="" loading="lazy" onerror={hideBroken}>
								{#if going(t)}
									<span class="bar"><span style:width="{pct(going(t))}%"></span></span>
								{/if}
								{#if watched(t)}
									<span class="done"><Check size={22} strokeWidth={3} /></span>
								{/if}
							</button>
							<span class="name">{t.title}</span>
						</div>
					{/each}
				</div>
			</div>
		{/key}
	{/if}
</TvScreen>

<style>
.page {
	padding: 156px 96px 96px;
	display: flex;
	flex-direction: column;
	gap: 30px;
}

.head {
	display: flex;
	align-items: baseline;
	gap: 22px;
}

h1 {
	margin: 0;
	font-size: 58px;
	font-weight: 700;
	letter-spacing: -0.03em;
}

.count {
	font: 400 24px var(--font-mono);
	color: var(--fg-subtle);
}

.filters {
	display: flex;
	gap: 14px;
}

.filter {
	height: 56px;
	padding: 0 26px;
	border-radius: 28px;
	border: 0;
	background: var(--surface-2);
	color: var(--fg-muted);
	font-size: 23px;
	font-weight: 500;
	cursor: pointer;
	outline: none;
	transition: transform var(--dur-fast);
}

.filter.on {
	background: rgba(255, 255, 255, 0.16);
	color: var(--fg);
}

.filter:focus {
	background: var(--player-ink);
	color: var(--fg-on-accent);
	transform: scale(1.06);
}

.grid {
	display: grid;
	grid-template-columns: repeat(5, minmax(0, 1fr));
	gap: 52px 40px;
	padding-top: 10px;
}

.item {
	display: flex;
	flex-direction: column;
	gap: 16px;
	min-width: 0;
}

.card {
	position: relative;
	display: block;
	width: 100%;
	aspect-ratio: 2 / 3;
	padding: 0;
	border: 0;
	border-radius: 14px;
	overflow: hidden;
	background: var(--bg-card);
	cursor: pointer;
	outline: none;
	transition:
		transform 220ms var(--ease),
		box-shadow 220ms;
}

.card:focus {
	transform: translateY(-12px) scale(1.04);
	box-shadow:
		0 0 0 3px var(--bg-deep),
		0 0 0 8px var(--accent),
		0 0 64px color-mix(in srgb, var(--accent) 60%, transparent),
		0 28px 56px rgba(0, 0, 0, 0.65);
}

.card img {
	width: 100%;
	height: 100%;
	object-fit: cover;
	display: block;
}

.bar {
	position: absolute;
	left: 0;
	right: 0;
	bottom: 0;
	height: 6px;
	background: rgba(255, 255, 255, 0.2);
}

.bar > span {
	display: block;
	height: 100%;
	background: var(--accent);
}

.done {
	position: absolute;
	right: 12px;
	top: 12px;
	width: 40px;
	height: 40px;
	display: grid;
	place-items: center;
	border-radius: 50%;
	background: var(--accent);
	color: var(--fg-on-accent);
	box-shadow: 0 4px 14px rgba(0, 0, 0, 0.5);
}

.name {
	font-size: 26px;
	font-weight: 500;
	white-space: nowrap;
	overflow: hidden;
	text-overflow: ellipsis;
}
</style>
