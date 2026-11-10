<script lang="ts">
import { page } from "$app/state";
import type { Kind } from "#lib/core/types.ts";
import Library from "#lib/screens/tv/Library.svelte";
</script>

<Library kind={page.params.kind as Kind} />
