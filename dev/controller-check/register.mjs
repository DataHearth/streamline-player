import { register } from "node:module";

globalThis.localStorage = {
	_m: new Map(),
	getItem(k) {
		return this._m.get(k) ?? null;
	},
	setItem(k, v) {
		this._m.set(k, String(v));
	},
	removeItem(k) {
		this._m.delete(k);
	},
};
register("./hooks.mjs", import.meta.url);
