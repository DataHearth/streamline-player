<script lang="ts" module>
import {
	Bell,
	Check,
	ChevronDown,
	ChevronLeft,
	ChevronRight,
	CircleCheck,
	Cpu,
	Download,
	Film,
	Gauge,
	Info,
	Keyboard,
	KeyRound,
	LogOut,
	Mail,
	Minus,
	Play,
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
	"chevron-left": ChevronLeft,
	"chevron-right": ChevronRight,
	cpu: Cpu,
	download: Download,
	film: Film,
	gauge: Gauge,
	info: Info,
	keyboard: Keyboard,
	"key-round": KeyRound,
	"log-out": LogOut,
	mail: Mail,
	minus: Minus,
	play: Play,
	search: Search,
	settings: Settings,
	square: Square,
	tick: Check,
	wifi: Wifi,
	x: X,
};

export type IconName = keyof typeof ICONS;
</script>

<script lang="ts">
import "#lib/ui/ui.css";

let {
	name,
	size,
	stroke,
	filled = false,
}: { name: IconName; size?: number; stroke?: number; filled?: boolean } = $props();
const Glyph = $derived(ICONS[name]);
</script>

<Glyph
	class="sl-ico"
	aria-hidden="true"
	style={[size && `font-size:${size}px`, filled && "fill:currentColor"].filter(Boolean).join(";") ||
		undefined}
	strokeWidth={stroke}
/>
