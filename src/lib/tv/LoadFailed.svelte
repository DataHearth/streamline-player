<script lang="ts">
	import HaltScreen from "#lib/player/HaltScreen.svelte";
	import { haltFor, type Halt } from "#lib/player/halt.ts";
	import { invoke } from "@tauri-apps/api/core";
	import { onMount } from "svelte";
	import { focusDefault } from "./focus";

	let { error, retry, onloaded, onback }: { error: string; retry: () => Promise<unknown>; onloaded: () => void; onback?: () => void } = $props();

	let halt = $state<Halt | null>(null);
	let host = "";
	let attempt = 0;
	let timer: ReturnType<typeof setInterval> | undefined;
	let root: HTMLElement;

	function show(raw: string) {
		const { halt: h, retryIn } = haltFor(raw, { host });
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
		invoke<string>("server_host").then((h) => {
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
	<HaltScreen {halt} backLabel="Back" onretry={tryAgain} {onback} />
{/if}
