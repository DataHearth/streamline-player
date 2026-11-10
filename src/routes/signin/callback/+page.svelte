<script lang="ts">
import { goto } from "$app/navigation";
import { page } from "$app/state";
import { SignInError, safeNext } from "#lib/core/auth.ts";
import { finishSso } from "#platform";
import { onMount } from "svelte";

onMount(() => {
	finishSso(new URLSearchParams(page.url.search))
		.then((next) => goto(safeNext(next), { replace: true }))
		.catch((e) => {
			const code = e instanceof SignInError ? e.code : "network";
			return goto(`/signin?error=${code}`, { replace: true });
		});
});
</script>
