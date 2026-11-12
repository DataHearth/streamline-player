<script lang="ts">
let { label }: { label: string } = $props();
</script>

<div class="spin" role="status" aria-label={label}></div>

<style>
.spin {
	position: absolute;
	left: 50%;
	top: 50%;
	width: 44px;
	height: 44px;
	margin: -22px 0 0 -22px;
	border: 3px solid rgba(255, 255, 255, 0.18);
	border-top-color: var(--player-ink);
	border-radius: 50%;
	animation: spin 800ms linear infinite;
	pointer-events: none;
}

@keyframes spin {
	to {
		transform: rotate(360deg);
	}
}
</style>
