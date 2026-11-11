<script lang="ts">
let {
	checked,
	label,
	onchange,
}: { checked: boolean; label: string; onchange: (on: boolean) => void } = $props();
</script>

<label class="switch">
	<input
		type="checkbox"
		role="switch"
		{checked}
		aria-checked={checked}
		aria-label={label}
		onchange={(e) => onchange(e.currentTarget.checked)}
	>
	<span class="track"></span>
</label>

<style>
.switch {
	position: relative;
	flex: none;
	display: block;
	width: 42px;
	height: 24px;
}

input {
	position: absolute;
	inset: 0;
	width: 100%;
	height: 100%;
	margin: 0;
	opacity: 0;
	cursor: pointer;
}

.track {
	display: block;
	box-sizing: border-box;
	width: 100%;
	height: 100%;
	padding: 2px;
	border-radius: var(--radius-full);
	background: color-mix(in srgb, var(--fg) 16%, transparent);
	pointer-events: none;
	transition: background 200ms cubic-bezier(0.2, 0, 0, 1);
}

.track::after {
	content: "";
	display: block;
	width: 20px;
	height: 20px;
	border-radius: 50%;
	background: var(--fg);
	box-shadow: 0 1px 3px rgba(0, 0, 0, 0.4);
	transition: transform 200ms cubic-bezier(0.2, 0, 0, 1);
}

input:checked + .track {
	background: var(--accent);
}

input:checked + .track::after {
	transform: translateX(18px);
}

input:focus-visible + .track {
	outline: 2px solid var(--accent-ring);
	outline-offset: 2px;
}
</style>
