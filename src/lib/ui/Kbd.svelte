<script lang="ts">
import "#lib/ui/ui.css";
import type { Snippet } from "svelte";

let { children }: { children: Snippet } = $props();
</script>

<span class="sl-kbd">{@render children()}</span>
