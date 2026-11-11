<script lang="ts">
import { page } from "$app/state";
import { FORM_FACTOR } from "#lib/platform/target.ts";
import Person from "#lib/screens/Person.svelte";
import TvPerson from "#lib/screens/tv/Person.svelte";

const id = $derived(page.params.id ?? "");
</script>

{#if FORM_FACTOR === "tv"}
	<TvPerson {id} />
{:else}
	<Person {id} />
{/if}
