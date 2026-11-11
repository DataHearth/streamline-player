<script lang="ts" module>
import {
	ArrowLeft,
	ArrowUpDown,
	Bell,
	Captions,
	Check,
	CheckCheck,
	ChevronDown,
	ChevronLeft,
	ChevronRight,
	CircleCheck,
	CirclePlay,
	Cpu,
	Download,
	Eye,
	Film,
	Gauge,
	Globe,
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
	Monitor,
	MonitorSmartphone,
	Play,
	RotateCcw,
	Search,
	Settings,
	SlidersHorizontal,
	Smartphone,
	Square,
	Tablet,
	Tv,
	User,
	Wifi,
	X,
} from "@lucide/svelte";

const ICONS = {
	"arrow-left": ArrowLeft,
	"arrow-up-down": ArrowUpDown,
	bell: Bell,
	captions: Captions,
	check: CircleCheck,
	"circle-play": CirclePlay,
	"check-check": CheckCheck,
	"chevron-down": ChevronDown,
	"chevron-left": ChevronLeft,
	"chevron-right": ChevronRight,
	cpu: Cpu,
	download: Download,
	eye: Eye,
	film: Film,
	gauge: Gauge,
	globe: Globe,
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
	monitor: Monitor,
	"monitor-smartphone": MonitorSmartphone,
	play: Play,
	"rotate-ccw": RotateCcw,
	search: Search,
	settings: Settings,
	sliders: SlidersHorizontal,
	smartphone: Smartphone,
	square: Square,
	tablet: Tablet,
	tick: Check,
	tv: Tv,
	user: User,
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
