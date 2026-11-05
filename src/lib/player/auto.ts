export const PRESETS = [20, 8, 4, 2, 1];

const MBIT = 1_000_000;
const USABLE_SHARE = 0.8;
const ORIGINAL_MARGIN = 1.15;
const KEY = "throughput";

const usableMbit = (bitsPerSecond: number) => (bitsPerSecond / MBIT) * USABLE_SHARE;

/** No measurement yet means no reason to hold back: a stall corrects it. */
export function autoCap(throughput: number | null, fileMbit: number | null, originalPlays: boolean) {
	const usable = throughput === null ? Infinity : usableMbit(throughput);
	if (originalPlays && usable >= (fileMbit ?? Infinity) * ORIGINAL_MARGIN) return null;
	return PRESETS.find((n) => n <= usable) ?? 1;
}

/**
 * Where a stall takes Auto, or null to stay. `measured` must come from this stall: a stream that
 * restarts after a seek stalls too, and an older reading says nothing about why. `mayGuess` is for a
 * stall gone on so long without a reading that the first big segment on a slow link is the likely cause.
 */
export function stepDown(measured: number | null, currentMbit: number, mayGuess: boolean) {
	if (measured === null && !mayGuess) return null;
	const usable = measured === null ? Infinity : usableMbit(measured);
	if (measured !== null && usable >= currentMbit) return null;
	return PRESETS.find((n) => n < currentMbit && n <= usable) ?? (currentMbit > 1 ? 1 : null);
}

// ponytail: one value per device, enough for a TV that never moves. Key it by network when phones ship.
export function rememberedThroughput() {
	try {
		return Number(localStorage.getItem(KEY)) || null;
	} catch {
		return null;
	}
}

export function rememberThroughput(bitsPerSecond: number) {
	try {
		localStorage.setItem(KEY, String(bitsPerSecond));
	} catch {
		// Next start measures again.
	}
}
