<script lang="ts">
	import LoadFailed from "#lib/tv/LoadFailed.svelte";
	import TvScreen from "#lib/tv/TvScreen.svelte";
	import { playerCapabilities } from "#lib/tv/api.ts";
	import { focus, focusDefault } from "#lib/tv/focus.ts";
	import { myLanguage, prefs, setPref, type ChoiceKey, type ToggleKey } from "#lib/tv/prefs.svelte.ts";
	import { invoke } from "@tauri-apps/api/core";
	import { Captions, Check, ChevronRight, CirclePlay, Eye, Languages, MonitorSmartphone, Tv, User } from "@lucide/svelte";
	import { tick, type Component } from "svelte";

	type Me = { name: string; email: string; role: string; library_language?: string };
	type Device = { id: string; name: string; platform: string; last_seen: string; streaming: boolean; current: boolean };
	type DeviceInfo = { name: string; model: string; platform: string; player: string; display: { name: string; width: number; height: number; refresh_hz: number; hdr: string[] } };
	type Caps = { video: { codec: string }[]; hdr: { formats: string[] }; audio: { codecs: string[]; passthrough: string[] } };
	type Row = { label: string; desc?: string; value: string; choice?: ChoiceKey; vals?: string[]; act?: () => void };
	type Section = { key: string; title: string; icon: Component; rows: Row[] };

	const QUALITIES = ["Auto", "Original", "20 Mbit/s", "8 Mbit/s", "4 Mbit/s", "2 Mbit/s", "1 Mbit/s"];
	const PLAYABLE = 12;
	const ACTIVE_SECS = 300;

	let me = $state.raw<Me | null>(null);
	let devices = $state.raw<Device[]>([]);
	let host = $state("");
	let info = $state.raw<DeviceInfo | null>(null);
	let caps = $state.raw<Caps | null>(null);
	let failed = $state<string | null>(null);
	let current = $state("profile");
	let pick = $state<ChoiceKey | null>(null);

	async function load() {
		[me, devices, host] = await Promise.all([invoke<Me>("me"), invoke<{ items: Device[] }>("devices").then((d) => d.items), invoke<string>("server_host")]);
		// Only a native player can answer these; the rows show a dash without them.
		invoke<DeviceInfo>("plugin:player|deviceInfo").then((i) => (info = i)).catch(() => {});
		refreshCaps();
	}
	load().catch((e) => (failed = String(e)));

	const refreshCaps = () => playerCapabilities().then((c) => (caps = c as Caps)).catch(() => {});

	function playable(c: Caps) {
		const v = new Set(c.video.map((x) => x.codec));
		const a = new Set([...c.audio.codecs, ...c.audio.passthrough]);
		const hdr = c.hdr.formats;
		return [v.has("h264"), v.has("hevc"), v.has("av1"), v.has("vp9"), hdr.includes("hdr10"), hdr.includes("dolby_vision"), ...["aac", "ac3", "eac3", "truehd", "dts", "flac"].map((x) => a.has(x))].filter(Boolean).length;
	}

	function seen(d: Device) {
		if (d.streaming) return "streaming now";
		const s = (Date.now() - Date.parse(d.last_seen)) / 1000;
		if (s < ACTIVE_SECS) return "active now";
		if (s < 3600) return `last seen ${Math.round(s / 60)} min ago`;
		if (s < 86400) return `last seen ${Math.round(s / 3600)} h ago`;
		const days = Math.round(s / 86400);
		return `last seen ${days} day${days === 1 ? "" : "s"} ago`;
	}

	const seenShort = (d: Device) => {
		const s = seen(d).replace(/^last seen /, "");
		return s.charAt(0).toUpperCase() + s.slice(1);
	};

	const choice = (label: string, key: ChoiceKey, vals: string[], desc?: string): Row => ({ label, desc, value: prefs[key], choice: key, vals });
	const toggle = (label: string, key: ToggleKey, desc?: string): Row => ({ label, desc, value: prefs[key] ? "On" : "Off", act: () => setPref(key, !prefs[key]) });
	const show = (label: string, value: string, desc?: string): Row => ({ label, desc, value });

	const sections = $derived.by((): Section[] => {
		const d = info?.display;
		const self = devices.find((x) => x.current);
		return [
			{ key: "profile", title: "Profile", icon: User, rows: [
				show("Name", me?.name ?? "", "Other people on this server see this name."),
				show("Email", me?.email ?? "", "Used to sign in, and for new-device approval emails."),
				show("Role", me?.role ?? ""),
			] },
			{ key: "playback", title: "Playback", icon: CirclePlay, rows: [
				choice("Quality at home", "qHome", QUALITIES, `On the same network as ${host}. Original plays the file as-is whenever this device can.`),
				choice("Quality away from home", "qAway", QUALITIES, "Auto picks what your connection can hold and steps down instead of buffering."),
				toggle("Play the next episode automatically", "autoplay"),
				toggle("Skip intros automatically", "autoSkip", "Uses the intro markers Streamline finds when it scans an episode."),
			] },
			{ key: "subs", title: "Audio & subtitles", icon: Captions, rows: [
				choice("Audio language", "audioLang", ["Original language", "English", "French"]),
				choice("Subtitles", "subsMode", ["Auto", "Always", "Off"], "Auto turns on forced subtitles when the audio isn’t in your language."),
				toggle("Anime in Japanese, with subtitles", "animeRule", `For anime, play the Japanese audio with full ${myLanguage()} subtitles, whatever Audio language and Subtitles say.`),
				choice("Size", "subSize", ["S", "M", "L", "XL"]),
				choice("Background", "subBg", ["None", "Shadow", "Box"]),
				choice("Color", "subColor", ["White", "Yellow"]),
			] },
			{ key: "display", title: "Display", icon: Eye, rows: [
				toggle("Show technical details", "techDetails", "Codecs, bitrates, file names, and whether a stream is copied or converted. Off keeps it to 4K, HDR and Atmos."),
			] },
			{ key: "language", title: "Language", icon: Languages, rows: [
				choice("App language", "language", ["English", "Français"], "Menus, buttons, and the emails the server sends you."),
				show("Titles and synopses", me?.library_language ?? "", "Comes with the library, the same for everyone on this server."),
			] },
			{ key: "devices", title: "Signed-in devices", icon: MonitorSmartphone, rows: [
				...(self ? [show(self.name, "This device", `${self.platform} · this device`)] : []),
				// The TV only manages itself: other devices are signed out from the web app.
				...devices.filter((x) => !x.current).map((x) => show(x.name, seenShort(x), `${x.platform} · ${seen(x)}`)),
			] },
			{ key: "device", title: "This device", icon: Tv, rows: [
				show("Name", info?.name ?? "—"),
				show("Platform", info?.platform ?? "—"),
				show("Video player", info?.player ?? "—"),
				show("Device key", "Android Keystore", "Created when you paired. It never leaves secure storage and is never shown to the app’s web layer."),
				show("What this device can play", caps ? `${playable(caps)} of ${PLAYABLE}` : "—", `Reported by ${info?.player.split(" ")[0] ?? "the player"}. Files in these formats play as-is; the server converts the rest.`),
				choice("Match frame rate", "frameRate", ["Off", "On start", "Always"], "Switches the TV to the video’s frame rate so motion doesn’t judder; the picture goes dark for a moment. On start switches when a video starts and back when it stops. Always also follows rate changes between episodes."),
				choice("E-AC-3 and Atmos output", "passEAC3", ["Pass through", "Decode here"], "Pass through sends the original audio to your TV or receiver, Atmos included. Decode here turns it into PCM on this device."),
				choice("TrueHD output", "passTrueHD", ["Pass through", "Decode here"], "This device can’t decode TrueHD itself: with Decode here, the server converts it to E-AC-3."),
				choice("DTS output", "passDTS", ["Pass through", "Decode here"], "Only pass DTS through if your receiver supports it. With Decode here, the server converts it to E-AC-3."),
				show(
					"HDR and Dolby Vision",
					d?.hdr.join(" · ") || "—",
					d && `What ${d.name} reports over HDMI · ${d.width} × ${d.height} · ${Math.round(d.refresh_hz)} Hz.${d.hdr.includes("HDR10+") ? "" : " HDR10+ plays as HDR10."}`,
				),
			] },
			{ key: "account", title: "Account", icon: User, rows: [
				show("Signed in as", me?.email ?? ""),
				show("Server", host),
				// ponytail: inert until pairing exists; then it removes the device key and opens pairing.
				show("Sign out of this device", "Sign out", "Removes the device key. You’ll need to pair again."),
			] },
		];
	});

	const section = $derived(sections.find((s) => s.key === current) ?? sections[0]!);
	const picking = $derived(pick ? sections.flatMap((s) => s.rows).find((r) => r.choice === pick) : undefined);

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
		<LoadFailed error={failed} retry={load} onloaded={() => (failed = null)} onback={() => history.back()} />
	{:else}
		<div class="page">
			<div class="sections" data-tv-group="1">
				<h1>Settings</h1>
				{#each sections as s (s.key)}
					<button data-tv="1" data-tv-current={s.key === current ? "1" : undefined} class="section" class:current={s.key === current} onfocus={() => (current = s.key)} onclick={() => (current = s.key)}>
						<s.icon size={28} strokeWidth={1.75} />{s.title}
					</button>
				{/each}
			</div>
			{#if picking && pick}
				<div class="rows" data-tv-trap="1" role="dialog" aria-label={picking.label}>
					<div class="pick-head">
						<span class="pick-title">{picking.label}</span>
						{#if picking.desc}<span class="pick-desc">{picking.desc}</span>{/if}
					</div>
					{#each picking.vals ?? [] as v}
						<button data-tv="1" data-tv-default={prefs[pick] === v ? "1" : undefined} class="row" class:on={prefs[pick] === v} onclick={() => choose(v)}>
							<span class="opt">{v}</span>
							{#if prefs[pick] === v}<Check size={30} strokeWidth={2.5} />{/if}
						</button>
					{/each}
				</div>
			{:else}
				<div class="rows" data-tv-group="1">
					{#each section.rows as r, i}
						<button data-tv="1" data-tv-current={i === 0 ? "1" : undefined} data-pick-row={r.choice} class="row" onclick={() => (r.choice ? open(r.choice) : r.act?.())}>
							<span class="text">
								<span class="label">{r.label}</span>
								{#if r.desc}<span class="desc">{r.desc}</span>{/if}
							</span>
							<span class="value">{r.value}</span>
							{#if r.choice}<span class="chevron"><ChevronRight size={26} strokeWidth={2} /></span>{/if}
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
