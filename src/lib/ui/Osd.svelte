<script lang="ts">
let { text }: { text: string } = $props();
</script>

<div class="wrap"><div class="osd">{text}</div></div>

<style>
.wrap {
	position: absolute;
	left: 50%;
	top: 50%;
	transform: translate(-50%, -50%);
	pointer-events: none;
}

.osd {
	display: inline-flex;
	align-items: center;
	gap: 12px;
	padding: 10px 18px;
	border-radius: 14px;
	background: rgba(0, 0, 0, 0.6);
	backdrop-filter: blur(12px);
	color: var(--player-ink);
	font: 500 22px var(--font-mono);
}
</style>
