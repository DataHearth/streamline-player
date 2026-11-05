<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import TvScreen from "#lib/tv/TvScreen.svelte";
	import { artwork, badges, episodesOf, inProgress, left, library, meta, pct, runtime, setWatched, stillPos, title as fetchTitle, watchState, type Kind, type Title, type Watch } from "#lib/tv/api.ts";
	import { Check, CheckCheck, Eye, Play, RotateCcw } from "@lucide/svelte";

	const kind = $derived(page.params.kind as Kind);
	const id = $derived(Number(page.params.id));
	let t = $state.raw<Title | null>(null);
	let watch = $state.raw<Watch[]>([]);
	let similar = $state.raw<Title[]>([]);
	let season = $state<number | null>(null);

	$effect(() => {
		const [k, i] = [kind, id];
		t = null;
		season = null;
		Promise.all([fetchTitle(k, i), watchState(), k === "movies" ? library("movies") : Promise.resolve([])]).then(([title, w, all]) => {
			t = title;
			watch = w;
			similar = all.filter((m) => m.id !== title.id && m.genres[0] === title.genres[0]).slice(0, 10);
		});
	});

	const stateOf = (media?: string) => watch.find((w) => w.media_id === media);
	const movieMedia = $derived(t?.media_files?.[0]?.id);
	const movieWatch = $derived(stateOf(movieMedia));
	const eps = $derived(t ? episodesOf(t) : []);
	const progressEp = $derived(eps.find((e) => inProgress(stateOf(e.media_file_id))));
	const nextEp = $derived(progressEp ?? eps.find((e) => e.has_file && !stateOf(e.media_file_id)?.watched) ?? eps[0]);
	const curSeason = $derived(season ?? progressEp?.season ?? t?.seasons?.at(-1)?.number ?? 1);
	const shown = $derived(t?.seasons?.find((s) => s.number === curSeason)?.episodes ?? []);
	const watched = $derived(kind === "series" ? eps.length > 0 && eps.every((e) => stateOf(e.media_file_id)?.watched) : !!movieWatch?.watched);
	const hasRestart = $derived(kind === "movies" && inProgress(movieWatch));
	const primary = $derived.by(() => {
		if (kind === "series") return nextEp ? `${progressEp ? "Resume" : "Play"} S${nextEp.season} · E${nextEp.number}` : "Play";
		return movieWatch && hasRestart ? `Resume · ${left(movieWatch.duration_seconds - movieWatch.position_seconds)}` : "Play";
	});
	const eyebrow = $derived(t ? (kind === "series" ? [t.network, ...t.genres] : t.genres).filter(Boolean).join(" · ") : "");

	function play() {
		if (kind === "series") return nextEp && goto(`/player/series/${id}?ep=${nextEp.id}${progressEp ? "&resume=1" : ""}`);
		goto(`/player/movies/${id}${hasRestart ? "?resume=1" : ""}`);
	}

	async function toggleWatched() {
		const list = kind === "series" ? eps.map((e) => e.media_file_id) : movieMedia ? [movieMedia] : [];
		await Promise.all(list.map((m) => setWatched(m, !watched)));
		watch = await watchState();
	}
</script>

<TvScreen onback={() => history.back()} scroll>
	{#if t}
		<div class="page">
			<div class="hero-img">
				<img src={artwork(kind, t.id, "backdrop")} alt="" onerror={(e) => ((e.currentTarget as HTMLImageElement).src = artwork(kind, id, "poster"))} />
				<div class="shade side"></div>
				<div class="shade low"></div>
			</div>
			<div class="head">
				<div class="eyebrow">{eyebrow}</div>
				<h1>{t.title}</h1>
				<div class="meta">
					<span>{meta(kind, t)}</span>
					{#each badges(t.file_summary) as c}<span class="chip">{c}</span>{/each}
				</div>
				<p>{t.overview}</p>
				<div class="actions">
					<button data-tv="1" data-tv-default="1" class="act main" onclick={play}><Play size={28} fill="currentColor" />{primary}</button>
					{#if hasRestart}
						<button data-tv="1" class="act" onclick={() => goto(`/player/movies/${id}`)}><RotateCcw size={26} strokeWidth={2} />From the start</button>
					{/if}
					<button data-tv="1" class="act" style:color={watched ? "var(--status-available)" : "var(--fg)"} onclick={toggleWatched}>
						{#if watched}<CheckCheck size={26} strokeWidth={2.2} />Watched{:else}<Eye size={26} strokeWidth={2.2} />Mark watched{/if}
					</button>
				</div>
			</div>
			<div class="more">
				{#if kind === "series"}
					<div class="seasons" data-tv-row="1">
						{#each t.seasons ?? [] as s}
							<button data-tv="1" class="season" class:on={s.number === curSeason} onclick={() => (season = s.number)} onfocus={() => (season = s.number)}>Season {s.number}</button>
						{/each}
					</div>
					<div class="eps" data-tv-row="1">
						{#each shown as e}
							{@const w = stateOf(e.media_file_id)}
							<button data-tv="1" class="ep" onclick={() => goto(`/player/series/${id}?ep=${e.id}${inProgress(w) ? "&resume=1" : ""}`)}>
								<span class="still">
									<img src={artwork("series", id, "backdrop")} alt="" loading="lazy" style:object-position={stillPos(e.number)} />
									{#if inProgress(w)}<span class="bar"><span style:width="{pct(w)}%"></span></span>{/if}
									{#if w?.watched}<span class="done"><Check size={22} strokeWidth={2.5} /></span>{/if}
								</span>
								<span class="ep-text">
									<span class="ep-num">E{String(e.number).padStart(2, "0")} &middot; {runtime(e.runtime)}</span>
									<span class="ep-title">{e.title}</span>
								</span>
							</button>
						{/each}
					</div>
				{:else if similar.length}
					<div class="more-title">More like this</div>
					<div class="similar" data-tv-row="1">
						{#each similar as it}
							<button data-tv="1" class="sim" onclick={() => goto(`/detail/movies/${it.id}`)}><img src={artwork("movies", it.id, "poster")} alt={it.title} loading="lazy" /></button>
						{/each}
					</div>
				{/if}
			</div>
		</div>
	{/if}
</TvScreen>

<style>
	.page {
		position: relative;
		min-height: 1080px;
	}

	.hero-img {
		position: absolute;
		left: 0;
		right: 0;
		top: 0;
		height: 1080px;
		overflow: hidden;
	}

	.hero-img img {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		object-fit: cover;
		object-position: 60% 25%;
	}

	.shade {
		position: absolute;
		inset: 0;
	}

	.side {
		background: linear-gradient(90deg, color-mix(in srgb, var(--bg-deep) 97%, transparent) 0%, color-mix(in srgb, var(--bg-deep) 82%, transparent) 34%, color-mix(in srgb, var(--bg-deep) 15%, transparent) 70%);
	}

	.low {
		background: linear-gradient(0deg, var(--bg-deep) 0%, color-mix(in srgb, var(--bg-deep) 75%, transparent) 30%, transparent 60%);
	}

	.head {
		position: relative;
		padding: 120px 120px 0;
		display: flex;
		flex-direction: column;
		gap: 22px;
		max-width: 1240px;
	}

	.eyebrow {
		font: 500 22px/1 var(--font-mono);
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--accent-text);
	}

	h1 {
		margin: 0;
		font-size: 84px;
		line-height: 1;
		font-weight: 700;
		letter-spacing: -0.03em;
	}

	.meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 14px;
		font: 400 24px var(--font-mono);
		color: var(--fg-muted);
	}

	.chip {
		padding: 4px 12px;
		border-radius: 8px;
		border: 1.5px solid rgba(255, 255, 255, 0.2);
		font-size: 21px;
		color: var(--fg);
	}

	p {
		margin: 0;
		font-size: 27px;
		line-height: 1.5;
		color: var(--fg-muted);
		max-width: 1050px;
		display: -webkit-box;
		-webkit-line-clamp: 3;
		line-clamp: 3;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}

	.actions {
		display: flex;
		align-items: center;
		gap: 18px;
		margin-top: 14px;
	}

	.act {
		height: 72px;
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 0 32px;
		border: 0;
		border-radius: var(--radius-lg);
		background: rgba(255, 255, 255, 0.08);
		color: var(--fg);
		font-size: 27px;
		font-weight: 500;
		cursor: pointer;
		outline: none;
		transition: transform var(--dur-fast), background var(--dur-fast);
	}

	.act.main {
		gap: 16px;
		padding: 0 38px 0 32px;
		background: rgba(255, 255, 255, 0.14);
		font-weight: 600;
	}

	.act:focus {
		background: var(--player-ink);
		color: var(--fg-on-accent) !important;
		transform: scale(1.06);
	}

	.act.main:focus {
		box-shadow: 0 16px 40px rgba(0, 0, 0, 0.5);
	}

	.more {
		position: relative;
		margin-top: 56px;
		padding-bottom: 80px;
		display: flex;
		flex-direction: column;
		gap: 22px;
	}

	.seasons {
		display: flex;
		gap: 14px;
		padding: 0 120px;
	}

	.season {
		height: 56px;
		padding: 0 26px;
		border-radius: 28px;
		border: 0;
		background: var(--surface);
		color: var(--fg-subtle);
		font-size: 24px;
		font-weight: 500;
		cursor: pointer;
		outline: none;
		transition: transform var(--dur-fast);
	}

	.season.on {
		background: rgba(255, 255, 255, 0.14);
		color: var(--fg);
	}

	.season:focus {
		background: var(--player-ink);
		color: var(--fg-on-accent);
		transform: scale(1.06);
	}

	.eps {
		display: flex;
		gap: 28px;
		overflow: hidden;
		padding: 20px 120px 40px;
	}

	.ep {
		flex: none;
		width: 400px;
		display: flex;
		flex-direction: column;
		gap: 14px;
		padding: 0;
		border: 0;
		background: transparent;
		color: var(--fg);
		text-align: left;
		cursor: pointer;
		outline: none;
		border-radius: var(--radius-lg);
		transition: transform 180ms var(--ease);
	}

	.ep:focus {
		transform: scale(1.07);
	}

	.still {
		position: relative;
		display: block;
		width: 100%;
		aspect-ratio: 16 / 9;
		border-radius: var(--radius-lg);
		overflow: hidden;
		background: var(--bg-card);
	}

	.still img {
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
		right: 14px;
		top: 14px;
		width: 38px;
		height: 38px;
		display: grid;
		place-items: center;
		border-radius: 50%;
		background: color-mix(in srgb, var(--bg-deep) 80%, transparent);
		color: var(--status-available);
	}

	.ep-text {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: 0 4px;
	}

	.ep-num {
		font: 500 20px var(--font-mono);
		color: var(--fg-subtle);
	}

	.ep-title {
		font-size: 25px;
		font-weight: 600;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.more-title {
		padding: 0 120px;
		font-size: 30px;
		font-weight: 600;
	}

	.similar {
		display: flex;
		gap: 28px;
		overflow: hidden;
		padding: 12px 120px 40px;
	}

	.sim {
		flex: none;
		width: 220px;
		padding: 0;
		border: 0;
		border-radius: 14px;
		background: transparent;
		cursor: pointer;
		outline: none;
		transition: transform 180ms var(--ease), box-shadow 180ms;
	}

	.sim:focus {
		transform: scale(1.08);
		box-shadow: 0 0 0 4px var(--fg), 0 24px 60px rgba(0, 0, 0, 0.7);
	}

	.sim img {
		width: 220px;
		aspect-ratio: 2 / 3;
		border-radius: 14px;
		object-fit: cover;
		display: block;
	}
</style>
