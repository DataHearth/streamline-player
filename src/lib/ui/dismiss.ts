/** Calls `onclose` on a press outside the node or on Escape, which the open popover keeps from reaching the shell. */
export function dismiss(node: HTMLElement, onclose: () => void) {
	let close = onclose;
	const onPress = (e: Event) => {
		if (!node.contains(e.target as Node)) close();
	};
	const onKey = (e: KeyboardEvent) => {
		if (e.key !== "Escape") return;
		e.stopPropagation();
		close();
	};
	document.addEventListener("pointerdown", onPress, true);
	window.addEventListener("keydown", onKey, true);
	return {
		update: (next: () => void) => {
			close = next;
		},
		destroy: () => {
			document.removeEventListener("pointerdown", onPress, true);
			window.removeEventListener("keydown", onKey, true);
		},
	};
}
