<script lang="ts">
import type { Snippet } from "svelte";

let { id, title, children }: { id: string; title: string; children: Snippet } = $props();
</script>

<section data-sec={id}>
	<h3>{title}</h3>
	<div class="card">{@render children()}</div>
</section>

<style>
section {
	display: flex;
	flex-direction: column;
	gap: 10px;
	scroll-margin-top: 92px;
}

h3 {
	margin: 0;
	padding: 0 4px;
	font-size: 16px;
	font-weight: 600;
	letter-spacing: -0.015em;
	line-height: 1.3;
}

.card {
	border-radius: var(--radius-lg);
	background: var(--bg-elevated);
	border: 1px solid var(--border);
	overflow: hidden;
}
</style>
