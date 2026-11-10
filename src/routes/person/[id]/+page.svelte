<script lang="ts">
import { page } from "$app/state";
import Person from "#lib/screens/tv/Person.svelte";
</script>

<Person id={page.params.id ?? ""} />
