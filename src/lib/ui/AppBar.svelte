<script lang="ts">
import Avatar from "#lib/ui/Avatar.svelte";
import Button from "#lib/ui/Button.svelte";
import Icon from "#lib/ui/Icon.svelte";
import Kbd from "#lib/ui/Kbd.svelte";
import Mark from "#lib/ui/Mark.svelte";
import "#lib/ui/ui.css";

type Tab = { id: string; label: string; on: boolean };

let {
	tabs,
	padLeft,
	padRight,
	scrolled,
	searchLabel,
	searchKbd,
	activityLabel,
	downloadsLabel,
	showDownloads,
	account,
	accountLabel,
	menuOpen,
	onhome,
	ontab,
	onsearch,
	onactivity,
	ondownloads,
	onmenu,
}: {
	tabs: Tab[];
	padLeft: number;
	padRight: number;
	scrolled: boolean;
	searchLabel: string;
	searchKbd: string;
	activityLabel: string;
	downloadsLabel: string;
	showDownloads: boolean;
	account: { name: string; email: string; initials: string; color: string; picture?: string };
	accountLabel: string;
	menuOpen: boolean;
	onhome: () => void;
	ontab: (id: string) => void;
	onsearch: () => void;
	onactivity: () => void;
	ondownloads: () => void;
	onmenu: () => void;
} = $props();
</script>

<div class="bar" class:scrolled data-tauri-drag-region style:padding="0 {padRight}px 0 {padLeft}px">
	<button type="button" class="brand" onclick={onhome}>
		<Mark size={30} />
		<span class="name"><span>streamline</span><span class="sub">player</span></span>
	</button>
	<nav>
		{#each tabs as t (t.id)}
			<button type="button" class="tab" class:on={t.on} onclick={() => ontab(t.id)}>
				{t.label}
				<span class="line"></span>
			</button>
		{/each}
	</nav>
	<div class="grow"></div>
	<div class="actions">
		<button type="button" class="sl-searchbtn" onclick={onsearch}>
			<Icon name="search" />
			<span class="label">{searchLabel}</span>
			<Kbd>{searchKbd}</Kbd>
		</button>
		<Button
			variant="icon"
			icon="bell"
			label={activityLabel}
			title={activityLabel}
			onclick={onactivity}
		/>
		{#if showDownloads}
			<Button
				variant="icon"
				icon="download"
				label={downloadsLabel}
				title={downloadsLabel}
				onclick={ondownloads}
			/>
		{/if}
		<button
			type="button"
			class="account"
			aria-label={accountLabel}
			aria-haspopup="menu"
			aria-expanded={menuOpen}
			onclick={onmenu}
		>
			<Avatar {...account} size={32} ring={menuOpen} />
		</button>
	</div>
</div>

<style>
.bar {
	position: absolute;
	left: 0;
	right: 0;
	top: 0;
	height: 64px;
	z-index: 20;
	display: flex;
	align-items: center;
	gap: 30px;
	background: linear-gradient(
		180deg,
		color-mix(in srgb, var(--bg-deep) 72%, transparent) 0%,
		transparent 100%
	);
	transition:
		background var(--dur-base),
		box-shadow var(--dur-base);
}

.bar.scrolled {
	background: color-mix(in srgb, var(--bg-deep) 78%, transparent);
	backdrop-filter: blur(16px) saturate(150%);
	box-shadow: inset 0 -1px 0 var(--border);
}

.brand {
	display: flex;
	align-items: center;
	gap: 10px;
	border: 0;
	background: transparent;
	padding: 0;
	cursor: pointer;
}

.name {
	display: flex;
	align-items: baseline;
	gap: 5px;
	font-size: 16px;
	font-weight: 600;
	letter-spacing: -0.02em;
	color: var(--fg);
	white-space: nowrap;
}

.sub {
	font-weight: 400;
	color: var(--fg-subtle);
}

nav {
	display: flex;
	align-items: center;
	gap: 4px;
	height: 100%;
}

.tab {
	position: relative;
	height: 100%;
	padding: 0 12px;
	border: 0;
	background: transparent;
	cursor: pointer;
	font-size: 14px;
	font-weight: 500;
	color: var(--fg-muted);
}

.tab:hover {
	color: var(--fg);
}

.tab.on {
	color: var(--fg);
	font-weight: 600;
}

.line {
	position: absolute;
	left: 12px;
	right: 12px;
	bottom: 14px;
	height: 2px;
	border-radius: 2px;
	background: var(--accent);
	opacity: 0;
}

.on .line {
	opacity: 1;
}

.grow {
	flex: 1;
}

.actions {
	display: flex;
	align-items: center;
	gap: 6px;
}

.sl-searchbtn {
	display: flex;
	align-items: center;
	flex: none;
	min-width: 210px;
	height: 38px;
	padding: 0 12px;
	gap: 8px;
	border: 1px solid var(--border);
	border-radius: var(--radius-md);
	background: color-mix(in srgb, var(--bg-card) 40%, transparent);
	color: var(--fg-subtle);
	font-size: 13px;
	white-space: nowrap;
	overflow: hidden;
	cursor: pointer;
}

.sl-searchbtn:hover {
	background: color-mix(in srgb, var(--bg-hover) 55%, transparent);
	color: var(--fg);
}

.sl-searchbtn :global(.sl-ico) {
	font-size: 16px;
}

.sl-searchbtn :global(.sl-kbd) {
	margin-left: auto;
}

.label {
	flex: 1;
	text-align: left;
}

.account {
	position: relative;
	width: 40px;
	height: 40px;
	display: grid;
	place-items: center;
	padding: 0;
	border: 0;
	background: transparent;
	cursor: pointer;
	border-radius: var(--radius-md);
}
</style>
