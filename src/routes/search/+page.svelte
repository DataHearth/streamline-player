<script lang="ts">
import { FORM_FACTOR } from "#lib/platform/target.ts";
import Search from "#lib/screens/Search.svelte";
import TvSearch from "#lib/screens/tv/Search.svelte";
</script>

{#if FORM_FACTOR === "tv"}
	<TvSearch />
{:else}
	<Search />
{/if}
