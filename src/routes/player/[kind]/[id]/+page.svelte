<script lang="ts">
	import { afterNavigate, goto } from "$app/navigation";
	import { page } from "$app/state";
	import TvPlayer from "#lib/player/TvPlayer.svelte";
	import { artwork, episodesOf, library, stillPos, title as fetchTitle, watchState, type Kind, type Title } from "#lib/tv/api.ts";

	type Play = { key: string; mediaId: string; heading: string; eyebrow: string; frame: string; framePos: string; start: number; next: { code: string; title: string; overview: string; still: string; stillPos: string; label: string; href: string } | null; more: { title: string; poster: string; href: string }[] };

	const kind = $derived(page.params.kind as Kind);
	const id = $derived(Number(page.params.id));
	const epId = $derived(Number(page.url.searchParams.get("ep")) || null);
	const resume = $derived(page.url.searchParams.has("resume"));
	let play = $state.raw<Play | null>(null);
	let fromDetail = false;

	afterNavigate(({ from }) => {
		if (from?.route.id?.startsWith("/detail")) fromDetail = true;
	});

	async function load(k: Kind, i: number, ep: number | null, wantsResume: boolean): Promise<Play> {
		const [t, watch] = await Promise.all([fetchTitle(k, i), watchState()]);
		if (k === "movies") return movie(t, wantsResume ? (watch.find((w) => w.movie_id === i)?.position_seconds ?? 0) : 0);
		const eps = episodesOf(t);
		const at = eps.findIndex((e) => e.id === ep);
		const e = eps[at];
		if (!e) throw new Error(`no episode ${ep}`);
		const following = eps[at + 1];
		const still = artwork("series", i, "backdrop");
		return {
			key: e.media_file_id,
			mediaId: e.media_file_id,
			heading: e.title,
			eyebrow: `${t.title} · S${e.season} E${e.number}`,
			frame: still,
			framePos: stillPos(e.number),
			start: wantsResume ? (watch.find((w) => w.media_id === e.media_file_id)?.position_seconds ?? 0) : 0,
			next: following
				? { code: `S${following.season} · E${following.number}`, title: following.title, overview: following.overview, still, stillPos: stillPos(following.number), label: `Play S${following.season} E${following.number}`, href: `/player/series/${i}?ep=${following.id}` }
				: null,
			more: [],
		};
	}

	async function movie(t: Title, start: number): Promise<Play> {
		const all = await library("movies");
		const mediaId = t.media_files?.[0]?.id ?? "";
		const g = t.genres[0] ?? "";
		const pool = all.filter((x) => x.id !== t.id);
		const more = [...pool.filter((x) => x.genres[0] === g), ...pool.filter((x) => x.genres[0] !== g && x.genres.includes(g)), ...pool.filter((x) => !x.genres.includes(g))].slice(0, 6);
		return {
			key: mediaId,
			mediaId,
			heading: t.title,
			eyebrow: `${t.year} · ${g}`,
			frame: artwork("movies", t.id, "backdrop"),
			framePos: "50% 40%",
			start,
			next: null,
			more: more.map((x) => ({ title: x.title, poster: artwork("movies", x.id, "poster"), href: `/detail/movies/${x.id}` })),
		};
	}

	$effect(() => {
		let stale = false;
		load(kind, id, epId, resume).then((p) => {
			if (!stale) play = p;
		});
		return () => {
			stale = true;
		};
	});

	const detail = $derived(`/detail/${kind}/${id}`);
	function exit() {
		if (fromDetail) history.back();
		else goto(detail, { replaceState: true });
	}
</script>

{#if play}
	{#key play.key}
		<TvPlayer {...play} onexit={exit} onnext={() => play?.next && goto(play.next.href, { replaceState: true })} onopen={(href: string) => goto(href, { replaceState: true })} />
	{/key}
{/if}
