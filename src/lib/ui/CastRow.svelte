<script lang="ts">
export type Member = {
	id: string;
	name: string;
	character: string;
	photo: string;
	initials: string;
	bg: string;
	fg: string;
};

let {
	title,
	count,
	members,
	padX = 56,
	onopen,
}: {
	title: string;
	count: number;
	members: Member[];
	padX?: number;
	onopen: (id: string) => void;
} = $props();

let broken = $state<Record<string, boolean>>({});
</script>

<section>
	<div class="cast-head">
		<h2>{title}</h2>
		<span class="count">{count}</span>
	</div>
	<div class="row" style:margin="0 -{padX}px" style:padding="0 {padX - 6}px 4px">
		{#each members as c (c.id)}
			<button type="button" class="castc" onclick={() => onopen(c.id)}>
				<span class="img" style:background={c.bg} style:color={c.fg}>
					{#if !broken[c.id]}
						<img src={c.photo} alt="" loading="lazy" onerror={() => (broken[c.id] = true)}>
					{:else}
						{c.initials}
					{/if}
				</span>
				<span class="who">
					<span class="n">{c.name}</span>
					<span class="r">{c.character}</span>
				</span>
			</button>
		{/each}
	</div>
</section>

<style>
section {
	display: flex;
	flex-direction: column;
	gap: 12px;
}

.cast-head {
	display: flex;
	align-items: baseline;
	gap: 12px;
}

h2 {
	margin: 0;
	font-size: 18px;
	font-weight: 600;
}

.count {
	font: 400 12px var(--font-mono);
	color: var(--fg-subtle);
}

.row {
	display: flex;
	gap: 4px;
	overflow-x: auto;
}

.castc {
	flex: none;
	width: 124px;
	display: flex;
	flex-direction: column;
	align-items: center;
	gap: 10px;
	padding: 10px 6px 12px;
	border: 0;
	border-radius: 14px;
	background: none;
	color: var(--fg);
	text-align: center;
	cursor: pointer;
}

.castc:hover {
	background: var(--surface);
}

.img {
	display: grid;
	place-items: center;
	width: 84px;
	height: 84px;
	border-radius: 50%;
	overflow: hidden;
	font-size: 27px;
	font-weight: 600;
	letter-spacing: -0.01em;
	box-shadow: 0 0 0 1px var(--border);
}

.img img {
	width: 100%;
	height: 100%;
	object-fit: cover;
	display: block;
}

.who {
	display: flex;
	flex-direction: column;
	gap: 3px;
	width: 100%;
	min-width: 0;
}

.n,
.r {
	display: -webkit-box;
	-webkit-line-clamp: 2;
	line-clamp: 2;
	-webkit-box-orient: vertical;
	overflow: hidden;
}

.n {
	font-size: 13.5px;
	font-weight: 600;
	line-height: 1.3;
}

.r {
	font-size: 12px;
	line-height: 1.35;
	color: var(--fg-subtle);
}
</style>
