<script lang="ts">
import Avatar from "#lib/ui/Avatar.svelte";
import Icon, { type IconName } from "#lib/ui/Icon.svelte";
import Kbd from "#lib/ui/Kbd.svelte";
import "#lib/ui/ui.css";

export type MenuItem = {
	icon?: IconName;
	label: string;
	keys?: string[];
	separator?: boolean;
	onclick: () => void;
};

let {
	name,
	email,
	initials,
	color,
	picture,
	items,
	label,
}: {
	name: string;
	email?: string;
	initials?: string;
	color?: string;
	picture?: string;
	items: MenuItem[];
	label: string;
} = $props();
</script>

<div class="sl-acct" role="menu" aria-label={label}>
	<div class="sl-acct__head">
		<Avatar {name} {email} {initials} {color} {picture} size={40} />
		<span class="sl-acct__who">
			<span class="sl-acct__name">{name}</span>
			{#if email}
				<span class="sl-acct__mail">{email}</span>
			{/if}
		</span>
	</div>
	{#each items as it (it.label)}
		{#if it.separator}
			<hr class="sl-acct__sep">
		{/if}
		<button type="button" role="menuitem" class="sl-acct__item" onclick={it.onclick}>
			{#if it.icon}
				<Icon name={it.icon} />
			{/if}
			<span class="sl-acct__label">{it.label}</span>
			{#if it.keys?.length}
				<span class="sl-acct__keys">
					{#each it.keys as k (k)}
						<Kbd>{k}</Kbd>
					{/each}
				</span>
			{/if}
		</button>
	{/each}
</div>
