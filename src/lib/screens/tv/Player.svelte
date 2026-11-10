<script lang="ts">
import { episodesOf, stillPos } from "#lib/core/format.ts";
import type { Kind, Title } from "#lib/core/types.ts";
import { m as i18n } from "#lib/paraglide/messages.js";
import { artwork, everything, title as fetchTitle, watchState } from "#platform";
import PlayerControls from "#lib/screens/tv/PlayerControls.svelte";
import LoadFailed from "#lib/tv/LoadFailed.svelte";
import TvScreen from "#lib/tv/TvScreen.svelte";
import { afterNavigate, goto } from "$app/navigation";

type Play = {
	key: string;
	anime: boolean;
	mediaId: string;
	titleName: string;
	heading: string;
	eyebrow: string;
	frame: string;
	poster: string;
	framePos: string;
	start: number;
	next: {
		code: string;
		title: string;
		overview: string;
		still: string;
		stillPos: string;
		label: string;
		href: string;
	} | null;
	more: { title: string; poster: string; href: string }[];
};

let { kind, id, epId, resume }: { kind: Kind; id: number; epId: number | null; resume: boolean } =
	$props();
let play = $state.raw<Play | null>(null);
let failed = $state<string | null>(null);
let fromDetail = false;
let season: number | null = null;

afterNavigate(({ from }) => {
	if (from?.route.id?.startsWith("/detail")) fromDetail = true;
});

async function load(k: Kind, i: number, ep: number | null, wantsResume: boolean): Promise<Play> {
	const [t, watch] = await Promise.all([fetchTitle(k, i), watchState()]);
	if (k === "movies")
		return movie(t, wantsResume ? (watch.find((w) => w.movie_id === i)?.position_seconds ?? 0) : 0);
	const eps = episodesOf(t);
	const at = eps.findIndex((e) => e.id === ep);
	const e = eps[at];
	if (!e) throw new Error(`no episode ${ep}`);
	const following = eps[at + 1]?.has_file ? eps[at + 1] : undefined;
	const still = artwork("series", i, "backdrop");
	season = e.season;
	return {
		key: e.media_file_id,
		mediaId: e.media_file_id,
		titleName: t.title,
		anime: isAnime(t),
		heading: e.title,
		eyebrow: `${t.title} · S${e.season} E${e.number}`,
		frame: still,
		poster: artwork("series", i, "poster"),
		framePos: stillPos(e.number, e.season),
		start: wantsResume
			? (watch.find((w) => w.media_id === e.media_file_id)?.position_seconds ?? 0)
			: 0,
		next: following
			? {
					code: `S${following.season} · E${following.number}`,
					title: following.title,
					overview: following.overview,
					still,
					stillPos: stillPos(following.number, following.season),
					label: i18n.player_play_episode({ season: following.season, episode: following.number }),
					href: `/player/series/${i}?ep=${following.id}`,
				}
			: null,
		more: following ? [] : await moreLike(k, t),
	};
}

const isAnime = (t: Title) => t.genres.some((g) => /^(anime|animation)$/i.test(g));

async function moreLike(k: Kind, t: Title) {
	const g = t.genres[0] ?? "";
	const pool = (await everything()).filter((x) => !(x.kind === k && x.id === t.id));
	const more = [
		...pool.filter((x) => x.genres[0] === g),
		...pool.filter((x) => x.genres[0] !== g && x.genres.includes(g)),
		...pool.filter((x) => !x.genres.includes(g)),
	].slice(0, 6);
	return more.map((x) => ({
		title: x.title,
		poster: artwork(x.kind, x.id, "poster"),
		href: `/detail/${x.kind}/${x.id}`,
	}));
}

async function movie(t: Title, start: number): Promise<Play> {
	const mediaId = t.media_files?.[0]?.id ?? "";
	const g = t.genres[0] ?? "";
	return {
		key: mediaId,
		mediaId,
		titleName: t.title,
		anime: isAnime(t),
		heading: t.title,
		eyebrow: `${t.year} · ${g}`,
		frame: artwork("movies", t.id, "backdrop"),
		poster: artwork("movies", t.id, "poster"),
		framePos: "50% 40%",
		start,
		next: null,
		more: await moreLike("movies", t),
	};
}

$effect(() => {
	let stale = false;
	failed = null;
	load(kind, id, epId, resume).then(
		(p) => {
			if (!stale) play = p;
		},
		(e) => {
			if (!stale) failed = String(e);
		},
	);
	return () => {
		stale = true;
	};
});

const detail = $derived(`/detail/${kind}/${id}`);
function exit() {
	if (fromDetail) history.back();
	else goto(season ? `${detail}?s=${season}` : detail, { replace: true });
}
</script>

{#if failed !== null}
	<TvScreen onback={exit}>
		<LoadFailed
			error={failed}
			retry={() => load(kind, id, epId, resume).then((p) => (play = p))}
			onloaded={() => (failed = null)}
			onback={exit}
		/>
	</TvScreen>
{:else if play}
	{#key play.key}
		<PlayerControls
			{...play}
			onexit={exit}
			onnext={() => play?.next && goto(play.next.href, { replace: true })}
			onopen={(href: string) => goto(href, { replace: true })}
		/>
	{/key}
{/if}
