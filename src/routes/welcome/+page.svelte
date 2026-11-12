<script lang="ts">
import { FORM_FACTOR } from "#lib/platform/target.ts";
import Welcome from "#lib/screens/Welcome.svelte";
import TvWelcome from "#lib/screens/tv/Welcome.svelte";
</script>

{#if FORM_FACTOR === "tv"}
	<TvWelcome />
{:else}
	<Welcome />
{/if}
