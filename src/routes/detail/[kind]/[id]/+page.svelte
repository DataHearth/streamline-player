<script lang="ts">
import { page } from "$app/state";
import type { Kind } from "#lib/core/types.ts";
import Detail from "#lib/screens/tv/Detail.svelte";
</script>

<Detail kind={page.params.kind as Kind} id={Number(page.params.id)} />
