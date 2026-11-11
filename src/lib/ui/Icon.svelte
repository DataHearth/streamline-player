<script lang="ts" module>
import {
	ArrowLeft,
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
	History,
	Info,
	Keyboard,
	KeyRound,
	Languages,
	LayoutGrid,
	List,
	ListFilter,
	LogOut,
	Mail,
	Minus,
	Play,
	RotateCcw,
	Search,
	Settings,
	SlidersHorizontal,
	Square,
	Wifi,
	X,
} from "@lucide/svelte";

const ICONS = {
	"arrow-left": ArrowLeft,
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
	history: History,
	info: Info,
	keyboard: Keyboard,
	"key-round": KeyRound,
	languages: Languages,
	"layout-grid": LayoutGrid,
	list: List,
	"list-filter": ListFilter,
	"log-out": LogOut,
	mail: Mail,
	minus: Minus,
	play: Play,
	"rotate-ccw": RotateCcw,
	search: Search,
	settings: Settings,
	sliders: SlidersHorizontal,
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
