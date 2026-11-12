import type { Pairing } from "#lib/core/types.ts";

export type PairPort = {
	connect(url: string): Promise<unknown>;
	pairStart(): Promise<Pairing>;
	pairPoll(): Promise<string>;
};

export type Status = "waiting" | "browser" | "approved" | "denied" | "unreachable" | "refused";

// Wrapped by hand: a browser's setTimeout throws "Illegal invocation" when called as a method of this object.
type Timer = ReturnType<typeof setTimeout>;
type Timers = {
	set: (fn: () => void, ms: number) => Timer;
	clear: (id: Timer | undefined) => void;
};

const SLOW_DOWN_SECS = 5;
const ADDRESS = /^([a-z]+:\/\/)?[a-z0-9.-]+(:\d+)?(\/.*)?$/i;

export function checkAddress(typed: string): "empty" | "bad" | null {
	const url = typed.trim();
	if (!url) return "empty";
	return ADDRESS.test(url) ? null : "bad";
}

/** One pairing at a time: a code on screen, polled until approved, denied or replaced by a fresh one. */
export function createPairing(
	port: PairPort,
	timers: Timers = {
		set: (fn, ms) => setTimeout(fn, ms),
		clear: (id) => clearTimeout(id),
	},
) {
	let pairing = $state<Pairing | null>(null);
	let left = $state(0);
	let status = $state<Status>("waiting");
	let generation = 0;
	let poller: ReturnType<typeof setTimeout> | undefined;
	let clock: ReturnType<typeof setTimeout> | undefined;

	const stop = () => {
		generation++;
		timers.clear(poller);
		timers.clear(clock);
	};

	const tick = (id: number) => {
		clock = timers.set(() => {
			if (id !== generation) return;
			if (left <= 1) void start();
			else {
				left--;
				tick(id);
			}
		}, 1000);
	};

	const poll = (id: number, interval: number) => {
		poller = timers.set(async () => {
			const answer = await port.pairPoll().catch(() => "pending");
			if (id !== generation) return;
			if (answer === "paired") {
				stop();
				status = "approved";
			} else if (answer === "expired") void start();
			else if (answer === "denied") {
				stop();
				status = "denied";
			} else poll(id, answer === "slow_down" ? interval + SLOW_DOWN_SECS : interval);
		}, interval * 1000);
	};

	async function start() {
		stop();
		const id = generation;
		try {
			const next = await port.pairStart();
			if (id !== generation) return;
			pairing = next;
			left = next.expires_in;
			status = "waiting";
			tick(id);
			poll(id, next.interval);
		} catch (e) {
			if (id !== generation) return;
			pairing = null;
			status = /"code"/.test(String(e)) ? "refused" : "unreachable";
		}
	}

	return {
		get pairing() {
			return pairing;
		},
		get left() {
			return left;
		},
		get status() {
			return status;
		},
		start,
		stop,
		reset() {
			stop();
			pairing = null;
			status = "waiting";
		},
		openedBrowser() {
			if (status === "waiting") status = "browser";
		},
	};
}
