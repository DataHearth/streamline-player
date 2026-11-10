<script lang="ts">
import { m as i18n } from "#lib/paraglide/messages.js";
import Icon from "#lib/ui/Icon.svelte";
import { TARGET } from "#lib/platform/target.ts";
import { windowControls } from "#platform";

let { hidden }: { hidden: boolean } = $props();
</script>

{#if TARGET === "windows"}
	<div class="win" class:hidden>
		<button type="button" aria-label={i18n.shell_minimize()} onclick={windowControls.minimize}>
			<Icon name="minus" size={14} stroke={1.25} />
		</button>
		<button
			type="button"
			aria-label={i18n.shell_maximize()}
			onclick={windowControls.toggleMaximize}
		>
			<Icon name="square" size={12} stroke={1.5} />
		</button>
		<button
			type="button"
			class="close"
			aria-label={i18n.shell_close()}
			onclick={windowControls.close}
		>
			<Icon name="x" size={15} stroke={1.25} />
		</button>
	</div>
{:else if TARGET === "linux"}
	<div class="gnome" class:hidden>
		<button type="button" aria-label={i18n.shell_minimize()} onclick={windowControls.minimize}>
			<Icon name="minus" size={13} stroke={2.25} />
		</button>
		<button
			type="button"
			aria-label={i18n.shell_maximize()}
			onclick={windowControls.toggleMaximize}
		>
			<Icon name="square" size={11} stroke={2.25} />
		</button>
		<button type="button" aria-label={i18n.shell_close()} onclick={windowControls.close}>
			<Icon name="x" size={13} stroke={2.25} />
		</button>
	</div>
{/if}

<style>
.win,
.gnome {
	position: absolute;
	display: flex;
	z-index: 70;
	transition: opacity 200ms;
}

.hidden {
	opacity: 0;
	pointer-events: none;
}

button {
	display: grid;
	place-items: center;
	padding: 0;
	border: 0;
	cursor: default;
}

.win {
	right: 0;
	top: 0;
}

.win button {
	width: 46px;
	height: 36px;
	background: transparent;
	color: #c8c8d2;
}

.win button:hover {
	background: rgba(255, 255, 255, 0.08);
}

.win .close {
	border-top-right-radius: 8px;
}

.win .close:hover {
	background: #c42b1c;
	color: #fff;
}

.gnome {
	right: 16px;
	top: 18px;
	gap: 12px;
}

.gnome button {
	width: 26px;
	height: 26px;
	border-radius: 50%;
	background: rgba(255, 255, 255, 0.1);
	color: #f5f5f7;
}

.gnome button:hover {
	background: rgba(255, 255, 255, 0.18);
}
</style>
