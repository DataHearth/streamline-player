<script lang="ts">
import type { Kind } from "#lib/core/types.ts";
import Player from "#lib/screens/tv/Player.svelte";
import { page } from "$app/state";
</script>

<Player
	kind={page.params.kind as Kind}
	id={Number(page.params.id)}
	epId={Number(page.url.searchParams.get("ep")) || null}
	resume={page.url.searchParams.has("resume")}
/>
