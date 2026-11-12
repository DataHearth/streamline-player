// node --import ./dev/controller-check/register.mjs dev/pairing-check.ts
import { deepStrictEqual as eq } from "node:assert";
import { checkAddress, createPairing, type PairPort } from "../src/lib/core/pairing.svelte.ts";
import type { Pairing } from "../src/lib/core/types.ts";

eq(checkAddress(""), "empty");
eq(checkAddress("   "), "empty");
eq(checkAddress("not an address!"), "bad");
eq(checkAddress("streamline.home.arpa:8081"), null);
eq(checkAddress("http://127.0.0.1:8081/"), null);

const flush = async () => {
	for (let i = 0; i < 5; i++) await new Promise((done) => setImmediate(done));
};

function clock() {
	let now = 0;
	let next = 1;
	const queue = new Map<number, { at: number; fn: () => void }>();
	return {
		set: ((fn: () => void, ms: number) => {
			queue.set(next, { at: now + ms, fn });
			return next++;
		}) as unknown as typeof setTimeout,
		clear: ((id: number) => queue.delete(id)) as unknown as typeof clearTimeout,
		async advance(ms: number) {
			const end = now + ms;
			for (;;) {
				const due = [...queue.entries()]
					.filter(([, t]) => t.at <= end)
					.sort((a, b) => a[1].at - b[1].at)[0];
				if (!due) break;
				queue.delete(due[0]);
				now = due[1].at;
				due[1].fn();
				await flush();
			}
			now = end;
		},
		get pending() {
			return queue.size;
		},
	};
}

const code = (n: number, expires_in = 600, interval = 5): Pairing => ({
	code: `AAA-00${n}`,
	name: "Desk",
	uri: "https://s/pair",
	uri_complete: `https://s/pair?code=AAA-00${n}`,
	expires_in,
	interval,
	qr: "",
});

function setup(
	over: Partial<PairPort> & { answers?: string[]; expires?: number; interval?: number } = {},
) {
	const t = clock();
	const calls = { start: 0, poll: [] as number[] };
	const answers = over.answers ?? [];
	let at = 0;
	const port: PairPort = {
		connect: async () => 1,
		pairStart: over.pairStart ?? (async () => code(++calls.start, over.expires, over.interval)),
		pairPoll: async () => {
			calls.poll.push(at);
			return answers.shift() ?? "pending";
		},
	};
	const p = createPairing(port, { set: t.set, clear: t.clear });
	return {
		p,
		calls,
		advance: async (ms: number) => {
			at += ms;
			await t.advance(ms);
		},
		t,
	};
}

{
	const s = setup();
	await s.p.start();
	eq(s.p.pairing?.code, "AAA-001");
	eq(s.p.status, "waiting");
	eq(s.p.left, 600);
	await s.advance(3000);
	eq(s.p.left, 597);
	s.p.openedBrowser();
	eq(s.p.status, "browser");
	s.p.stop();
	eq(s.t.pending, 0);
}

{
	const s = setup({ answers: ["pending", "paired"] });
	await s.p.start();
	await s.advance(5000);
	eq(s.p.status, "waiting");
	await s.advance(5000);
	eq(s.p.status, "approved");
	eq(s.t.pending, 0);
	const polled = s.calls.poll.length;
	await s.advance(60000);
	eq(s.calls.poll.length, polled);
}

{
	const s = setup({ answers: ["denied"] });
	await s.p.start();
	await s.advance(5000);
	eq(s.p.status, "denied");
	eq(s.t.pending, 0);
	await s.p.start();
	eq(s.p.status, "waiting");
	eq(s.p.pairing?.code, "AAA-002");
}

{
	const s = setup({ answers: ["expired"] });
	await s.p.start();
	await s.advance(5000);
	eq(s.calls.start, 2);
	eq(s.p.pairing?.code, "AAA-002");
	eq(s.p.left, 600);
}

{
	const s = setup({ expires: 4, interval: 60 });
	await s.p.start();
	await s.advance(2000);
	eq(s.p.pairing?.code, "AAA-001");
	eq(s.p.left, 2);
	await s.advance(1000);
	eq(s.p.left, 1);
	await s.advance(1000);
	eq(s.p.pairing?.code, "AAA-002");
	eq(s.p.left, 4);
	eq(s.calls.poll.length, 0);
}

{
	const s = setup({
		pairStart: async () => {
			throw new Error("error sending request");
		},
	});
	await s.p.start();
	eq(s.p.status, "unreachable");
	eq(s.p.pairing, null);
	eq(s.t.pending, 0);
}

{
	const s = setup({
		pairStart: async () => {
			throw JSON.stringify({ error: { code: "x" } });
		},
	});
	await s.p.start();
	eq(s.p.status, "refused");
}

{
	const s = setup({ answers: ["slow_down"] });
	await s.p.start();
	await s.advance(5000);
	eq(s.calls.poll.length, 1);
	await s.advance(9000);
	eq(s.calls.poll.length, 1);
	await s.advance(1000);
	eq(s.calls.poll.length, 2);
}

console.log("pairing-check: ok");
