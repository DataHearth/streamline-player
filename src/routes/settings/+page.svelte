<script lang="ts">
import { FORM_FACTOR } from "#lib/platform/target.ts";
import Settings from "#lib/screens/Settings.svelte";
import TvSettings from "#lib/screens/tv/Settings.svelte";
</script>

{#if FORM_FACTOR === "tv"}
	<TvSettings />
{:else}
	<Settings />
{/if}
