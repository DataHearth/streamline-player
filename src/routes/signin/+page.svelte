<script lang="ts">
import { goto } from "#lib/ui/nav.ts";
import { page } from "$app/state";
import { safeNext } from "#lib/core/auth.ts";
import SignIn from "#lib/screens/SignIn.svelte";

const next = $derived(safeNext(page.url.searchParams.get("next")));
</script>

<SignIn
	{next}
	failed={page.url.searchParams.get("error")}
	done={() => goto(next, { replace: true })}
/>
