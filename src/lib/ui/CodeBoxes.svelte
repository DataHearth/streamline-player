<script lang="ts">
let {
	code,
	approved = false,
	size = "pair",
}: { code: string; approved?: boolean; size?: "pair" | "approve" } = $props();

const chars = $derived([...code.toUpperCase()]);
</script>

<div class="code {size}" role="img" aria-label={code}>
	{#each chars as ch}
		{#if ch === "-"}
			<span class="dash">–</span>
		{:else}
			<span class="ch" class:ok={approved}>{ch}</span>
		{/if}
	{/each}
</div>

<style>
.code {
	display: flex;
	align-items: center;
	gap: 6px;
}

.ch,
.dash {
	display: grid;
	place-items: center;
	font-family: var(--font-mono);
	font-weight: 500;
}

.ch {
	border-radius: var(--radius-md);
	background: var(--bg-deep);
	border: 1px solid var(--border-strong);
	color: var(--fg);
}

.ch.ok {
	background: color-mix(in srgb, var(--status-available) 10%, transparent);
	border-color: color-mix(in srgb, var(--status-available) 35%, transparent);
	color: var(--status-available);
}

.dash {
	width: 18px;
	color: var(--fg-faint);
}

.pair .ch {
	width: 50px;
	height: 62px;
	font-size: 30px;
}

.pair .dash {
	height: 62px;
	font-size: 30px;
}

.approve .ch {
	width: 54px;
	height: 58px;
	font-size: 28px;
}

.approve .dash {
	height: 58px;
	font-size: 28px;
}
</style>
