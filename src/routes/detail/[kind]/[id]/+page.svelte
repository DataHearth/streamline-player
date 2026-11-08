<script lang="ts">
	import { m as i18n } from "#lib/paraglide/messages.js";
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import { autoCap, PRESETS, presetOf, rememberedThroughput } from "#lib/player/auto.ts";
	import { prefs } from "#lib/tv/prefs.svelte.ts";
	import LoadFailed from "#lib/tv/LoadFailed.svelte";
	import TvScreen from "#lib/tv/TvScreen.svelte";
	import { artwork, badges, headshot, monogram, hideBroken, decisions, network as whereAmI, episodesOf, everything, inProgress, left, meta, orPoster, pct, runtime, setWatched, stillPos, title as fetchTitle, watchState, type Decision, type Kind, type Title, type Watch } from "#lib/tv/api.ts";
	import { toast } from "#lib/tv/toast.svelte.ts";
	import { Check, CheckCheck, Eye, Play, RotateCcw } from "@lucide/svelte";

	const MAX_SIMILAR = 10;
	const MAX_CAST = 14;
	const kind = $derived(page.params.kind as Kind);
	const id = $derived(Number(page.params.id));
	let t = $state.raw<Title | null>(null);
	let watch = $state.raw<Watch[]>([]);
	let similar = $state.raw<(Title & { kind: Kind })[]>([]);
	let season = $state<number | null>(null);
	let heroBlur = $state(false);
	let failed = $state<string | null>(null);
	let playback = $state.raw<Decision | null>(null);
	const PLAIN = { direct: i18n.detail_plays_as_is(), remux: i18n.detail_plays_original(), audio_transcode: i18n.detail_audio_converted(), transcode: i18n.detail_converted() };
	const seasonParam = $derived(Number(page.url.searchParams.get("s")) || null);
	let loadId = 0;

	$effect(() => {
		const [k, i] = [kind, id];
		t = null;
		watch = [];
		similar = [];
		season = null;
		heroBlur = false;
		failed = null;
		load(k, i).catch((e) => (failed = String(e)));
	});

	async function load(k: Kind, i: number) {
		const call = ++loadId;
		const [title, w, all] = await Promise.all([fetchTitle(k, i), watchState(), k === "movies" ? everything() : Promise.resolve([])]).catch((e) => {
			if (call === loadId) throw e;
			return [];
		});
		if (call !== loadId) return;
		t = title;
		watch = w;
		const g = title.genres[0] ?? "";
		const pool = all.filter((m) => !(m.kind === k && m.id === title.id));
		similar = [...pool.filter((m) => m.genres[0] === g).slice(0, MAX_SIMILAR), ...pool.filter((m) => m.genres[0] !== g && m.genres.includes(g)).slice(0, 4)].slice(0, MAX_SIMILAR);
	}

	const stateOf = (media?: string) => watch.find((w) => w.media_id === media);
	const movieMedia = $derived(t?.media_files?.[0]?.id);
	const movieWatch = $derived(stateOf(movieMedia));
	const eps = $derived(t ? episodesOf(t) : []);
	const available = $derived(eps.filter((e) => e.has_file));
	const progressEp = $derived(eps.find((e) => inProgress(stateOf(e.media_file_id))));
	const nextEp = $derived(progressEp ?? eps.find((e) => e.has_file && !stateOf(e.media_file_id)?.watched) ?? eps[0]);
	const curSeason = $derived(season ?? seasonParam ?? progressEp?.season ?? t?.seasons?.at(-1)?.number ?? 1);
	const shown = $derived(t?.seasons?.find((s) => s.number === curSeason)?.episodes ?? []);
	const watched = $derived(kind === "series" ? available.length > 0 && available.every((e) => stateOf(e.media_file_id)?.watched) : !!movieWatch?.watched);
	const hasRestart = $derived(kind === "movies" && inProgress(movieWatch));
	const primary = $derived.by(() => {
		if (kind === "series") return nextEp ? (progressEp ? i18n.detail_resume_episode : i18n.detail_play_episode)({ season: nextEp.season, episode: nextEp.number }) : i18n.common_play();
		return movieWatch && hasRestart ? i18n.detail_resume_left({ left: left(movieWatch.duration_seconds - movieWatch.position_seconds) }) : i18n.common_play();
	});
	const playMedia = $derived(kind === "series" ? nextEp?.media_file_id : movieMedia);
	const full = $derived(playback && !playback.error && (playback.mode === "direct" || playback.mode === "remux"));

	// Failing to know how it will play must not get in the way of the page.
	$effect(() => {
		const media = playMedia;
		playback = null;
		if (!media) return;
		let stale = false;
		Promise.all([decisions(media, [{}, ...PRESETS.map((n) => ({ max_bitrate: n * 1_000_000 }))]), whereAmI()]).then(
			([r, net]) => {
				if (stale) return;
				const original = r.decisions[0];
				const q = presetOf(net === "home" ? prefs.qHome : prefs.qAway);
				const cap = q === "original" ? null : q === "auto" ? autoCap(rememberedThroughput(net), r.bit_rate / 1_000_000, !!original && !original.error && original.video.action === "copy") : q;
				playback = r.decisions[cap ? PRESETS.indexOf(cap) + 1 : 0] ?? null;
			},
			() => {
				if (!stale) playback = null;
			},
		);
		return () => (stale = true);
	});

	// On a series, only who appears in the season on screen, as the design's desktop Detail does.
	const castShown = $derived((t?.cast ?? []).filter((c) => kind !== "series" || !c.seasons || c.seasons.includes(curSeason)));

	const eyebrow = $derived(t ? (kind === "series" ? [t.network, ...t.genres] : t.genres).filter(Boolean).join(" · ") : "");

	function play() {
		if (kind === "series") return nextEp && goto(`/player/series/${id}?ep=${nextEp.id}${progressEp ? "&resume=1" : ""}`);
		goto(`/player/movies/${id}${hasRestart ? "?resume=1" : ""}`);
	}

	async function toggleWatched() {
		const list = kind === "series" ? available.map((e) => e.media_file_id) : movieMedia ? [movieMedia] : [];
		const was = watched;
		await Promise.all(list.map((m) => setWatched(m, !was)));
		watch = await watchState();
		toast(kind === "series" ? (was ? i18n.detail_marked_series_unwatched() : i18n.detail_marked_series_watched()) : was ? i18n.detail_marked_unwatched() : i18n.detail_marked_watched());
	}
</script>

{#snippet castRow()}
	{#if castShown.length}
		<div class="more-title">{kind === "series" ? i18n.detail_cast_season({ n: curSeason }) : i18n.detail_cast()}<span class="count">{castShown.length}</span></div>
		<div class="cast">
			{#each castShown.slice(0, MAX_CAST) as c (c.id)}
				{@const m = monogram(c.name)}
				<button data-tv="1" data-fid="cast:{c.id}" class="castc" onclick={() => goto(`/person/${c.id}`)}>
					<span class="face" style:background={m.bg} style:color={m.fg}>
						{m.initials}<img src={headshot(c.id)} alt="" loading="lazy" onerror={hideBroken} />
					</span>
					<span class="cast-name">{c.name}</span>
					<span class="cast-role">{c.character}</span>
				</button>
			{/each}
		</div>
	{/if}
{/snippet}

<TvScreen onback={() => history.back()} scroll>
	{#if failed !== null}
		<LoadFailed error={failed} retry={() => load(kind, id)} onloaded={() => (failed = null)} onback={() => history.back()} />
	{:else if t}
		<div class="page">
			<div class="hero-img">
				<img
					src={artwork(kind, t.id, "backdrop")}
					alt=""
					style:filter={heroBlur ? "blur(36px) saturate(1.3)" : undefined}
					style:transform={heroBlur ? "scale(1.25)" : undefined}
					onerror={(e) => {
						orPoster(artwork(kind, id, "poster"))(e);
						heroBlur = true;
					}}
				/>
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
						<button data-tv="1" class="act" onclick={() => goto(`/player/movies/${id}`)}><RotateCcw size={26} strokeWidth={2} />{i18n.detail_from_start()}</button>
					{/if}
					<button data-tv="1" class="act mark" style:color={watched ? "var(--status-available)" : "var(--fg)"} onclick={toggleWatched}>
						{#if watched}<CheckCheck size={26} strokeWidth={2.2} />{i18n.detail_watched()}{:else}<Eye size={26} strokeWidth={2.2} />{i18n.detail_mark_watched()}{/if}
					</button>
				</div>
				{#if playback && !playback.error}
					<div class="status">
						<span class="status-dot" style:background={full ? "var(--status-available)" : "var(--status-downloading)"} style:box-shadow="0 0 0 5px color-mix(in srgb, {full ? 'var(--status-available) 18%' : 'var(--status-downloading) 20%'}, transparent)"></span>
						<span>{PLAIN[playback.mode]}</span>
					</div>
				{/if}
			</div>
			<div class="more">
				{#if kind === "series"}
					<div class="seasons">
						{#each t.seasons ?? [] as s}
							<button data-tv="1" class="season" class:on={s.number === curSeason} onclick={() => (season = s.number)} onfocus={() => (season = s.number)}>{i18n.common_season_n({ n: s.number })}</button>
						{/each}
					</div>
					<div class="eps">
						{#each shown as e}
							{@const w = stateOf(e.media_file_id)}
							<button data-tv="1" class="ep" style:opacity={e.has_file ? 1 : 0.55} style:cursor={e.has_file ? "pointer" : "default"} onclick={() => e.has_file && goto(`/player/series/${id}?ep=${e.id}${inProgress(w) ? "&resume=1" : ""}`)}>
								<span class="still">
									<img src={artwork("series", id, "backdrop")} alt="" loading="lazy" style:object-position={stillPos(e.number, curSeason)} style:filter={e.has_file ? "none" : "grayscale(1)"} onerror={orPoster(artwork("series", id, "poster"))} />
									{#if inProgress(w)}<span class="bar"><span style:width="{pct(w)}%"></span></span>{/if}
									{#if w?.watched}<span class="done"><Check size={22} strokeWidth={2.5} /></span>{/if}
								</span>
								<span class="ep-text">
									<span class="ep-num">{i18n.detail_episode_number({ n: String(e.number).padStart(2, "0") })} &middot; {runtime(e.runtime)}</span>
									<span class="ep-title">{e.title}</span>
								</span>
							</button>
						{/each}
					</div>
					{@render castRow()}
				{:else}
					{@render castRow()}
					<div class="more-title">{i18n.detail_more_like_this()}</div>
					<div class="similar">
						{#each similar as it (`${it.kind}:${it.id}`)}
							<button data-tv="1" class="sim" aria-label={it.title} onclick={() => goto(`/detail/${it.kind}/${it.id}`)}><img src={artwork(it.kind, it.id, "poster")} alt={it.title} loading="lazy" onerror={hideBroken} /></button>
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

	.status {
		display: flex;
		align-items: center;
		gap: 14px;
		margin-top: 4px;
		font: 400 22px var(--font-mono);
		color: var(--fg);
		white-space: nowrap;
	}

	.status-dot {
		width: 12px;
		height: 12px;
		border-radius: 50%;
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
		transition: transform var(--dur-fast);
	}

	.act.mark {
		padding: 0 30px;
	}

	.act.main {
		transition: transform var(--dur-fast), background var(--dur-fast);
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

	.count {
		margin-left: 14px;
		font: 400 22px var(--font-mono);
		color: var(--fg-subtle);
	}

	.cast {
		display: flex;
		gap: 12px;
		overflow: hidden;
		padding: 12px 110px 40px;
	}

	.castc {
		flex: none;
		width: 200px;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 8px;
		padding: 12px 10px 16px;
		border: 0;
		border-radius: 24px;
		background: none;
		color: var(--fg);
		text-align: center;
		cursor: pointer;
		outline: none;
	}

	.face {
		position: relative;
		display: grid;
		place-items: center;
		width: 150px;
		height: 150px;
		margin-bottom: 8px;
		border-radius: 50%;
		overflow: hidden;
		font-size: 48px;
		font-weight: 600;
		letter-spacing: -0.01em;
		box-shadow: 0 0 0 1px var(--border);
		transition: transform 180ms var(--ease), box-shadow 180ms;
	}

	.face img {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		object-fit: cover;
	}

	.castc:focus .face {
		transform: scale(1.08);
		box-shadow: 0 0 0 4px var(--fg), 0 24px 60px rgba(0, 0, 0, 0.7);
	}

	.cast-name,
	.cast-role {
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}

	.cast-name {
		font-size: 24px;
		font-weight: 600;
		line-height: 1.25;
	}

	.cast-role {
		font-size: 20px;
		line-height: 1.3;
		color: var(--fg-subtle);
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
