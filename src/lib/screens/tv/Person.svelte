<script lang="ts">
import { m as i18n } from "#lib/paraglide/messages.js";
import { goto } from "#lib/ui/nav.ts";
import LoadFailed from "#lib/tv/LoadFailed.svelte";
import TvScreen from "#lib/tv/TvScreen.svelte";
import type { Credit, Person } from "#lib/core/types.ts";
import { monogram, seasonSpan } from "#lib/core/format.ts";
import { artwork, headshot, person as fetchPerson } from "#platform";
import { hideBroken } from "#lib/ui/img.ts";

let { id }: { id: string } = $props();
let p = $state.raw<Person | null>(null);
let failed = $state<string | null>(null);
let bgHidden = $state(false);
let loadId = 0;

$effect(() => {
	const i = id;
	p = null;
	failed = null;
	bgHidden = false;
	load(i).catch((e) => (failed = String(e)));
});

async function load(i: string) {
	const call = ++loadId;
	const found = await fetchPerson(i);
	if (call === loadId) p = found;
}

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
const DEPARTMENTS: Record<string, () => string> = { Acting: i18n.person_acting };
const m = $derived(monogram(p?.name ?? ""));
const backdrop = $derived(credits[0]);
const firstKnown = $derived(p?.known_for.findIndex((k) => k.id !== undefined) ?? -1);

const open = (c: Pick<Credit, "kind" | "id" | "seasons">) =>
	goto(
		`/detail/${c.kind}/${c.id}${c.kind === "series" && c.seasons?.length ? `?s=${c.seasons[0]}` : ""}`,
	);
</script>

{#snippet section(name: string, list: Credit[], first: boolean)}
	<section>
		<h2>{name}<span class="count">{list.length}</span></h2>
		<div class="grid">
			{#each list as c, i (`${c.kind}:${c.id}`)}
				<button
					type="button"
					data-tv="1"
					data-fid="{c.kind}:{c.id}"
					data-tv-default={first && i === 0 && firstKnown < 0 ? "1" : undefined}
					class="card"
					onclick={() => open(c)}
				>
					<img src={artwork(c.kind, c.id, "poster")} alt="" loading="lazy" onerror={hideBroken}>
					<span class="t">{c.title}</span>
					<span class="r">{c.character}</span>
					<span class="s" class:span={c.kind === "series"}
						>{c.kind === "series"
							? seasonSpan(c.seasons ?? [], c.total_seasons ?? c.seasons?.length ?? 0)
							: c.year}</span
					>
				</button>
			{/each}
		</div>
	</section>
{/snippet}

<TvScreen onback={() => history.back()} scroll>
	{#if failed !== null}
		<LoadFailed
			error={failed}
			retry={() => load(id)}
			onloaded={() => (failed = null)}
			onback={() => history.back()}
		/>
	{:else if p}
		<div class="page">
			<div class="glow">
				{#if backdrop && !bgHidden}
					<img
						src={artwork(backdrop.kind, backdrop.id, "backdrop")}
						alt=""
						onerror={() => (bgHidden = true)}
					>
				{/if}
				<div class="fade"></div>
			</div>
			<div class="body">
				<header>
					<span class="face" style:background={m.bg} style:color={m.fg}
						>{m.initials}<img src={headshot(p.id)} alt="" onerror={hideBroken}></span
					>
					<div>
						<div class="eyebrow">{DEPARTMENTS[p.department]?.() ?? p.department}</div>
						<h1>{p.name}</h1>
						<div class="sum">{summary}</div>
					</div>
				</header>

				{#if p.known_for.length}
					<section>
						<h2>{i18n.person_known_for()}</h2>
						<div class="known">
							{#each p.known_for as k, i}
								{#if k.kind && k.id !== undefined}
									{@const kid = k.id}
									{@const kk = k.kind}
									<button
										type="button"
										data-tv="1"
										data-fid="known:{i}"
										data-tv-default={i === firstKnown ? "1" : undefined}
										class="ki"
										onclick={() => goto(`/detail/${kk}/${kid}`)}
									>
										<img class="kp" src={artwork(kk, kid, "poster")} alt="" onerror={hideBroken}>
										<span class="kt"
											><span class="ktt">{k.title}</span
											><span class="ks"
												>{[k.year, characterOf.get(`${kk}:${kid}`)]
													.filter(Boolean)
													.join(" · ")}</span
											></span
										>
										<span class="kst on">{i18n.person_on_server()}</span>
									</button>
								{:else}
									<div class="ki out">
										<span class="kp"></span>
										<span class="kt"
											><span class="ktt">{k.title}</span><span class="ks">{k.year}</span></span
										>
										<span class="kst">{i18n.person_not_in_library()}</span>
									</div>
								{/if}
							{/each}
						</div>
					</section>
				{/if}

				{#if series.length}
					{@render section(i18n.person_series_on_server(), series, true)}
				{/if}
				{#if movies.length}
					{@render section(i18n.person_movies_on_server(), movies, !series.length)}
				{/if}
			</div>
		</div>
	{/if}
</TvScreen>

<style>
.page {
	position: relative;
	min-height: 1080px;
	background: var(--bg-deep);
	color: var(--fg);
}

.glow {
	position: absolute;
	left: 0;
	right: 0;
	top: 0;
	height: 720px;
	overflow: hidden;
	pointer-events: none;
}

.glow img {
	position: absolute;
	left: -80px;
	top: -80px;
	width: calc(100% + 160px);
	height: calc(100% + 160px);
	object-fit: cover;
	filter: blur(64px) saturate(1.3);
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
	padding: 96px 120px 96px;
	display: flex;
	flex-direction: column;
	gap: 40px;
}

header {
	display: flex;
	align-items: center;
	gap: 44px;
}

.face {
	position: relative;
	flex: none;
	display: grid;
	place-items: center;
	width: 180px;
	height: 180px;
	border-radius: 50%;
	overflow: hidden;
	font-size: 60px;
	font-weight: 600;
	letter-spacing: -0.02em;
	box-shadow:
		0 0 0 1px rgba(255, 255, 255, 0.1),
		0 32px 76px rgba(0, 0, 0, 0.45);
}

.face img {
	position: absolute;
	inset: 0;
	width: 100%;
	height: 100%;
	object-fit: cover;
}

.eyebrow {
	font: 500 22px / 1 var(--font-mono);
	letter-spacing: 0.14em;
	text-transform: uppercase;
	color: var(--accent-text);
}

h1 {
	margin: 10px 0 8px;
	font-size: 76px;
	line-height: 1.05;
	font-weight: 700;
	letter-spacing: -0.03em;
}

.sum {
	font: 400 24px var(--font-mono);
	color: var(--fg-muted);
}

section {
	display: flex;
	flex-direction: column;
	gap: 18px;
}

h2 {
	margin: 0;
	font-size: 30px;
	font-weight: 600;
}

.count {
	margin-left: 14px;
	font: 400 22px var(--font-mono);
	color: var(--fg-subtle);
}

/* One row, as on the phone: a second row would push the credits below the fold. */
.known {
	display: flex;
	gap: 18px;
	overflow: hidden;
	margin: -12px -120px;
	padding: 12px 120px;
}

.ki {
	flex: none;
	width: 440px;
	display: flex;
	align-items: center;
	gap: 20px;
	min-height: 100px;
	padding: 16px 24px 16px 16px;
	border: 2px solid color-mix(in srgb, var(--fg) 10%, transparent);
	border-radius: 18px;
	background: var(--bg-elevated);
	color: var(--fg);
	text-align: left;
	font: inherit;
	cursor: pointer;
	outline: none;
	transition:
		transform 180ms var(--ease),
		border-color 180ms;
}

.ki:focus {
	transform: scale(1.04);
	border-color: var(--fg);
}

.ki.out {
	border-style: dashed;
	background: none;
	cursor: default;
}

.kp {
	flex: none;
	width: 54px;
	aspect-ratio: 2 / 3;
	border-radius: 6px;
	object-fit: cover;
	background: var(--bg-card);
}

.out .kp {
	background: var(--bg-elevated);
	box-shadow: inset 0 0 0 1px var(--border);
}

.kt {
	flex: 1;
	min-width: 0;
	display: flex;
	flex-direction: column;
	gap: 4px;
}

.ktt {
	font-size: 24px;
	font-weight: 600;
	white-space: nowrap;
	overflow: hidden;
	text-overflow: ellipsis;
}

.out .ktt {
	font-weight: 500;
	color: var(--fg-muted);
}

.ks {
	font: 400 19px var(--font-mono);
	color: var(--fg-subtle);
	white-space: nowrap;
	overflow: hidden;
	text-overflow: ellipsis;
}

.kst {
	flex: none;
	display: inline-flex;
	align-items: center;
	gap: 10px;
	font-size: 20px;
	white-space: nowrap;
	color: var(--fg-subtle);
}

.kst.on {
	color: var(--status-available);
}

.kst.on::before {
	content: "";
	width: 10px;
	height: 10px;
	border-radius: 50%;
	background: currentColor;
}

.grid {
	display: grid;
	grid-template-columns: repeat(auto-fill, 220px);
	gap: 44px 30px;
}

.card {
	display: flex;
	flex-direction: column;
	gap: 4px;
	padding: 0;
	border: 0;
	background: transparent;
	color: var(--fg);
	text-align: left;
	cursor: pointer;
	outline: none;
	min-width: 0;
}

.card img {
	width: 220px;
	aspect-ratio: 2 / 3;
	margin-bottom: 12px;
	border-radius: 14px;
	object-fit: cover;
	display: block;
	background: var(--bg-card);
	transition:
		transform 180ms var(--ease),
		box-shadow 180ms;
}

.card:focus img {
	transform: scale(1.06);
	box-shadow:
		0 0 0 4px var(--fg),
		0 24px 60px rgba(0, 0, 0, 0.7);
}

/* Above the focused poster's shadow, which would grey them out. */
.t,
.r,
.s {
	position: relative;
	white-space: nowrap;
	overflow: hidden;
	text-overflow: ellipsis;
}

.t {
	font-size: 24px;
	font-weight: 500;
}

.r {
	font-size: 21px;
	color: var(--fg-muted);
}

.s {
	font: 400 19px var(--font-mono);
	color: var(--fg-subtle);
}

.s.span {
	color: var(--accent-text);
}
</style>
