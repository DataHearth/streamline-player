<script lang="ts" module>
import {
	ArrowUpDown,
	Bell,
	Check,
	CheckCheck,
	ChevronDown,
	ChevronLeft,
	ChevronRight,
	CircleCheck,
	Cpu,
	Download,
	Eye,
	Film,
	Gauge,
	Info,
	Keyboard,
	KeyRound,
	LayoutGrid,
	List,
	ListFilter,
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
	"arrow-up-down": ArrowUpDown,
	bell: Bell,
	check: CircleCheck,
	"check-check": CheckCheck,
	"chevron-down": ChevronDown,
	"chevron-left": ChevronLeft,
	"chevron-right": ChevronRight,
	cpu: Cpu,
	download: Download,
	eye: Eye,
	film: Film,
	gauge: Gauge,
	info: Info,
	keyboard: Keyboard,
	"key-round": KeyRound,
	"layout-grid": LayoutGrid,
	list: List,
	"list-filter": ListFilter,
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
