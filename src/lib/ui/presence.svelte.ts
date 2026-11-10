/** Keeps a panel mounted while it animates out: `shown` flips a frame after `mounted` so the enter transition runs. */
export function presence(open: () => boolean, exitMs: number) {
	const state = $state({ mounted: false, shown: false });
	$effect(() => {
		if (open()) {
			state.mounted = true;
			let second = 0;
			const first = requestAnimationFrame(() => {
				second = requestAnimationFrame(() => {
					state.shown = true;
				});
			});
			return () => {
				cancelAnimationFrame(first);
				cancelAnimationFrame(second);
			};
		}
		state.shown = false;
		const timer = setTimeout(() => {
			state.mounted = false;
		}, exitMs);
		return () => clearTimeout(timer);
	});
	return state;
}
