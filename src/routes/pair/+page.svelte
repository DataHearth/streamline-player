<script lang="ts">
import { goto } from "#lib/ui/nav.ts";
import { page } from "$app/state";
import { IS_WEB } from "#lib/platform/target.ts";
import Approve from "#lib/screens/Approve.svelte";
import { onMount } from "svelte";

onMount(() => {
	if (!IS_WEB) goto("/", { replace: true });
});
</script>

{#if IS_WEB}
	<Approve code={page.url.searchParams.get("code")} />
{/if}
