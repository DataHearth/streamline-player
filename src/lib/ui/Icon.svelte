<script lang="ts" module>
import {
	Bell,
	CircleCheck,
	Cpu,
	Download,
	Gauge,
	Keyboard,
	LogOut,
	Minus,
	Search,
	Settings,
	Square,
	Wifi,
	X,
} from "@lucide/svelte";

const ICONS = {
	bell: Bell,
	check: CircleCheck,
	cpu: Cpu,
	download: Download,
	gauge: Gauge,
	keyboard: Keyboard,
	"log-out": LogOut,
	minus: Minus,
	search: Search,
	settings: Settings,
	square: Square,
	wifi: Wifi,
	x: X,
};

export type IconName = keyof typeof ICONS;
</script>

<script lang="ts">
import "#lib/ui/ui.css";

let { name, size, stroke }: { name: IconName; size?: number; stroke?: number } = $props();
const Glyph = $derived(ICONS[name]);
</script>

<Glyph
	class="sl-ico"
	aria-hidden="true"
	style={size ? `font-size:${size}px` : undefined}
	strokeWidth={stroke}
/>
