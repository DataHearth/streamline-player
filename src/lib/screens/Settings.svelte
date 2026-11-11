<script lang="ts">
import {
	changed,
	type Draft,
	type FieldError,
	PICTURE_TYPES,
	pictureProblem,
	refusal,
	validate,
} from "#lib/core/account.ts";
import { browserCaps, type Caps, chips } from "#lib/core/capabilities.ts";
import { seen } from "#lib/core/devices.ts";
import { lib, loadLibrary } from "#lib/core/library.svelte.ts";
import { account, initialsOf, setMe } from "#lib/core/me.svelte.ts";
import { type ChoiceKey, prefs, setPref, type ToggleKey } from "#lib/core/prefs.svelte.ts";
import { QUALITIES, shown } from "#lib/core/prefs-labels.ts";
import { toast } from "#lib/core/toast.svelte.ts";
import type { Device, DeviceInfo } from "#lib/core/types.ts";
import { m as i18n } from "#lib/paraglide/messages.js";
import { IS_WEB } from "#lib/platform/target.ts";
import Art from "#lib/ui/Art.svelte";
import Avatar from "#lib/ui/Avatar.svelte";
import Button from "#lib/ui/Button.svelte";
import ChoiceMenu from "#lib/ui/ChoiceMenu.svelte";
import Dialog from "#lib/ui/Dialog.svelte";
import Icon, { type IconName } from "#lib/ui/Icon.svelte";
import { goto } from "#lib/ui/nav.ts";
import { presence } from "#lib/ui/presence.svelte.ts";
import { useScroll } from "#lib/ui/scroll.ts";
import SettingsList from "#lib/ui/SettingsList.svelte";
import SettingsRow from "#lib/ui/SettingsRow.svelte";
import Switch from "#lib/ui/Switch.svelte";
import TextField from "#lib/ui/TextField.svelte";
import {
	artwork,
	authState,
	deviceInfo,
	devices as listDevices,
	home,
	library,
	me as whoAmI,
	playerCapabilities,
	removePicture,
	revokeDevice,
	serverHost,
	setPicture,
	signOut as signOutNow,
	title as fetchTitle,
	updateMe,
	watchState,
	myPicture,
} from "#platform";
import { onMount } from "svelte";

const APP_VERSION = import.meta.env.VITE_APP_VERSION;
const PAD_X = 56;
const TOP = 92;
const PING_MS = 5000;
const SPY_OFFSET = 120;
const CUE_SIZE = { S: 15, M: 19, L: 23, XL: 28 };

const me = $derived(account.me);
const getScroll = useScroll();

let devices = $state.raw<Device[]>([]);
let host = $state("");
let keystore = $state("");
let info = $state.raw<DeviceInfo | null>(null);
let caps = $state.raw<Caps | null>(null);
let active = $state("profile");
let ping = $state(0);
let beat = $state(0);
let draft = $state<Draft>({ name: "", email: "" });
let error = $state<FieldError | null>(null);
let saving = $state(false);
let leaving = $state<Device | null>(null);
let picker = $state<HTMLInputElement>();

const confirm = presence(() => leaving !== null, 240);
const pending = $derived(me ? changed(me, draft) : {});
const dirty = $derived(Object.keys(pending).length > 0);

let seeded = false;
$effect(() => {
	if (me && !seeded) {
		seeded = true;
		draft = { name: me.name, email: me.email };
	}
});

const wording = (e: FieldError) =>
	({
		empty_name: i18n.settings_err_name(),
		invalid_email: i18n.settings_err_email_invalid(),
		email_taken: i18n.settings_err_email_taken(),
		too_large: i18n.settings_err_picture_large(),
		unsupported_type: i18n.settings_err_picture_type(),
		unknown: i18n.settings_err_unknown(),
	})[e.message] ?? e.message;
const errorOf = (field: FieldError["field"]) =>
	error?.field === field ? wording(error) : undefined;

async function save() {
	if (!me || saving) return;
	const invalid = validate(draft);
	if (invalid) {
		error = invalid;
		return;
	}
	saving = true;
	try {
		const next = await updateMe(pending);
		setMe(next);
		draft = { name: next.name, email: next.email };
		error = null;
		toast(i18n.settings_saved());
	} catch (e) {
		error = refusal(String(e));
	} finally {
		saving = false;
	}
}

function cancel() {
	if (!me) return;
	draft = { name: me.name, email: me.email };
	error = null;
}

async function choosePicture(file: File | undefined) {
	if (!file) return;
	const problem = pictureProblem(file);
	if (problem) {
		error = { field: "picture", message: problem };
		return;
	}
	try {
		await setPicture(file);
		setMe(await whoAmI(), true);
		error = null;
		toast(i18n.settings_saved());
	} catch (e) {
		error = refusal(String(e));
	}
}

async function dropPicture() {
	try {
		await removePicture();
		setMe(await whoAmI(), true);
		error = null;
	} catch (e) {
		error = refusal(String(e));
	}
}

async function confirmSignOut() {
	const d = leaving;
	if (!d) return;
	leaving = null;
	try {
		await revokeDevice(d.id);
		devices = devices.filter((x) => x.id !== d.id);
		toast(i18n.settings_signed_out_device({ name: d.name }));
	} catch {
		toast(i18n.settings_err_unknown(), { icon: "wifi", warning: true });
	}
}

async function leave() {
	await signOutNow();
	toast(IS_WEB ? i18n.shell_signed_out() : i18n.shell_signed_out_key_removed());
	await goto(IS_WEB ? "/signin" : "/welcome", { replace: true });
}

const pingOnce = async () => {
	const t = Date.now();
	try {
		await whoAmI();
		ping = Math.max(1, Date.now() - t);
		beat++;
	} catch {}
};

const SECTIONS: { key: string; icon: IconName; group?: () => string }[] = [
	{ key: "profile", icon: "user" },
	{ key: "playback", icon: "circle-play", group: i18n.settings_group_watching },
	{ key: "subs", icon: "captions", group: i18n.settings_group_watching },
	{ key: "display", icon: "eye", group: i18n.settings_group_app },
	{ key: "language", icon: "languages", group: i18n.settings_group_app },
	{ key: "devices", icon: "monitor-smartphone", group: i18n.settings_account },
	{ key: "device", icon: IS_WEB ? "globe" : "monitor", group: i18n.settings_account },
	{ key: "account", icon: "user", group: i18n.settings_account },
];
const TITLES: Record<string, () => string> = {
	profile: i18n.settings_profile,
	playback: i18n.settings_playback,
	subs: i18n.settings_audio_subs,
	display: i18n.settings_display,
	language: i18n.settings_language,
	devices: i18n.settings_signed_in_devices,
	device: IS_WEB ? i18n.settings_this_browser : i18n.settings_this_device,
	account: i18n.settings_account,
};
const navGroups = $derived(
	SECTIONS.reduce<{ name: string; items: typeof SECTIONS }[]>((out, s) => {
		const name = s.group?.() ?? "";
		const g = out.find((x) => x.name === name);
		if (g) g.items.push(s);
		else out.push({ name, items: [s] });
		return out;
	}, []),
);

function jump(key: string) {
	active = key;
	spyHeldUntil = Date.now() + 800;
	document
		.querySelector(`[data-sec="${key}"]`)
		?.scrollIntoView({ behavior: "smooth", block: "start" });
}

let spyHeldUntil = 0;
function spy() {
	const box = getScroll?.();
	if (!box || Date.now() < spyHeldUntil) return;
	const els = [...box.querySelectorAll<HTMLElement>("[data-sec]")];
	if (!els.length) return;
	const limit = box.getBoundingClientRect().top + SPY_OFFSET;
	let current = els[0]!.dataset.sec!;
	for (const el of els) if (el.getBoundingClientRect().top <= limit) current = el.dataset.sec!;
	if (box.scrollTop + box.clientHeight >= box.scrollHeight - 4) current = els.at(-1)!.dataset.sec!;
	active = current;
}

const choice = (key: ChoiceKey, vals: string[]) =>
	vals.map((v) => ({ value: v, label: shown(key, v) }));

const selfDevice = $derived(devices.find((d) => d.current));
const others = $derived(devices.filter((d) => !d.current));
const deviceIcon = (d: Device): IconName =>
	/browser/i.test(d.platform)
		? "globe"
		: /tv/i.test(d.platform)
			? "tv"
			: /ipad|tablet/i.test(d.platform)
				? "tablet"
				: /android|ios|iphone/i.test(d.platform)
					? "smartphone"
					: "monitor";

const chipList = $derived(caps ? chips(caps) : []);
const groups = $derived(
	(["video", "audio", "subs"] as const)
		.map((g) => ({
			key: g,
			label: {
				video: i18n.settings_caps_video,
				audio: i18n.settings_caps_audio,
				subs: i18n.settings_caps_subs,
			}[g](),
			items: chipList.filter((c) => c.group === g),
		}))
		.filter((g) => g.items.length),
);

const preview = $derived(
	lib.shows[0]
		? { kind: "series" as const, id: lib.shows[0].id }
		: lib.movies[0]
			? { kind: "movies" as const, id: lib.movies[0].id }
			: null,
);
const cueColor = $derived(
	prefs.subColor === "Yellow"
		? "color-mix(in srgb, var(--status-wanted) 40%, var(--fg))"
		: "var(--fg)",
);
const cueShadow = $derived(
	prefs.subBg === "Shadow"
		? "0 2px 4px rgba(0,0,0,0.9), 0 0 10px rgba(0,0,0,0.6)"
		: prefs.subBg === "None"
			? "0 0 2px rgba(0,0,0,0.9)"
			: "none",
);

const toggle = (key: ToggleKey) => (on: boolean) => setPref(key, on);

onMount(() => {
	if (!account.me)
		whoAmI().then(
			(v) => setMe(v),
			() => {},
		);
	listDevices().then(
		(d) => (devices = d),
		() => {},
	);
	serverHost().then((h) => (host = h));
	authState().then((a) => {
		keystore = a.store_error ? i18n.common_key_not_stored({ error: a.store_error }) : a.keystore;
	});
	deviceInfo().then(
		(i) => (info = i),
		() => {},
	);
	playerCapabilities().then(
		(c) => (caps = c as Caps),
		() => {
			if (IS_WEB) caps = browserCaps();
		},
	);
	loadLibrary({ library, watchState, home, title: fetchTitle });
	pingOnce();
	const timer = setInterval(pingOnce, PING_MS);
	const box = getScroll?.();
	box?.addEventListener("scroll", spy, { passive: true });
	return () => {
		clearInterval(timer);
		box?.removeEventListener("scroll", spy);
	};
});
</script>

<div class="settings" style:padding="{TOP}px {PAD_X}px 64px">
	<nav aria-label={i18n.settings_nav()} style:top="{TOP}px">
		{#each navGroups as g (g.name)}
			<div class="group">
				{#if g.name}
					<span class="group-name">{g.name}</span>
				{/if}
				{#each g.items as s (s.key)}
					<button type="button" class:on={active === s.key} onclick={() => jump(s.key)}>
						<Icon name={s.icon} />
						<span>{TITLES[s.key]!()}</span>
					</button>
				{/each}
			</div>
		{/each}
	</nav>

	<div class="main">
		<header>
			<Avatar
				name={me?.name}
				email={me?.email}
				initials={me ? initialsOf(me.name) : ""}
				size={48}
				color="var(--kind-album)"
				picture={me?.picture ? myPicture(account.version) : undefined}
			/>
			<span class="who">
				<h1>{i18n.settings_title()}</h1>
				<span class="mono">{me?.name} · {me?.email} · {me?.role}</span>
			</span>
			{#if ping}
				<span class="ping">
					{#key beat}
						<span class="dot"></span>
					{/key}
					{i18n.settings_ping({ host, ms: ping })}
				</span>
			{/if}
		</header>

		<SettingsList id="profile" title={TITLES.profile!()}>
			<SettingsRow label={i18n.settings_picture()} desc={i18n.settings_picture_desc()}>
				{#snippet control()}
					<span class="picture">
						<Avatar
							name={me?.name}
							email={me?.email}
							initials={me ? initialsOf(me.name) : ""}
							color="var(--kind-album)"
							picture={me?.picture ? myPicture(account.version) : undefined}
						/>
						<input
							bind:this={picker}
							type="file"
							hidden
							accept={PICTURE_TYPES.join(",")}
							onchange={(e) => {
								choosePicture(e.currentTarget.files?.[0]);
								e.currentTarget.value = "";
							}}
						>
						<Button variant="secondary" size="sm" onclick={() => picker?.click()}>
							{i18n.settings_picture_choose()}
						</Button>
						{#if me?.picture}
							<Button variant="ghost" size="sm" onclick={dropPicture}>
								{i18n.settings_picture_remove()}
							</Button>
						{/if}
					</span>
				{/snippet}
				{#snippet below()}
					{#if errorOf("picture")}
						<span class="error" role="alert">{errorOf("picture")}</span>
					{/if}
				{/snippet}
			</SettingsRow>
			<SettingsRow label={i18n.settings_name()} desc={i18n.settings_name_desc()}>
				{#snippet control()}
					<TextField
						label={i18n.settings_name()}
						value={draft.name}
						error={errorOf("name")}
						oninput={(v) => (draft.name = v)}
						onenter={save}
					/>
				{/snippet}
			</SettingsRow>
			<SettingsRow label={i18n.settings_email()} desc={i18n.settings_email_desc()}>
				{#snippet control()}
					<TextField
						label={i18n.settings_email()}
						type="email"
						value={draft.email}
						error={errorOf("email")}
						oninput={(v) => (draft.email = v)}
						onenter={save}
					/>
				{/snippet}
			</SettingsRow>
			{#if dirty}
				<div class="actions">
					<Button variant="ghost" onclick={cancel}>{i18n.settings_cancel()}</Button>
					<Button onclick={save} disabled={saving}>{i18n.settings_save()}</Button>
				</div>
			{/if}
			<SettingsRow
				label={i18n.settings_role()}
				desc={me?.role === "Admin" ? i18n.settings_role_desc_admin() : undefined}
			>
				{#snippet control()}
					<span class="info">{me?.role}</span>
				{/snippet}
			</SettingsRow>
		</SettingsList>

		<SettingsList id="playback" title={TITLES.playback!()}>
			<SettingsRow
				label={i18n.settings_quality_home()}
				desc={i18n.settings_quality_home_desc({ host })}
			>
				{#snippet control()}
					<ChoiceMenu
						select
						label={i18n.settings_quality_home()}
						options={choice("qHome", QUALITIES)}
						value={prefs.qHome}
						onchange={(v) => setPref("qHome", v)}
					/>
				{/snippet}
			</SettingsRow>
			<SettingsRow label={i18n.settings_quality_away()} desc={i18n.settings_quality_away_desc()}>
				{#snippet control()}
					<ChoiceMenu
						select
						label={i18n.settings_quality_away()}
						options={choice("qAway", QUALITIES)}
						value={prefs.qAway}
						onchange={(v) => setPref("qAway", v)}
					/>
				{/snippet}
			</SettingsRow>
			<SettingsRow label={i18n.settings_autoplay()}>
				{#snippet control()}
					<Switch
						label={i18n.settings_autoplay()}
						checked={prefs.autoplay}
						onchange={toggle("autoplay")}
					/>
				{/snippet}
			</SettingsRow>
			<SettingsRow label={i18n.settings_auto_skip()} desc={i18n.settings_auto_skip_desc()}>
				{#snippet control()}
					<Switch
						label={i18n.settings_auto_skip()}
						checked={prefs.autoSkip}
						onchange={toggle("autoSkip")}
					/>
				{/snippet}
			</SettingsRow>
		</SettingsList>

		<SettingsList id="subs" title={TITLES.subs!()}>
			<SettingsRow label={i18n.settings_audio_language()}>
				{#snippet control()}
					<ChoiceMenu
						label={i18n.settings_audio_language()}
						options={choice("audioLang", ["Original language", "English", "French"])}
						value={prefs.audioLang}
						onchange={(v) => setPref("audioLang", v)}
					/>
				{/snippet}
			</SettingsRow>
			<SettingsRow label={i18n.settings_subtitles()} desc={i18n.settings_subtitles_desc()}>
				{#snippet control()}
					<ChoiceMenu
						label={i18n.settings_subtitles()}
						options={choice("subsMode", ["Auto", "Always", "Off"])}
						value={prefs.subsMode}
						onchange={(v) => setPref("subsMode", v)}
					/>
				{/snippet}
			</SettingsRow>
			<SettingsRow label={i18n.settings_anime()} desc={i18n.settings_anime_desc()}>
				{#snippet control()}
					<Switch
						label={i18n.settings_anime()}
						checked={prefs.animeRule}
						onchange={toggle("animeRule")}
					/>
				{/snippet}
			</SettingsRow>
			<SettingsRow label={i18n.settings_size()}>
				{#snippet control()}
					<ChoiceMenu
						label={i18n.settings_size()}
						options={choice("subSize", ["S", "M", "L", "XL"])}
						value={prefs.subSize}
						onchange={(v) => setPref("subSize", v)}
					/>
				{/snippet}
			</SettingsRow>
			<SettingsRow label={i18n.settings_background()}>
				{#snippet control()}
					<ChoiceMenu
						label={i18n.settings_background()}
						options={choice("subBg", ["None", "Shadow", "Box"])}
						value={prefs.subBg}
						onchange={(v) => setPref("subBg", v)}
					/>
				{/snippet}
			</SettingsRow>
			<SettingsRow label={i18n.settings_color()}>
				{#snippet control()}
					<ChoiceMenu
						label={i18n.settings_color()}
						options={choice("subColor", ["White", "Yellow"])}
						value={prefs.subColor}
						onchange={(v) => setPref("subColor", v)}
					/>
				{/snippet}
			</SettingsRow>
			<SettingsRow label={i18n.settings_preview()}>
				{#snippet below()}
					<div class="preview">
						{#if preview}
							<Art src={artwork(preview.kind, preview.id, "backdrop")} />
						{/if}
						<div class="cue-line">
							<span
								class="cue"
								data-preview
								style:font-size="{CUE_SIZE[prefs.subSize as keyof typeof CUE_SIZE] ?? 19}px"
								style:color={cueColor}
								style:text-shadow={cueShadow}
								style:background={prefs.subBg === "Box" ? "rgba(0,0,0,0.72)" : "transparent"}
								style:padding={prefs.subBg === "Box" ? "3px 10px" : "0"}
								>{i18n.settings_preview_cue()}</span
							>
						</div>
					</div>
				{/snippet}
			</SettingsRow>
		</SettingsList>

		<SettingsList id="display" title={TITLES.display!()}>
			<SettingsRow label={i18n.settings_tech_details()} desc={i18n.settings_tech_details_desc()}>
				{#snippet control()}
					<Switch
						label={i18n.settings_tech_details()}
						checked={prefs.techDetails}
						onchange={toggle("techDetails")}
					/>
				{/snippet}
			</SettingsRow>
			<SettingsRow label={i18n.settings_shortcuts()}>
				{#snippet control()}
					<span class="info">{i18n.settings_shortcuts_value()}</span>
				{/snippet}
			</SettingsRow>
		</SettingsList>

		<SettingsList id="language" title={TITLES.language!()}>
			<SettingsRow
				label={i18n.settings_app_language()}
				desc={i18n.settings_app_language_desc_emails()}
			>
				{#snippet control()}
					<ChoiceMenu
						label={i18n.settings_app_language()}
						options={choice("language", ["English", "Français"])}
						value={prefs.language}
						onchange={(v) => setPref("language", v)}
					/>
				{/snippet}
			</SettingsRow>
			<SettingsRow
				label={i18n.settings_titles_synopses()}
				desc={i18n.settings_titles_synopses_desc()}
			>
				{#snippet control()}
					<span class="info">{me?.library_language}</span>
				{/snippet}
			</SettingsRow>
		</SettingsList>

		<SettingsList id="devices" title={TITLES.devices!()}>
			{#if selfDevice}
				<SettingsRow
					label={selfDevice.name}
					desc="{selfDevice.platform} · {i18n.settings_this_device_lower()}"
					icon={deviceIcon(selfDevice)}
				>
					{#snippet control()}
						<span class="tag"
							>{IS_WEB ? i18n.settings_this_browser() : i18n.settings_this_device()}</span
						>
					{/snippet}
				</SettingsRow>
			{/if}
			{#each others as d (d.id)}
				<SettingsRow label={d.name} desc="{d.platform} · {seen(d)}" icon={deviceIcon(d)}>
					{#snippet control()}
						<Button variant="danger-outline" onclick={() => (leaving = d)}>
							{i18n.shell_sign_out()}
						</Button>
					{/snippet}
				</SettingsRow>
			{/each}
		</SettingsList>

		<SettingsList id="device" title={TITLES.device!()}>
			{#if IS_WEB}
				<SettingsRow label={i18n.settings_browser()}>
					{#snippet control()}
						<span class="info">{info?.name ?? "—"}</span>
					{/snippet}
				</SettingsRow>
			{:else}
				<SettingsRow label={i18n.settings_name()}>
					{#snippet control()}
						<span class="info">{info?.name ?? "—"}</span>
					{/snippet}
				</SettingsRow>
				<SettingsRow label={i18n.settings_platform()}>
					{#snippet control()}
						<span class="info">{info?.platform ?? "—"}</span>
					{/snippet}
				</SettingsRow>
			{/if}
			{#if IS_WEB}
				<SettingsRow label={i18n.settings_session()} desc={i18n.settings_session_desc()}>
					{#snippet control()}
						<span class="info">{i18n.settings_session_value()}</span>
					{/snippet}
				</SettingsRow>
			{/if}
			<SettingsRow
				label={i18n.settings_video_player()}
				desc={IS_WEB ? undefined : i18n.settings_video_player_desc()}
			>
				{#snippet control()}
					<span class="info">{info?.player ?? "—"}</span>
				{/snippet}
			</SettingsRow>
			{#if !IS_WEB}
				<SettingsRow label={i18n.settings_device_key()} desc={i18n.settings_device_key_desc()}>
					{#snippet control()}
						<span class="info">{keystore}</span>
					{/snippet}
				</SettingsRow>
			{/if}
			<SettingsRow
				label={IS_WEB ? i18n.settings_can_play_browser() : i18n.settings_can_play()}
				desc={i18n.settings_can_play_desc({
					player: info?.player.split(" ")[0] ?? i18n.settings_the_player(),
				})}
			>
				{#snippet below()}
					<div class="caps">
						{#each groups as g (g.key)}
							<div class="cap-group">
								<span class="cap-label">{g.label}</span>
								<div class="cap-items">
									{#each g.items as c (c.name)}
										<span class="cap" class:yes={c.ok}>
											<Icon name={c.ok ? "tick" : "x"} />{c.name}
										</span>
									{/each}
								</div>
							</div>
						{/each}
					</div>
				{/snippet}
			</SettingsRow>
		</SettingsList>

		<SettingsList id="account" title={TITLES.account!()}>
			<SettingsRow label={i18n.settings_signed_in_as()}>
				{#snippet control()}
					<span class="info">{me?.email}</span>
				{/snippet}
			</SettingsRow>
			<SettingsRow label={i18n.settings_server()}>
				{#snippet control()}
					<span class="info">{host}</span>
				{/snippet}
			</SettingsRow>
			<SettingsRow
				label={IS_WEB ? i18n.shell_sign_out() : i18n.settings_sign_out()}
				desc={IS_WEB ? i18n.settings_sign_out_browser_desc() : i18n.settings_sign_out_desc()}
			>
				{#snippet control()}
					<Button variant="danger-outline" onclick={leave}>{i18n.settings_sign_out_value()}</Button>
				{/snippet}
			</SettingsRow>
		</SettingsList>

		<footer>
			{i18n.settings_footer({ version: APP_VERSION, platform: info?.platform ?? "" })}
		</footer>
	</div>
</div>

{#if confirm.mounted && leaving}
	<Dialog
		title={i18n.settings_sign_out_title({ name: leaving.name })}
		closeLabel={i18n.shell_close()}
		shown={confirm.shown}
		onclose={() => (leaving = null)}
	>
		<div class="ask">
			<p>{i18n.settings_sign_out_body()}</p>
			<div class="ask-actions">
				<Button variant="ghost" onclick={() => (leaving = null)}>{i18n.settings_cancel()}</Button>
				<Button variant="danger-outline" onclick={confirmSignOut}
					>{i18n.settings_sign_out_value()}</Button
				>
			</div>
		</div>
	</Dialog>
{/if}

<style>
.settings {
	box-sizing: border-box;
	min-height: 100%;
	display: grid;
	grid-template-columns: 220px minmax(0, 800px);
	justify-content: start;
	gap: 48px;
	background: var(--bg-deep);
	color: var(--fg);
}

nav {
	position: sticky;
	align-self: start;
	display: flex;
	flex-direction: column;
	gap: 16px;
}

.group {
	display: flex;
	flex-direction: column;
	gap: 1px;
}

.group-name {
	padding: 0 10px 5px;
	font: 500 10px var(--font-mono);
	letter-spacing: 0.14em;
	text-transform: uppercase;
	color: var(--fg-faint);
}

nav button {
	flex: none;
	height: 32px;
	display: flex;
	align-items: center;
	gap: 10px;
	padding: 0 10px;
	border: 0;
	border-radius: 8px;
	background: transparent;
	color: var(--fg-muted);
	font-size: 13.5px;
	font-weight: 500;
	text-align: left;
	cursor: pointer;
}

nav button:hover {
	background: var(--surface);
	color: var(--fg);
}

nav button.on {
	background: var(--accent-soft);
	color: var(--accent-text);
}

nav button :global(.sl-ico) {
	flex: none;
	font-size: 16px;
}

nav button span {
	flex: 1;
	min-width: 0;
	white-space: nowrap;
	overflow: hidden;
	text-overflow: ellipsis;
}

.main {
	display: flex;
	flex-direction: column;
	gap: 28px;
	min-width: 0;
}

header {
	display: flex;
	align-items: center;
	gap: 18px;
	flex-wrap: wrap;
}

.who {
	flex: 1;
	min-width: 220px;
	display: flex;
	flex-direction: column;
	gap: 3px;
}

h1 {
	margin: 0;
	font-size: 30px;
	font-weight: 700;
	letter-spacing: -0.025em;
}

.mono {
	font: 400 12px var(--font-mono);
	color: var(--fg-subtle);
}

.ping {
	display: flex;
	align-items: center;
	gap: 8px;
	padding: 6px 10px;
	border-radius: var(--radius-full);
	background: color-mix(in srgb, var(--status-available) 10%, transparent);
	border: 1px solid color-mix(in srgb, var(--status-available) 25%, transparent);
	font: 500 11.5px var(--font-mono);
	font-variant-numeric: tabular-nums;
	color: var(--status-available);
	white-space: nowrap;
}

.dot {
	width: 6px;
	height: 6px;
	border-radius: 50%;
	background: var(--status-available);
	animation: beat 900ms var(--ease);
}

@keyframes beat {
	from {
		box-shadow: 0 0 0 0 color-mix(in srgb, var(--status-available) 60%, transparent);
	}
	to {
		box-shadow: 0 0 0 7px transparent;
	}
}

.picture {
	flex: none;
	display: flex;
	align-items: center;
	gap: 10px;
}

.actions {
	display: flex;
	justify-content: flex-end;
	gap: 8px;
	padding: 12px 20px;
	border-bottom: 1px solid var(--border);
}

.error {
	font-size: 12px;
	color: var(--status-failed);
}

.info {
	flex: none;
	font: 400 12.5px var(--font-mono);
	color: var(--fg-muted);
	text-align: right;
}

.tag {
	flex: none;
	height: 22px;
	display: inline-flex;
	align-items: center;
	padding: 0 8px;
	border-radius: var(--radius-sm);
	background: color-mix(in srgb, var(--accent) 12%, transparent);
	border: 1px solid color-mix(in srgb, var(--accent) 32%, transparent);
	color: var(--accent-text);
	font: 500 11px var(--font-mono);
}

.preview {
	position: relative;
	height: 150px;
	border-radius: 12px;
	overflow: hidden;
	background: var(--bg-deep);
}

.preview :global(.art) {
	opacity: 0.85;
}

.cue-line {
	position: absolute;
	left: 0;
	right: 0;
	bottom: 16px;
	display: flex;
	justify-content: center;
}

.cue {
	font-weight: 500;
	border-radius: 5px;
}

.caps {
	display: flex;
	flex-direction: column;
	gap: 10px;
}

.cap-group {
	display: flex;
	align-items: flex-start;
	gap: 12px;
}

.cap-label {
	width: 52px;
	padding-top: 5px;
	font: 500 10.5px var(--font-mono);
	letter-spacing: 0.1em;
	text-transform: uppercase;
	color: var(--fg-faint);
}

.cap-items {
	flex: 1;
	display: flex;
	flex-wrap: wrap;
	gap: 6px;
}

.cap {
	display: inline-flex;
	align-items: center;
	gap: 6px;
	height: 26px;
	padding: 0 9px;
	border: 1px solid var(--border);
	border-radius: 7px;
	background: color-mix(in srgb, var(--fg) 3%, transparent);
	color: var(--fg-subtle);
	font: 500 11.5px var(--font-mono);
}

.cap.yes {
	color: var(--status-available);
	background: color-mix(in srgb, var(--status-available) 8%, transparent);
	border-color: color-mix(in srgb, var(--status-available) 25%, transparent);
}

.cap :global(.sl-ico) {
	font-size: 12px;
	stroke-width: 2.5;
}

footer {
	padding-top: 8px;
	text-align: center;
	font: 400 11.5px var(--font-mono);
	color: var(--fg-faint);
}

.ask {
	display: flex;
	flex-direction: column;
	gap: 16px;
	max-width: 380px;
}

.ask p {
	margin: 0;
	font-size: 14px;
	line-height: 1.5;
	color: var(--fg-muted);
}

.ask-actions {
	display: flex;
	justify-content: flex-end;
	gap: 8px;
}
</style>
