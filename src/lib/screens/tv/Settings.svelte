<script lang="ts">
import { m as i18n } from "#lib/paraglide/messages.js";
import { goto } from "$app/navigation";
import LoadFailed from "#lib/tv/LoadFailed.svelte";
import TvScreen from "#lib/tv/TvScreen.svelte";
import { focus, focusDefault } from "#lib/tv/focus.ts";
import { prefs, setPref, type ChoiceKey, type ToggleKey } from "#lib/core/prefs.svelte.ts";
import {
	authState,
	deviceInfo,
	devices as listDevices,
	me as whoAmI,
	playerCapabilities,
	serverHost,
	signOut as signOutNow,
} from "#platform";
import type { Device, DeviceInfo, Me } from "#lib/core/types.ts";
import {
	Captions,
	Check,
	ChevronRight,
	CirclePlay,
	Eye,
	Languages,
	MonitorSmartphone,
	Tv,
	User,
} from "@lucide/svelte";
import { tick, type Component } from "svelte";

type Caps = {
	video: { codec: string }[];
	hdr: { formats: string[] };
	audio: { codecs: string[]; passthrough: string[] };
};
type Row = {
	label: string;
	desc?: string;
	value: string;
	choice?: ChoiceKey;
	vals?: string[];
	act?: () => void;
};
type Section = { key: string; title: string; icon: Component; rows: Row[] };

const QUALITIES = ["Auto", "Original", "20 Mbit/s", "8 Mbit/s", "4 Mbit/s", "2 Mbit/s", "1 Mbit/s"];
const PLAYABLE = 12;
const ACTIVE_SECS = 300;

let me = $state.raw<Me | null>(null);
let devices = $state.raw<Device[]>([]);
let host = $state("");
let keystore = $state("");
let info = $state.raw<DeviceInfo | null>(null);
let caps = $state.raw<Caps | null>(null);
let failed = $state<string | null>(null);
let current = $state("profile");
let pick = $state<ChoiceKey | null>(null);

async function load() {
	[me, devices, host, keystore] = await Promise.all([
		whoAmI(),
		listDevices(),
		serverHost(),
		authState().then((a) =>
			a.store_error ? i18n.common_key_not_stored({ error: a.store_error }) : a.keystore,
		),
	]);
	// Only a native player can answer these; the rows show a dash without them.
	deviceInfo()
		.then((i) => (info = i))
		.catch(() => {});
	refreshCaps();
}
load().catch((e) => (failed = String(e)));

async function signOut() {
	await signOutNow();
	goto("/welcome", { replace: true });
}

const refreshCaps = () =>
	playerCapabilities()
		.then((c) => (caps = c as Caps))
		.catch(() => {});

function playable(c: Caps) {
	const v = new Set(c.video.map((x) => x.codec));
	const a = new Set([...c.audio.codecs, ...c.audio.passthrough]);
	const hdr = c.hdr.formats;
	return [
		v.has("h264"),
		v.has("hevc"),
		v.has("av1"),
		v.has("vp9"),
		hdr.includes("hdr10"),
		hdr.includes("dolby_vision"),
		...["aac", "ac3", "eac3", "truehd", "dts", "flac"].map((x) => a.has(x)),
	].filter(Boolean).length;
}

function seen(d: Device, short = false) {
	if (d.streaming)
		return short ? i18n.settings_streaming_now_short() : i18n.settings_streaming_now();
	const s = (Date.now() - Date.parse(d.last_seen)) / 1000;
	if (s < ACTIVE_SECS) return short ? i18n.settings_active_now_short() : i18n.settings_active_now();
	if (s < 3600)
		return (short ? i18n.settings_seen_min_short : i18n.settings_seen_min)({
			count: Math.round(s / 60),
		});
	if (s < 86400)
		return (short ? i18n.settings_seen_h_short : i18n.settings_seen_h)({
			count: Math.round(s / 3600),
		});
	const count = Math.round(s / 86400);
	if (short)
		return (count === 1 ? i18n.settings_seen_days_one_short : i18n.settings_seen_days_other_short)({
			count,
		});
	return (count === 1 ? i18n.settings_seen_days_one : i18n.settings_seen_days_other)({ count });
}

// Stored values stay English (the player's rules read them); only what is shown is translated.
const VALUES: Record<string, () => string> = {
	Auto: i18n.value_auto,
	Original: i18n.value_original,
	"Original language": i18n.value_original_language,
	English: i18n.value_english,
	French: i18n.value_french,
	Always: i18n.value_always,
	None: i18n.value_none,
	Shadow: i18n.value_shadow,
	Box: i18n.value_box,
	White: i18n.value_white,
	Yellow: i18n.value_yellow,
	"On start": i18n.value_on_start,
	"Pass through": i18n.value_pass_through,
	"Decode here": i18n.value_decode_here,
};
const shown = (key: ChoiceKey, v: string) => {
	// Each app language is named in itself, so it can be found from any other.
	if (key === "language") return v;
	if (v === "Off") return key === "subsMode" ? i18n.value_off() : i18n.value_frame_off();
	return VALUES[v]?.() ?? v;
};

const choice = (label: string, key: ChoiceKey, vals: string[], desc?: string): Row => ({
	label,
	desc,
	value: shown(key, prefs[key]),
	choice: key,
	vals,
});
const toggle = (label: string, key: ToggleKey, desc?: string): Row => ({
	label,
	desc,
	value: prefs[key] ? i18n.common_on() : i18n.common_off(),
	act: () => setPref(key, !prefs[key]),
});
const show = (label: string, value: string, desc?: string): Row => ({ label, desc, value });

const sections = $derived.by((): Section[] => {
	const d = info?.display;
	const self = devices.find((x) => x.current);
	return [
		{
			key: "profile",
			title: i18n.settings_profile(),
			icon: User,
			rows: [
				show(i18n.settings_name(), me?.name ?? "", i18n.settings_name_desc()),
				show(i18n.settings_email(), me?.email ?? "", i18n.settings_email_desc()),
				show(i18n.settings_role(), me?.role ?? ""),
			],
		},
		{
			key: "playback",
			title: i18n.settings_playback(),
			icon: CirclePlay,
			rows: [
				choice(
					i18n.settings_quality_home(),
					"qHome",
					QUALITIES,
					i18n.settings_quality_home_desc({ host }),
				),
				choice(i18n.settings_quality_away(), "qAway", QUALITIES, i18n.settings_quality_away_desc()),
				toggle(i18n.settings_autoplay(), "autoplay"),
				toggle(i18n.settings_auto_skip(), "autoSkip", i18n.settings_auto_skip_desc()),
			],
		},
		{
			key: "subs",
			title: i18n.settings_audio_subs(),
			icon: Captions,
			rows: [
				choice(i18n.settings_audio_language(), "audioLang", [
					"Original language",
					"English",
					"French",
				]),
				choice(
					i18n.settings_subtitles(),
					"subsMode",
					["Auto", "Always", "Off"],
					i18n.settings_subtitles_desc(),
				),
				toggle(i18n.settings_anime(), "animeRule", i18n.settings_anime_desc()),
				choice(i18n.settings_size(), "subSize", ["S", "M", "L", "XL"]),
				choice(i18n.settings_background(), "subBg", ["None", "Shadow", "Box"]),
				choice(i18n.settings_color(), "subColor", ["White", "Yellow"]),
			],
		},
		{
			key: "display",
			title: i18n.settings_display(),
			icon: Eye,
			rows: [
				toggle(i18n.settings_tech_details(), "techDetails", i18n.settings_tech_details_desc()),
			],
		},
		{
			key: "language",
			title: i18n.settings_language(),
			icon: Languages,
			rows: [
				choice(
					i18n.settings_app_language(),
					"language",
					["English", "Français"],
					i18n.settings_app_language_desc(),
				),
				show(
					i18n.settings_titles_synopses(),
					me?.library_language ?? "",
					i18n.settings_titles_synopses_desc(),
				),
			],
		},
		{
			key: "devices",
			title: i18n.settings_signed_in_devices(),
			icon: MonitorSmartphone,
			rows: [
				...(self
					? [
							show(
								self.name,
								i18n.settings_this_device(),
								`${self.platform} · ${i18n.settings_this_device_lower()}`,
							),
						]
					: []),
				// The TV only manages itself: other devices are signed out from the web app.
				...devices
					.filter((x) => !x.current)
					.map((x) => show(x.name, seen(x, true), `${x.platform} · ${seen(x)}`)),
			],
		},
		{
			key: "device",
			title: i18n.settings_this_device(),
			icon: Tv,
			rows: [
				show(i18n.settings_name(), info?.name ?? "—"),
				show(i18n.settings_platform(), info?.platform ?? "—"),
				show(i18n.settings_video_player(), info?.player ?? "—"),
				show(i18n.settings_device_key(), keystore, i18n.settings_device_key_desc()),
				show(
					i18n.settings_can_play(),
					caps ? i18n.settings_can_play_value({ count: playable(caps), total: PLAYABLE }) : "—",
					i18n.settings_can_play_desc({
						player: info?.player.split(" ")[0] ?? i18n.settings_the_player(),
					}),
				),
				choice(
					i18n.settings_frame_rate(),
					"frameRate",
					["Off", "On start", "Always"],
					i18n.settings_frame_rate_desc(),
				),
				choice(
					i18n.settings_eac3(),
					"passEAC3",
					["Pass through", "Decode here"],
					i18n.settings_eac3_desc(),
				),
				choice(
					i18n.settings_truehd(),
					"passTrueHD",
					["Pass through", "Decode here"],
					i18n.settings_truehd_desc(),
				),
				choice(
					i18n.settings_dts(),
					"passDTS",
					["Pass through", "Decode here"],
					i18n.settings_dts_desc(),
				),
				show(
					i18n.settings_hdr(),
					d?.hdr.join(" · ") || "—",
					d &&
						i18n.settings_hdr_desc({
							display: d.name,
							width: d.width,
							height: d.height,
							hz: Math.round(d.refresh_hz),
						}) + (d.hdr.includes("HDR10+") ? "" : i18n.settings_hdr10plus_as_hdr10()),
				),
			],
		},
		{
			key: "account",
			title: i18n.settings_account(),
			icon: User,
			rows: [
				show(i18n.settings_signed_in_as(), me?.email ?? ""),
				show(i18n.settings_server(), host),
				{
					...show(
						i18n.settings_sign_out(),
						i18n.settings_sign_out_value(),
						i18n.settings_sign_out_desc(),
					),
					act: signOut,
				},
			],
		},
	];
});

const section = $derived(sections.find((s) => s.key === current) ?? sections[0]!);
const picking = $derived(
	pick ? sections.flatMap((s) => s.rows).find((r) => r.choice === pick) : undefined,
);

const stage = () => document.querySelector<HTMLElement>("[data-tv-screen]");

async function open(key: ChoiceKey) {
	pick = key;
	await tick();
	const root = stage();
	if (root) focusDefault(root);
}

async function close() {
	const key = pick;
	pick = null;
	await tick();
	const root = stage();
	if (root) focus(root.querySelector<HTMLElement>(`[data-pick-row="${key}"]`), root);
}

function choose(value: string) {
	if (!pick) return;
	setPref(pick, value);
	if (pick.startsWith("pass")) refreshCaps();
	close();
}
</script>

<TvScreen tab={failed === null ? "settings" : undefined} scroll onback={pick ? close : undefined}>
	{#if failed !== null}
		<LoadFailed
			error={failed}
			retry={load}
			onloaded={() => (failed = null)}
			onback={() => history.back()}
		/>
	{:else}
		<div class="page">
			<div class="sections" data-tv-group="1">
				<h1>{i18n.settings_title()}</h1>
				{#each sections as s (s.key)}
					<button
						type="button"
						data-tv="1"
						data-tv-current={s.key === current ? "1" : undefined}
						class="section"
						class:current={s.key === current}
						onfocus={() => (current = s.key)}
						onclick={() => (current = s.key)}
					>
						<s.icon size={28} strokeWidth={1.75} />{s.title}
					</button>
				{/each}
			</div>
			{#if picking && pick}
				<div class="rows" data-tv-trap="1" role="dialog" aria-label={picking.label}>
					<div class="pick-head">
						<span class="pick-title">{picking.label}</span>
						{#if picking.desc}
							<span class="pick-desc">{picking.desc}</span>
						{/if}
					</div>
					{#each picking.vals ?? [] as v}
						<button
							type="button"
							data-tv="1"
							data-tv-default={prefs[pick] === v ? "1" : undefined}
							class="row"
							class:on={prefs[pick] === v}
							onclick={() => choose(v)}
						>
							<span class="opt">{shown(pick, v)}</span>
							{#if prefs[pick] === v}
								<Check size={30} strokeWidth={2.5} />
							{/if}
						</button>
					{/each}
				</div>
			{:else}
				<div class="rows" data-tv-group="1">
					{#each section.rows as r, i}
						<button
							type="button"
							data-tv="1"
							data-tv-current={i === 0 ? "1" : undefined}
							data-pick-row={r.choice}
							class="row"
							onclick={() => (r.choice ? open(r.choice) : r.act?.())}
						>
							<span class="text">
								<span class="label">{r.label}</span>
								{#if r.desc}
									<span class="desc">{r.desc}</span>
								{/if}
							</span>
							<span class="value">{r.value}</span>
							{#if r.choice}
								<span class="chevron"><ChevronRight size={26} strokeWidth={2} /></span>
							{/if}
						</button>
					{/each}
				</div>
			{/if}
		</div>
	{/if}
</TvScreen>

<style>
.page {
	padding: 156px 96px 80px 96px;
	display: grid;
	grid-template-columns: 420px minmax(0, 1fr);
	gap: 72px;
	align-items: start;
}

.sections {
	display: flex;
	flex-direction: column;
	gap: 10px;
}

h1 {
	margin: 0 0 24px;
	font-size: 58px;
	font-weight: 700;
	letter-spacing: -0.03em;
}

.section {
	height: 72px;
	display: flex;
	align-items: center;
	gap: 20px;
	padding: 0 24px;
	border: 0;
	border-radius: 18px;
	background: transparent;
	color: var(--fg);
	font-size: 26px;
	font-weight: 500;
	text-align: left;
	cursor: pointer;
	outline: none;
	transition: transform var(--dur-fast);
}

.section.current {
	background: rgba(255, 255, 255, 0.1);
}

.section:focus {
	background: var(--player-ink);
	color: var(--fg-on-accent);
	transform: scale(1.04);
}

.rows {
	display: flex;
	flex-direction: column;
	gap: 14px;
	padding-top: 96px;
}

.row {
	min-height: 92px;
	display: flex;
	align-items: center;
	gap: 24px;
	padding: 16px 30px;
	border: 0;
	border-radius: 18px;
	background: var(--surface);
	color: var(--fg);
	text-align: left;
	cursor: pointer;
	outline: none;
	transition: transform var(--dur-fast);
}

.row.on {
	color: var(--accent-text);
}

.row:focus {
	background: var(--player-ink);
	color: var(--fg-on-accent);
	transform: scale(1.03);
}

.text {
	flex: 1;
	min-width: 0;
	display: flex;
	flex-direction: column;
	gap: 4px;
}

.label,
.opt {
	font-size: 26px;
	font-weight: 500;
}

.opt {
	flex: 1;
	min-width: 0;
}

.desc {
	font-size: 20px;
	line-height: 1.4;
	opacity: 0.65;
}

.value {
	flex: none;
	font: 500 24px var(--font-mono);
	opacity: 0.85;
}

.chevron {
	flex: none;
	display: flex;
	margin: 0 -8px 0 -10px;
	opacity: 0.6;
}

.pick-head {
	display: flex;
	flex-direction: column;
	gap: 8px;
	padding: 0 30px 18px;
}

.pick-title {
	font-size: 36px;
	font-weight: 600;
	letter-spacing: -0.02em;
}

.pick-desc {
	max-width: 900px;
	font-size: 22px;
	line-height: 1.45;
	color: var(--fg-subtle);
	text-wrap: pretty;
}
</style>
