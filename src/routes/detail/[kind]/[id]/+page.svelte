<script lang="ts">
import { page } from "$app/state";
import type { Kind } from "#lib/core/types.ts";
import { FORM_FACTOR } from "#lib/platform/target.ts";
import Detail from "#lib/screens/Detail.svelte";
import TvDetail from "#lib/screens/tv/Detail.svelte";

const kind = $derived(page.params.kind as Kind);
const id = $derived(Number(page.params.id));
</script>

{#if FORM_FACTOR === "tv"}
	<TvDetail {kind} {id} />
{:else}
	<Detail {kind} {id} />
{/if}
