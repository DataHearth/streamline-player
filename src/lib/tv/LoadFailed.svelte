<script lang="ts">
import { m as i18n } from "#lib/paraglide/messages.js";
import HaltScreen from "#lib/tv/HaltScreen.svelte";
import { haltFor, type Halt } from "#lib/core/playback/halt.ts";
import { PLAYER_NAME, serverHost } from "#lib/platform/native.ts";
import { onMount } from "svelte";
import { focusDefault } from "./focus";

let {
	error,
	retry,
	onloaded,
	onback,
}: { error: string; retry: () => Promise<unknown>; onloaded: () => void; onback?: () => void } =
	$props();

let halt = $state<Halt | null>(null);
let host = "";
let attempt = 0;
let timer: ReturnType<typeof setInterval> | undefined;
let root: HTMLElement;

function show(raw: string) {
	const { halt: h, retryIn } = haltFor(raw, { host }, PLAYER_NAME);
	clearInterval(timer);
	halt = retryIn ? { ...h, left: retryIn, total: retryIn } : h;
	if (retryIn) {
		timer = setInterval(() => {
			if (!halt?.left) return;
			if (halt.left <= 1) tryAgain();
			else halt = { ...halt, left: halt.left - 1 };
		}, 1000);
	}
	setTimeout(() => focusDefault(root));
}

function tryAgain() {
	const id = ++attempt;
	clearInterval(timer);
	retry().then(
		() => id === attempt && onloaded(),
		(e) => id === attempt && show(String(e)),
	);
}

onMount(() => {
	root = document.querySelector<HTMLElement>("[data-tv-screen]") ?? document.body;
	serverHost().then((h) => {
		host = h;
		// svelte-ignore state_referenced_locally
		show(error);
	});
	return () => {
		attempt++;
		clearInterval(timer);
		// Whatever replaced this screen starts with nothing focused.
		focusDefault(root);
	};
});
</script>

{#if halt}
	<HaltScreen {halt} backLabel={i18n.common_back()} onretry={tryAgain} {onback} />
{/if}
