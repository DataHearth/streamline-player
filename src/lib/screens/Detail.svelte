<script lang="ts">
import { playUrl, watchedTitle } from "#lib/core/browse.ts";
import {
	defaultSeason,
	episodeState,
	moreLikeThis,
	newEpisodes,
	primaryAction,
	techRows,
	type TechMode,
} from "#lib/core/detail.ts";
import {
	addedText,
	badges,
	episodesOf,
	left,
	meta,
	monogram,
	runtime,
	sizeText,
	stillPos,
} from "#lib/core/format.ts";
import { applyWatch, lib, loadLibrary } from "#lib/core/library.svelte.ts";
import { autoCap, PRESETS, presetOf, rememberedThroughput } from "#lib/core/playback/auto.ts";
import {
	applyChoice,
	audioLanguages,
	type Choice,
	type Choices,
	defaultChoice,
	effectiveChoice,
	loadChoices,
	type Ref,
	type Scope,
	saveChoices,
	subtitleLanguages,
} from "#lib/core/playback/choice.ts";
import {
	audioName,
	codecName,
	audioOutName,
	fileMbit,
	languageLabel,
	subInfo,
} from "#lib/core/playback/names.ts";
import type { Tracks } from "#lib/core/playback/port.ts";
import { isAnime, language } from "#lib/core/playback/rules.ts";
import { myLanguage, prefs } from "#lib/core/prefs.svelte.ts";
import { toast } from "#lib/core/toast.svelte.ts";
import type { Decision, Kind, Title } from "#lib/core/types.ts";
import { m as i18n } from "#lib/paraglide/messages.js";
import Art from "#lib/ui/Art.svelte";
import Button from "#lib/ui/Button.svelte";
import CastRow, { type Member } from "#lib/ui/CastRow.svelte";
import Chip from "#lib/ui/Chip.svelte";
import EmptyState from "#lib/ui/EmptyState.svelte";
import EpisodeList, { type Ep } from "#lib/ui/EpisodeList.svelte";
import Icon from "#lib/ui/Icon.svelte";
import LanguagePopover from "#lib/ui/LanguagePopover.svelte";
import { goto } from "#lib/ui/nav.ts";
import NotFound from "#lib/ui/NotFound.svelte";
import { presence } from "#lib/ui/presence.svelte.ts";
import { useScroll } from "#lib/ui/scroll.ts";
import TechDetails, { type Row } from "#lib/ui/TechDetails.svelte";
import {
	artwork,
	decisions,
	deviceInfo,
	headshot,
	home,
	library,
	network as whereAmI,
	PLAYER_NAME,
	playbackPort,
	setWatched,
	title as fetchTitle,
	watchState,
} from "#platform";
import { afterNavigate } from "$app/navigation";
import { page } from "$app/state";
import { onMount, untrack } from "svelte";

const PAD_X = 56;
const BAR = 64;
const MAX_SIMILAR = 8;
const MAX_CAST = 14;
const POPOVER_W = 264;
const POPOVER_H = 330;
const port = { library, watchState, home, title: fetchTitle };

let { kind, id }: { kind: Kind; id: number } = $props();

const now = new Date();
const scroller = useScroll();
let box = $state<HTMLElement>();
let innerHeight = $state(window.innerHeight);
let t = $state.raw<Title | null>(null);
let missing = $state(false);
let failed = $state<string | null>(null);
let fromApp = false;
let loadId = 0;
let locked = $state<number | null>(null);
let techOpen = $state<boolean | null>(null);
let playback = $state.raw<Decision | null>(null);
let bitRate = $state(0);
let device = $state("");
let tracks = $state.raw<Tracks | null>(null);
let choices = $state<Choices>(loadChoices());

afterNavigate(({ from }) => {
	fromApp = from !== null;
});

const ready = $derived(lib.loaded || lib.error !== null);
const series = $derived(kind === "series");
const primary = $derived(t ? primaryAction(t, kind, lib.watch) : null);
const asked = $derived(Number(page.url.searchParams.get("s")) || null);
const season = $derived(
	asked && t?.seasons?.some((s) => s.number === asked) ? asked : (locked ?? 1),
);

async function load(k: Kind, i: number) {
	const call = ++loadId;
	t = null;
	missing = false;
	failed = null;
	locked = null;
	techOpen = null;
	tracks = null;
	playback = null;
	try {
		const found = await fetchTitle(k, i);
		if (call === loadId) t = found;
	} catch (e) {
		if (call !== loadId) return;
		if (String(e).includes('"not_found"')) missing = true;
		else failed = String(e);
	}
}

$effect(() => {
	const [k, i] = [kind, id];
	untrack(() => load(k, i));
});

onMount(() => {
	loadLibrary(port, lib.loaded);
});

// The season Play lands in is fixed once, so marking episodes watched does not move the page.
$effect(() => {
	if (t && ready && locked === null) {
		const found = t;
		untrack(() => (locked = defaultSeason(found, lib.watch, null)));
	}
});

$effect(() => {
	const media = primary?.mediaId;
	tracks = null;
	if (!media) return;
	let stale = false;
	playbackPort.mediaTracks(media).then(
		(r) => {
			if (!stale) tracks = r;
		},
		() => {},
	);
	return () => (stale = true);
});

const PLAIN = $derived({
	direct: i18n.detail_plays_as_is(),
	remux: i18n.detail_plays_original(),
	audio_transcode: i18n.detail_audio_converted(),
	transcode: i18n.detail_converted(),
});

// Failing to know how it will play must not get in the way of the page.
$effect(() => {
	const media = primary?.mediaId;
	playback = null;
	if (!media) return;
	let stale = false;
	Promise.all([
		decisions(media, [{}, ...PRESETS.map((n) => ({ max_bitrate: n * 1_000_000 }))]),
		whereAmI(),
	]).then(
		([r, net]) => {
			if (stale) return;
			const original = r.decisions[0];
			const q = presetOf(net === "home" ? prefs.qHome : prefs.qAway);
			const cap =
				q === "original"
					? null
					: q === "auto"
						? autoCap(
								rememberedThroughput(net),
								r.bit_rate / 1_000_000,
								!!original && !original.error && original.video.action === "copy",
							)
						: q;
			bitRate = r.bit_rate;
			playback = r.decisions[cap ? PRESETS.indexOf(cap) + 1 : 0] ?? null;
		},
		() => {
			if (!stale) playback = null;
		},
	);
	return () => (stale = true);
});

onMount(() => {
	deviceInfo().then(
		(d) => (device = d.name),
		() => {},
	);
});

const decided = $derived(playback && !playback.error ? playback : null);
const full = $derived(decided && (decided.mode === "direct" || decided.mode === "remux"));

const watched = $derived(t ? watchedTitle(t, kind, lib.watch) : false);
const eps = $derived(t ? episodesOf(t) : []);
const available = $derived(eps.filter((e) => e.has_file));
const resuming = $derived(primary?.label === "resume");
const primaryText = $derived.by(() => {
	if (!primary) return i18n.common_play();
	if (series)
		return (resuming ? i18n.detail_resume_episode : i18n.detail_play_episode)({
			season: primary.season ?? 1,
			episode: primary.episode ?? 1,
		});
	const w = lib.watch.find((x) => x.media_id === primary.mediaId);
	return resuming && w
		? i18n.detail_resume_left({ left: left(w.duration_seconds - w.position_seconds) })
		: i18n.common_play();
});

const eyebrow = $derived(
	t ? (series ? [t.network, ...t.genres] : t.genres).filter(Boolean).join(" · ") : "",
);

function back() {
	if (fromApp) history.back();
	else goto("/");
}

function play() {
	if (!primary) return;
	goto(playUrl(kind, id, primary.episodeId, resuming));
}

const refOf = (s?: number, e?: number): Ref => ({ kind, id, season: s, episode: e });

type Target = {
	media: string;
	ref: { movie_id?: number; series_id?: number; season?: number; episode_id?: number };
};
const movieTargets = (): Target[] =>
	t?.media_files?.[0] ? [{ media: t.media_files[0].id, ref: { movie_id: id } }] : [];
const episodeTarget = (e: (typeof eps)[number]): Target => ({
	media: e.media_file_id,
	ref: { series_id: id, season: e.season, episode_id: e.id },
});

async function mark(targets: Target[], to: boolean, message: string) {
	try {
		await Promise.all(targets.map((x) => setWatched(x.media, to)));
		for (const x of targets) applyWatch(x.media, to, x.ref);
		toast(message);
	} catch {}
}

const toggleWatched = () =>
	mark(
		series ? available.map(episodeTarget) : movieTargets(),
		!watched,
		series
			? watched
				? i18n.detail_marked_series_unwatched()
				: i18n.detail_marked_series_watched()
			: watched
				? i18n.detail_marked_unwatched()
				: i18n.detail_marked_watched(),
	);

const seasonEps = $derived(t?.seasons?.find((s) => s.number === season)?.episodes ?? []);
const seasonTargets = $derived(
	seasonEps.filter((e) => e.has_file).map((e) => episodeTarget({ ...e, season })),
);
const seasonWatched = $derived(
	seasonEps.some((e) => e.has_file) &&
		seasonEps.filter((e) => e.has_file).every((e) => episodeState(e, lib.watch) === "watched"),
);
const arrived = $derived(newEpisodes(lib.feed, id, now));

const two = (n: number) => String(n).padStart(2, "0");
const epLabel = (n: number) => i18n.detail_episode_number({ n: two(n) });

const languageNamed = (name: string, from: { language: string }[]) => {
	const hit = from.find((x) => language(x.language) === name);
	return hit ? languageLabel(hit.language) : name;
};
const base = $derived<Choice>(
	tracks
		? defaultChoice(tracks.audio, {
				audioLang: prefs.audioLang,
				anime: isAnime(t?.genres ?? []),
				animeRule: prefs.animeRule,
				subsMode: prefs.subsMode,
				myLanguage: myLanguage(),
			})
		: { audio: "", subs: "forced" },
);
const choiceAt = (s?: number, e?: number) => effectiveChoice(choices, refOf(s, e)) ?? base;
const subsShort = (c: Choice) =>
	c.subs === "off"
		? i18n.detail_subs_none()
		: c.subs === "forced"
			? i18n.detail_subs_forced()
			: i18n.detail_subs_language({ language: languageNamed(c.subs, tracks?.subtitles ?? []) });
const audioShort = (c: Choice) => (c.audio ? languageNamed(c.audio, tracks?.audio ?? []) : "");
const langSummary = (c: Choice) => `${audioShort(c)} · ${subsShort(c)}`.replace(/^ · /, "");

const heroLang = $derived(
	i18n.detail_lang_label({
		audio: audioShort(effectiveChoice(choices, refOf()) ?? base),
		subs: subsShort(effectiveChoice(choices, refOf()) ?? base),
	}),
);
const seasonLang = $derived(
	i18n.detail_lang_label_season({
		n: season,
		audio: audioShort(choiceAt(season)),
		subs: subsShort(choiceAt(season)),
	}),
);

const episodes = $derived<Ep[]>(
	seasonEps.map((e) => {
		const state = episodeState(e, lib.watch);
		const w = lib.watch.find((x) => x.media_id === e.media_file_id);
		const isNew = e.has_file && arrived.has(`${season}:${e.number}`);
		const own = effectiveChoice(choices, refOf(season, e.number));
		const c = own ?? choiceAt(season);
		const exact = !!choices[`series:${id}:${season}:${e.number}`];
		return {
			key: String(e.id),
			num: epLabel(e.number),
			title: e.title,
			synopsis: state === "wanted" ? i18n.detail_wanted_synopsis() : e.overview,
			runtime: runtime(e.runtime),
			still: artwork("series", id, "backdrop"),
			pos: stillPos(e.number, season),
			state,
			isNew,
			pct:
				w && w.duration_seconds > 0
					? Math.round((w.position_seconds / w.duration_seconds) * 100)
					: 0,
			pill:
				state === "wanted"
					? { text: i18n.detail_wanted(), tone: "warn" }
					: state === "progress" && w
						? { text: left(w.duration_seconds - w.position_seconds), tone: "accent" }
						: state === "unwatched" && isNew
							? { text: i18n.home_new(), tone: "accent" }
							: null,
			playLabel: i18n.library_play_episode({ title: t?.title ?? "", season, episode: e.number }),
			markLabel:
				state === "watched"
					? i18n.detail_mark_episode_unwatched({ n: two(e.number) })
					: i18n.detail_mark_episode_watched({ n: two(e.number) }),
			langLabel: i18n.detail_lang_label_episode({
				n: two(e.number),
				audio: audioShort(c),
				subs: subsShort(c),
			}),
			langOwn: exact || !!own,
		};
	}),
);

const byKey = (key: string) => seasonEps.find((e) => String(e.id) === key);

function playEpisode(key: string) {
	const e = byKey(key);
	if (e) goto(playUrl("series", id, e.id, episodeState(e, lib.watch) === "progress"));
}

function markEpisode(key: string) {
	const e = byKey(key);
	if (!e) return;
	const was = episodeState(e, lib.watch) === "watched";
	mark(
		[episodeTarget({ ...e, season })],
		!was,
		was
			? i18n.detail_episode_marked_unwatched({ n: e.number })
			: i18n.detail_episode_marked_watched({ n: e.number }),
	);
}

const castShown = $derived(
	(t?.cast ?? []).filter((c) => !series || !c.seasons || c.seasons.includes(season)),
);
const members = $derived<Member[]>(
	castShown.slice(0, MAX_CAST).map((c) => {
		const m = monogram(c.name);
		return {
			id: c.id,
			name: c.name,
			character: c.character,
			photo: headshot(c.id),
			initials: m.initials,
			bg: m.bg,
			fg: m.fg,
		};
	}),
);

const similar = $derived(
	t
		? moreLikeThis(
				t,
				kind,
				[
					...lib.movies.map((x) => ({ ...x, kind: "movies" as Kind })),
					...lib.shows.map((x) => ({ ...x, kind: "series" as Kind })),
				],
				MAX_SIMILAR,
			)
		: [],
);

const modeText = (m: TechMode) =>
	m.kind === "asis"
		? i18n.detail_mode_as_is()
		: m.kind === "converted"
			? i18n.detail_mode_converted({ to: m.to })
			: m.kind === "burn"
				? i18n.detail_mode_burn({ formats: m.formats.join(", ") })
				: i18n.detail_mode_shown();
const modeTone = (m: TechMode) =>
	m.kind === "converted" ? "convert" : m.kind === "burn" ? "warn" : "copy";

const rows = $derived.by<Row[]>(() => {
	if (!t) return [];
	const d = decided;
	const q = t.file_summary;
	const label = {
		video: i18n.player_stat_video(),
		audio: i18n.player_audio(),
		subs: i18n.detail_row_subs(),
		file: i18n.detail_row_file(),
		bitrate: i18n.player_stat_bitrate(),
		added: i18n.detail_row_added(),
	};
	return techRows(t, kind, tracks, {
		video: d ? { copy: d.video.action === "copy", out: codecName(d.video.codec) } : undefined,
		audio: d
			? { copy: d.audio.action === "copy", out: audioOutName({ audio: d.audio }) }
			: undefined,
		subs: (tracks?.subtitles ?? []).map((s) => {
			const info = subInfo(s);
			return { format: info.fmt, burn: !!info.burnOnly };
		}),
		bitrate: bitRate ? i18n.detail_bitrate_avg({ rate: fileMbit(bitRate) }) : undefined,
		size: sizeText(q?.size),
		added: addedText(t.added_at, now),
	}).map((r) => ({
		label: label[r.key],
		value:
			r.key === "subs"
				? i18n.detail_tracks_formats({ count: r.count ?? 0, formats: r.value })
				: r.value,
		mode: r.mode && modeText(r.mode),
		tone: r.mode && modeTone(r.mode),
	}));
});

const techSummary = $derived.by(() => {
	const q = t?.file_summary;
	return [
		q?.resolution,
		q?.hdr,
		q?.audio && `${q.audio} ${q.channels ?? ""}`.trim(),
		series ? null : sizeText(q?.size),
	]
		.filter(Boolean)
		.join(" · ");
});

const statusDetail = $derived.by(() => {
	const d = decided;
	if (!d || !prefs.techDetails) return "";
	const q = t?.file_summary;
	if (d.mode === "direct")
		return i18n.detail_pb_direct({
			video: [q?.codec, q?.hdr].filter(Boolean).join(" "),
			audio: q?.audio ?? "",
		});
	if (d.mode === "remux")
		return i18n.detail_pb_remux({ container: tracks?.container ?? "", player: PLAYER_NAME });
	return i18n.detail_pb_reasons({
		reasons: [...d.video_reasons, ...d.audio_reasons].map((r) => r.text).join(", "),
	});
});

const audioTracks = $derived(
	tracks?.audio.length
		? tracks.audio.map((a) => `${languageLabel(a.language)} · ${audioName(a)}`)
		: [[t?.file_summary?.audio, t?.file_summary?.channels].filter(Boolean).join(" ")].filter(
				Boolean,
			),
);
const subsLine = $derived(
	tracks?.subtitles.length
		? i18n.detail_subs_line({
				list: tracks.subtitles
					.map((s) => {
						const info = subInfo(s);
						const name = languageLabel(s.language);
						return info.burnOnly
							? i18n.detail_sub_item_burn({ name, format: info.fmt })
							: i18n.detail_sub_item({ name, format: info.fmt });
					})
					.join(", "),
			})
		: "",
);

type Open = { scope: Scope; ep: number | null; draft: Choice; x: number; y: number; up: boolean };
let lang = $state<Open | null>(null);
const popover = presence(() => lang !== null, 170);
let lastLang = $state.raw<Open | null>(null);
$effect(() => {
	if (lang) lastLang = lang;
});
const shownLang = $derived(lang ?? lastLang);

function openLang(scope: Scope, el: HTMLElement, ep: number | null) {
	if (lang && lang.scope === scope && lang.ep === ep) return closeLang();
	const s = scope === "title" ? undefined : season;
	const draft = { ...(effectiveChoice(choices, refOf(s, ep ?? undefined)) ?? base) };
	let x = 0;
	let y = BAR;
	let up = false;
	if (box) {
		const b = box.getBoundingClientRect();
		const r = el.getBoundingClientRect();
		x = Math.max(0, Math.min(box.offsetWidth - POPOVER_W, r.right - b.left - POPOVER_W));
		y = r.bottom - b.top + 8;
		const host = scroller?.();
		const bottom = host ? host.getBoundingClientRect().bottom : window.innerHeight;
		const top = (host ? host.getBoundingClientRect().top : 0) + BAR + 8;
		up = r.bottom + POPOVER_H > bottom && r.top - POPOVER_H >= top;
		if (up) y = r.top - b.top - 8;
	}
	lang = { scope, ep, draft, x, y, up };
}

const closeLang = () => {
	lang = null;
};

function pick(patch: Partial<Choice>) {
	if (lang) lang.draft = { ...lang.draft, ...patch };
}

function applyLang() {
	if (!lang) return;
	const ref = refOf(lang.scope === "title" ? undefined : season, lang.ep ?? undefined);
	choices = applyChoice(choices, ref, lang.scope, lang.draft);
	saveChoices(choices);
	closeLang();
}

$effect(() => {
	if (!lang) return;
	const onPress = (e: PointerEvent) => {
		const el = e.target as Element | null;
		if (el?.closest('[role="dialog"]') || el?.closest("[data-lang-trigger]")) return;
		closeLang();
	};
	const onKey = (e: KeyboardEvent) => {
		if (e.key !== "Escape") return;
		e.stopPropagation();
		closeLang();
	};
	document.addEventListener("pointerdown", onPress, true);
	window.addEventListener("keydown", onKey, true);
	return () => {
		document.removeEventListener("pointerdown", onPress, true);
		window.removeEventListener("keydown", onKey, true);
	};
});

const audioOptions = $derived(
	audioLanguages(tracks?.audio ?? []).map((name) => ({
		value: name,
		label: languageNamed(name, tracks?.audio ?? []),
	})),
);
const subsOptions = $derived([
	{ value: "off", label: i18n.player_subtitles_off() },
	{ value: "forced", label: i18n.detail_forced_only() },
	...subtitleLanguages(tracks?.subtitles ?? []).map((name) => ({
		value: name,
		label: languageNamed(name, tracks?.subtitles ?? []),
	})),
]);
const langTitle = $derived(
	!shownLang
		? ""
		: shownLang.ep !== null
			? i18n.detail_lang_title_episode({ n: two(shownLang.ep) })
			: shownLang.scope === "season"
				? i18n.detail_lang_title_season({ n: season })
				: i18n.detail_lang_title(),
);
const langHint = $derived(
	!shownLang
		? ""
		: !series
			? i18n.detail_lang_hint_movie()
			: shownLang.ep !== null
				? i18n.detail_lang_hint_episode({ n: two(shownLang.ep) })
				: shownLang.scope === "season"
					? i18n.detail_lang_hint_season({ n: season })
					: i18n.detail_lang_hint_series(),
);

function setSeason(n: number) {
	goto(`/detail/${kind}/${id}?s=${n}`, { replace: true, reset: false });
}

const heroHeight = $derived(
	!series && castShown.length ? Math.max(420, innerHeight - 250) : Math.round(innerHeight * 0.78),
);
</script>

<svelte:window bind:innerHeight />

<div class="detail" bind:this={box}>
	{#if missing}
		<NotFound
			title={i18n.detail_not_found()}
			label={i18n.detail_back_home()}
			onback={() => goto("/")}
		/>
	{:else if failed !== null}
		<div class="state">
			<EmptyState icon="film" title={i18n.home_error_title()} text={failed}>
				{#snippet action()}
					<Button variant="secondary" onclick={() => load(kind, id)}
						>{i18n.halt_try_again()}</Button
					>
				{/snippet}
			</EmptyState>
		</div>
	{:else if t && ready}
		<section class="hero" style:min-height="{heroHeight}px">
			<div class="backdrop">
				<Art src={artwork(kind, t.id, "backdrop")} position="50% 25%" />
				<div class="side"></div>
				<div class="low"></div>
			</div>
			<button type="button" class="back" onclick={back}>
				<Icon name="arrow-left" size={14} stroke={2} />{i18n.common_back()}
			</button>
			<div class="copy">
				<div class="eyebrow">{eyebrow}</div>
				<h1 style:font-size="{t.title.length > 24 ? 46 : 56}px">{t.title}</h1>
				<div class="meta">
					<span class="line">{meta(kind, t)}</span>
					{#each badges(t.file_summary) as c}
						<Chip>{c}</Chip>
					{/each}
				</div>
				<p>{t.overview}</p>
				<div class="actions">
					<button type="button" class="cta" disabled={!primary} onclick={play}>
						<Icon name="play" filled size={18} />{primaryText}
					</button>
					{#if !series && resuming}
						<button type="button" class="glass restart" onclick={() => goto(playUrl("movies", id))}>
							<Icon name="rotate-ccw" size={17} />{i18n.detail_from_start()}
						</button>
					{/if}
					<button
						type="button"
						class="glass sq mark-title"
						style:color={watched ? "var(--status-available)" : "var(--fg)"}
						aria-label={watched ? i18n.detail_mark_as_unwatched() : i18n.detail_mark_as_watched()}
						title={watched ? i18n.detail_mark_as_unwatched() : i18n.detail_mark_as_watched()}
						onclick={toggleWatched}
					>
						<Icon name={watched ? "check-check" : "eye"} size={19} stroke={2} />
					</button>
					<button
						type="button"
						class="glass sq"
						data-lang-trigger="hero"
						aria-label={heroLang}
						title={heroLang}
						aria-expanded={lang?.scope === "title"}
						onclick={(e) => openLang("title", e.currentTarget, null)}
					>
						<Icon name="languages" size={19} stroke={1.9} />
					</button>
				</div>
				{#if decided}
					<div class="status">
						<span class="dot" class:copy={full}></span>
						<span class="plain">{PLAIN[decided.mode]}</span>
						<span class="detail-note">{statusDetail}</span>
					</div>
				{/if}
			</div>
		</section>

		<div class="body" style:padding="8px {PAD_X}px 56px">
			{#if series && t.seasons?.length}
				<EpisodeList
					seasons={t.seasons.map((s) => ({
						n: s.number,
						label: i18n.common_season_n({ n: s.number }),
					}))}
					{season}
					note={i18n.detail_season_note({
						count: seasonEps.length,
						watched: episodes.filter((e) => e.state === "watched").length,
					})}
					seasonMark={{
						watched: seasonWatched,
						label: seasonWatched
							? i18n.detail_mark_season_unwatched({ n: season })
							: i18n.detail_mark_season_watched({ n: season }),
						onclick: () =>
							mark(
								seasonTargets,
								!seasonWatched,
								seasonWatched
									? i18n.detail_season_marked_unwatched({ n: season })
									: i18n.detail_season_marked_watched({ n: season }),
							),
					}}
					seasonLang={{
						label: seasonLang,
						onclick: (e) => openLang("season", e.currentTarget as HTMLElement, null),
					}}
					eps={episodes}
					newLabel={i18n.home_new()}
					onseason={setSeason}
					onplay={playEpisode}
					onmark={markEpisode}
					onlang={(key, e) => {
						const ep = byKey(key);
						if (ep) openLang("episode", e.currentTarget as HTMLElement, ep.number);
					}}
				/>
			{/if}

			{#if members.length}
				<CastRow
					title={series ? i18n.detail_cast_season({ n: season }) : i18n.detail_cast()}
					count={castShown.length}
					{members}
					padX={PAD_X}
					onopen={(person) => goto(`/person/${person}`)}
				/>
			{/if}

			<TechDetails
				label={i18n.detail_technical()}
				summary={techSummary}
				open={techOpen ?? prefs.techDetails}
				status={{
					label: decided ? PLAIN[decided.mode] : "",
					where: device ? i18n.detail_on_device({ device }) : "",
					copy: !!full,
				}}
				{rows}
				audioTitle={i18n.detail_audio_tracks()}
				audio={audioTracks}
				subsTitle={i18n.player_subtitles()}
				subs={series ? undefined : subsLine}
				ontoggle={() => (techOpen = !(techOpen ?? prefs.techDetails))}
			/>

			{#if similar.length}
				<section class="more">
					<h2>{i18n.detail_more_like_this()}</h2>
					<div class="similar">
						{#each similar as it (`${it.kind}:${it.id}`)}
							<button type="button" onclick={() => goto(`/detail/${it.kind}/${it.id}`)}>
								<span class="poster"
									><Art src={artwork(it.kind, it.id, "poster")} kind="poster" ratio="2 / 3" /></span
								>
								<span class="name">{it.title}</span>
							</button>
						{/each}
					</div>
				</section>
			{/if}
		</div>

		{#if popover.mounted && shownLang}
			<LanguagePopover
				title={langTitle}
				audioLabel={i18n.player_audio()}
				subsLabel={i18n.player_subtitles()}
				audio={audioOptions}
				subs={subsOptions}
				draft={shownLang.draft}
				hint={langHint}
				applyLabel={i18n.detail_apply()}
				unavailable={audioOptions.length ? undefined : i18n.detail_lang_unavailable()}
				x={shownLang.x}
				y={shownLang.y}
				up={shownLang.up}
				shown={popover.shown && lang !== null}
				onpick={pick}
				onapply={applyLang}
			/>
		{/if}
	{/if}
</div>

<style>
.detail {
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

.hero {
	position: relative;
	display: flex;
	flex-direction: column;
	justify-content: flex-end;
	box-sizing: border-box;
	padding: 134px 56px 44px;
	overflow: hidden;
}

.backdrop {
	position: absolute;
	inset: 0;
}

.side {
	position: absolute;
	inset: 0;
	background: linear-gradient(
		90deg,
		color-mix(in srgb, var(--bg-deep) 95%, transparent) 0%,
		var(--bg-overlay) 36%,
		color-mix(in srgb, var(--bg-deep) 10%, transparent) 72%
	);
}

.low {
	position: absolute;
	inset: 0;
	background: linear-gradient(
		0deg,
		var(--bg-deep) 0%,
		color-mix(in srgb, var(--bg-deep) 50%, transparent) 26%,
		transparent 50%
	);
}

.back {
	position: absolute;
	left: 56px;
	top: 78px;
	display: flex;
	align-items: center;
	gap: 6px;
	height: 34px;
	padding: 0 13px 0 10px;
	border: 1px solid var(--border-strong);
	border-radius: var(--radius-full);
	background: rgba(0, 0, 0, 0.4);
	backdrop-filter: blur(8px);
	-webkit-backdrop-filter: blur(8px);
	color: var(--fg-muted);
	font-size: 12.5px;
	font-weight: 500;
	cursor: pointer;
}

.back:hover {
	background: rgba(0, 0, 0, 0.6);
	color: var(--fg);
}

.copy {
	position: relative;
	max-width: 660px;
	display: flex;
	flex-direction: column;
	gap: 14px;
}

.eyebrow {
	font: 500 11px / 1 var(--font-mono);
	letter-spacing: 0.14em;
	text-transform: uppercase;
	color: var(--accent-text);
	white-space: nowrap;
}

h1 {
	margin: 0;
	line-height: 1.02;
	font-weight: 700;
	letter-spacing: -0.03em;
	text-wrap: balance;
}

.meta {
	display: flex;
	flex-wrap: wrap;
	align-items: center;
	gap: 8px;
	font: 400 12.5px var(--font-mono);
	color: var(--fg-muted);
}

.line {
	white-space: nowrap;
}

p {
	margin: 0;
	max-width: 600px;
	font-size: 15px;
	line-height: 1.55;
	color: var(--fg-muted);
	text-wrap: pretty;
}

.actions {
	display: flex;
	flex-wrap: wrap;
	align-items: center;
	gap: 10px;
	margin-top: 6px;
}

.cta {
	display: inline-flex;
	align-items: center;
	justify-content: center;
	gap: 10px;
	min-width: 184px;
	height: 46px;
	padding: 0 22px 0 18px;
	border: 0;
	border-radius: var(--radius-md);
	background: var(--accent);
	color: var(--fg-on-accent);
	font-size: 15px;
	font-weight: 600;
	white-space: nowrap;
	cursor: pointer;
	transition:
		background 150ms,
		box-shadow 150ms,
		transform 150ms;
}

.cta:hover {
	background: var(--accent-hover);
	box-shadow: 0 0 24px color-mix(in srgb, var(--accent) 35%, transparent);
}

.cta:active {
	transform: scale(0.97);
}

.cta:disabled {
	opacity: 0.5;
	pointer-events: none;
}

.glass {
	display: inline-flex;
	align-items: center;
	justify-content: center;
	gap: 9px;
	flex: none;
	height: 46px;
	padding: 0 18px;
	border: 1px solid rgba(255, 255, 255, 0.16);
	border-radius: var(--radius-md);
	background: var(--player-chip);
	backdrop-filter: blur(12px);
	-webkit-backdrop-filter: blur(12px);
	color: var(--player-ink);
	font-size: 15px;
	font-weight: 500;
	white-space: nowrap;
	cursor: pointer;
}

.glass:hover {
	background: rgba(255, 255, 255, 0.14);
}

.glass.sq {
	width: 46px;
	padding: 0;
	color: var(--fg);
}

.status {
	display: flex;
	align-items: center;
	gap: 9px;
	margin-top: 2px;
	font: 400 12px var(--font-mono);
	color: var(--fg-muted);
}

.dot {
	width: 7px;
	height: 7px;
	border-radius: 50%;
	background: var(--status-downloading);
	box-shadow: 0 0 0 3px color-mix(in srgb, var(--status-downloading) 20%, transparent);
}

.dot.copy {
	background: var(--status-available);
	box-shadow: 0 0 0 3px color-mix(in srgb, var(--status-available) 18%, transparent);
}

.plain {
	color: var(--fg);
	white-space: nowrap;
}

.detail-note {
	color: var(--fg-subtle);
}

.body {
	display: flex;
	flex-direction: column;
	gap: 40px;
	box-sizing: border-box;
}

.more {
	display: flex;
	flex-direction: column;
	gap: 14px;
}

h2 {
	margin: 0;
	font-size: 18px;
	font-weight: 600;
}

.similar {
	display: grid;
	grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
	gap: 16px;
}

.similar > button {
	display: flex;
	flex-direction: column;
	gap: 8px;
	min-width: 0;
	padding: 0;
	border: 0;
	background: transparent;
	color: var(--fg);
	text-align: left;
	cursor: pointer;
}

.poster {
	position: relative;
	display: block;
	width: 100%;
	aspect-ratio: 2 / 3;
	border-radius: var(--radius-md);
	overflow: hidden;
	background: var(--bg-card);
	box-shadow: 0 0 0 1px var(--border);
	transition:
		box-shadow var(--dur-fast),
		transform var(--dur-fast);
}

.similar > button:hover .poster {
	box-shadow:
		0 0 0 2px color-mix(in srgb, var(--accent) 55%, transparent),
		0 16px 32px rgba(0, 0, 0, 0.45);
	transform: scale(1.03);
}

.name {
	font-size: 13px;
	font-weight: 500;
	white-space: nowrap;
	overflow: hidden;
	text-overflow: ellipsis;
}
</style>
