import type { Me } from "#lib/core/types.ts";

/** The signed-in account, shared by the shell's avatar and Settings so a change shows in both at once. */
export const account: { me: Me | null; version: number } = $state({ me: null, version: 0 });

export function setMe(me: Me, pictureChanged = false) {
	account.me = me;
	if (pictureChanged) account.version++;
}

export function clearMe() {
	account.me = null;
}

export const initialsOf = (name: string) =>
	name
		.trim()
		.split(/\s+/)
		.filter(Boolean)
		.slice(0, 2)
		.map((w) => w[0])
		.join("")
		.toUpperCase();
