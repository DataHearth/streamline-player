<script lang="ts">
import { page } from "$app/state";
import type { Kind } from "#lib/core/types.ts";
import { FORM_FACTOR } from "#lib/platform/target.ts";
import Library from "#lib/screens/Library.svelte";
import TvLibrary from "#lib/screens/tv/Library.svelte";

const kind = $derived(page.params.kind as Kind);
</script>

{#if FORM_FACTOR === "tv"}
	<TvLibrary {kind} />
{:else}
	<Library {kind} />
{/if}
