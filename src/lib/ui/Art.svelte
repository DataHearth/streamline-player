<script lang="ts">
import Icon from "#lib/ui/Icon.svelte";

let {
	src,
	ratio,
	kind = "backdrop",
	position,
}: { src?: string; ratio?: string; kind?: "poster" | "backdrop"; position?: string } = $props();

let failed = $state(false);
let loaded = $state(false);
let img = $state<HTMLImageElement>();

$effect(() => {
	src;
	failed = false;
	loaded = false;
});
// A cached image can finish before the handlers are attached.
$effect(() => {
	if (img?.complete) {
		if (img.naturalWidth > 0) loaded = true;
		else failed = true;
	}
});
</script>

<span class="art" class:sized={!!ratio} style:aspect-ratio={ratio}>
	{#if kind === "poster"}
		<span class="ph"><Icon name="film" /></span>
	{:else}
		<span class="frame" aria-hidden="true"></span>
	{/if}
	{#if src && !failed}
		<img
			bind:this={img}
			{src}
			alt=""
			loading="lazy"
			decoding="async"
			class:loaded
			style:object-position={position}
			onload={() => (loaded = true)}
			onerror={() => (failed = true)}
		>
	{/if}
</span>

<style>
.art {
	position: absolute;
	inset: 0;
	display: block;
	overflow: hidden;
}

.art.sized {
	position: relative;
	inset: auto;
	width: 100%;
}

.frame {
	position: absolute;
	inset: 0;
	overflow: hidden;
	background: var(--bg-stage);
}

.frame::before {
	content: "";
	position: absolute;
	inset: 18% -10% 30% 40%;
	border-radius: 50%;
	background: color-mix(in srgb, var(--accent) 24%, var(--bg-stage));
	filter: blur(60px);
}

.frame::after {
	content: "";
	position: absolute;
	inset: 55% 30% -20% -10%;
	border-radius: 50%;
	background: color-mix(in srgb, var(--status-wanted) 16%, var(--bg-stage));
	filter: blur(70px);
}

.ph {
	position: absolute;
	inset: 0;
	display: grid;
	place-items: center;
	font-size: 40px;
	color: var(--fg-faint);
}

img {
	position: absolute;
	inset: 0;
	width: 100%;
	height: 100%;
	object-fit: cover;
	display: block;
	opacity: 0;
}

img.loaded {
	opacity: 1;
}
</style>
