<script lang="ts">
import { FORM_FACTOR } from "#lib/platform/target.ts";
import Home from "#lib/screens/Home.svelte";
import TvHome from "#lib/screens/tv/Home.svelte";
</script>

{#if FORM_FACTOR === "tv"}
	<TvHome />
{:else}
	<Home />
{/if}
