<script lang="ts">
import Kbd from "#lib/ui/Kbd.svelte";
import "#lib/ui/ui.css";

export type KeyGroup = { title: string; items: { keys: string[]; label: string }[] };

let { groups, columns = 2 }: { groups: KeyGroup[]; columns?: number } = $props();
</script>

<div class="sl-keys" style:--cols={columns}>
	{#each groups as g (g.title)}
		<div class="sl-col" style:gap="0">
			<h3 class="sl-sec" style:padding="0">{g.title}</h3>
			{#each g.items as it (it.label)}
				<div class="sl-keys__row">
					<span>{it.label}</span>
					<span class="sl-keys__k">
						{#each it.keys as k, i (i)}
							<Kbd>{k}</Kbd>
						{/each}
					</span>
				</div>
			{/each}
		</div>
	{/each}
</div>
