<script lang="ts" module>
import {
	Bell,
	ChevronDown,
	CircleCheck,
	Cpu,
	Download,
	Gauge,
	Keyboard,
	KeyRound,
	LogOut,
	Mail,
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
	"chevron-down": ChevronDown,
	cpu: Cpu,
	download: Download,
	gauge: Gauge,
	keyboard: Keyboard,
	"key-round": KeyRound,
	"log-out": LogOut,
	mail: Mail,
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
