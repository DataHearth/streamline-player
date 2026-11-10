<script lang="ts">
import { staticUrl } from "#lib/ui/nav.ts";
let { size = 36, src = staticUrl("streamline-mark.png") }: { size?: number; src?: string } =
	$props();
const radius = $derived(size >= 48 ? "12px" : size >= 36 ? "var(--radius-md)" : "8px");
</script>

<img {src} alt="" width={size} height={size} style:border-radius={radius}>

<style>
img {
	display: block;
	flex: none;
	box-shadow: 0 0 0 1px var(--border);
}
</style>
