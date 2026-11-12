<script lang="ts">
import type { Kind } from "#lib/core/types.ts";
import { FORM_FACTOR } from "#lib/platform/target.ts";
import Player from "#lib/screens/Player.svelte";
import TvPlayer from "#lib/screens/tv/Player.svelte";
import { page } from "$app/state";

const props = $derived({
	kind: page.params.kind as Kind,
	id: Number(page.params.id),
	epId: Number(page.url.searchParams.get("ep")) || null,
	resume: page.url.searchParams.has("resume"),
});
</script>

{#if FORM_FACTOR === "tv"}
	<TvPlayer {...props} />
{:else}
	<Player {...props} />
{/if}
