<script lang="ts">
	import { goto } from "$app/navigation";
	import LoadFailed from "#lib/tv/LoadFailed.svelte";
	import TvScreen from "#lib/tv/TvScreen.svelte";
	import { artwork, everything, hideBroken, type Kind, type Title } from "#lib/tv/api.ts";
	import { search } from "#lib/tv/search.svelte.ts";
	import { Delete, Mic, Search } from "@lucide/svelte";

	type Entry = Title & { kind: Kind };
	const KEYS = "abcdefghijklmnopqrstuvwxyz1234567890".split("");
	const MAX_RESULTS = 12;

	let all = $state.raw<Entry[]>([]);
	let failed = $state<string | null>(null);

	const load = () => everything().then((t) => (all = t));
	load().catch((e) => (failed = String(e)));

	const norm = (x: string) => x.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
	const q = $derived(search.q.trim());
	const results = $derived.by(() => {
		if (!q) {
			const recent = (k: Kind) => all.filter((t) => t.kind === k).sort((a, b) => b.added_at.localeCompare(a.added_at)).slice(0, 4);
			return [...recent("movies"), ...recent("series")];
		}
		const nq = norm(q);
		const starts = (t: Entry) => (norm(t.title).startsWith(nq) ? 0 : 1);
		return all
			.filter((t) => norm(t.title).includes(nq) || t.genres.some((g) => norm(g).startsWith(nq)))
			.sort((a, b) => starts(a) - starts(b) || b.rating - a.rating);
	});
	const heading = $derived(q ? `${results.length} result${results.length === 1 ? "" : "s"}` : "Recently added");
</script>

<TvScreen tab={failed === null ? "search" : undefined}>
	{#if failed !== null}
		<LoadFailed error={failed} retry={load} onloaded={() => (failed = null)} onback={() => history.back()} />
	{:else}
		<div class="page">
			<div class="board">
				<div class="field">
					<Search size={32} strokeWidth={1.75} color="var(--fg-subtle)" />
					<span class="q" class:empty={!search.q}>{search.q || "Search"}</span><span class="caret"></span>
				</div>
				<div class="keys">
					{#each KEYS as k, i}
						<button data-tv="1" data-tv-default={i === 0 && !q ? "1" : undefined} class="key" onclick={() => (search.q += k)}>{k}</button>
					{/each}
				</div>
				<div class="wide">
					<button data-tv="1" class="key" onclick={() => (search.q += " ")}>Space</button>
					<button data-tv="1" class="key" aria-label="Delete" onclick={() => (search.q = search.q.slice(0, -1))}><Delete size="1em" strokeWidth={1.5} /></button>
					<button data-tv="1" class="key" onclick={() => (search.q = "")}>Clear</button>
				</div>
				<div class="hint"><Mic size={26} strokeWidth={1.75} />Hold the mic button on your remote to search by voice</div>
			</div>
			<div class="results">
				<div class="heading">{heading}</div>
				<div class="grid">
					{#each results.slice(0, MAX_RESULTS) as t (`${t.kind}:${t.id}`)}
						<button data-tv="1" data-fid="{t.kind}:{t.id}" class="card" onclick={() => goto(`/detail/${t.kind}/${t.id}`)}>
							<img src={artwork(t.kind, t.id, "poster")} alt="" loading="lazy" onerror={hideBroken} />
							<span class="name">{t.title}</span>
						</button>
					{/each}
				</div>
			</div>
		</div>
	{/if}
</TvScreen>

<style>
	.page {
		padding: 156px 96px 80px 96px;
		display: grid;
		grid-template-columns: 600px minmax(0, 1fr);
		gap: 64px;
		align-items: start;
	}

	.board {
		display: flex;
		flex-direction: column;
		gap: 22px;
	}

	.field {
		height: 84px;
		display: flex;
		align-items: center;
		gap: 16px;
		padding: 0 26px;
		border-radius: 18px;
		background: var(--bg-elevated);
		border: 2px solid color-mix(in srgb, var(--accent) 55%, transparent);
	}

	.q {
		font-size: 32px;
		color: var(--fg);
		/* A trailing Space would collapse and look unpressed. */
		white-space: pre;
		overflow: hidden;
	}

	.q.empty {
		color: var(--fg-faint);
	}

	.caret {
		flex: none;
		width: 3px;
		height: 40px;
		background: var(--accent);
		animation: pulse 1s steps(1) infinite;
	}

	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.3;
		}
	}

	.keys {
		display: grid;
		grid-template-columns: repeat(7, 72px);
		gap: 12px;
	}

	.wide {
		display: grid;
		grid-template-columns: 1fr 1fr 1fr;
		gap: 12px;
	}

	.key {
		display: grid;
		place-items: center;
		width: 100%;
		height: 72px;
		padding: 0;
		border: 0;
		border-radius: 14px;
		background: rgba(255, 255, 255, 0.07);
		color: var(--player-ink);
		font: 500 28px var(--font-sans);
		cursor: pointer;
		outline: none;
		transition: transform 150ms, background 150ms;
	}

	.wide .key {
		height: 68px;
		font-size: 24px;
	}

	.key:focus {
		background: var(--player-ink);
		color: #0b0b10;
		transform: scale(1.1);
	}

	.hint {
		display: flex;
		align-items: center;
		gap: 14px;
		font-size: 22px;
		white-space: nowrap;
		color: var(--fg-subtle);
	}

	.results {
		display: flex;
		flex-direction: column;
		gap: 24px;
		min-width: 0;
	}

	.heading {
		font-size: 30px;
		font-weight: 600;
	}

	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, 196px);
		gap: 40px 30px;
	}

	.card {
		display: flex;
		flex-direction: column;
		gap: 12px;
		padding: 0;
		border: 0;
		background: transparent;
		color: var(--fg);
		text-align: left;
		cursor: pointer;
		outline: none;
		border-radius: 14px;
		transition: transform 180ms var(--ease);
	}

	.card:focus {
		transform: scale(1.08);
	}

	.card img {
		width: 196px;
		aspect-ratio: 2 / 3;
		border-radius: 14px;
		object-fit: cover;
		display: block;
		background: var(--bg-card);
	}

	.name {
		font-size: 21px;
		font-weight: 500;
		width: 196px;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
