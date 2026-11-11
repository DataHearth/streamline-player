<script lang="ts">
import "#lib/ui/ui.css";

const PALETTE = [
	"var(--kind-album)",
	"var(--kind-episode)",
	"var(--kind-movie)",
	"var(--kind-book)",
];

let {
	name,
	email,
	initials,
	size = 40,
	color,
	ring = false,
	picture,
}: {
	name?: string;
	email?: string;
	initials?: string;
	size?: 32 | 40 | 48;
	color?: string;
	ring?: boolean;
	picture?: string;
} = $props();

let broken = $state(false);
$effect(() => {
	picture;
	broken = false;
});

const hash = (s: string) => {
	let h = 0;
	for (const c of s) h = (h * 31 + c.charCodeAt(0)) | 0;
	return Math.abs(h);
};
const source = $derived(name || email || "?");
const text = $derived(
	initials ||
		source
			.split(/[\s@._-]+/)
			.filter(Boolean)
			.slice(0, 2)
			.map((w) => w[0])
			.join("")
			.toUpperCase(),
);
const fill = $derived(color || PALETTE[hash(email || name || text) % PALETTE.length]);
</script>

<span
	class="sl-avatar"
	class:sl-avatar--32={size === 32}
	class:sl-avatar--48={size === 48}
	class:ring
	role="img"
	aria-label={name || email || text}
	style:--av={fill}
	>{#if picture && !broken}
		<img src={picture} alt="" onerror={() => (broken = true)}>
	{:else}
		{text}
	{/if}</span
>

<style>
img {
	width: 100%;
	height: 100%;
	border-radius: inherit;
	object-fit: cover;
}

.ring {
	box-shadow: 0 0 0 2px var(--accent);
}
</style>
