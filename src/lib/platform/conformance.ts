import type * as N from "./native.ts";
import type * as W from "./web.ts";

type Same<A, B> = [A] extends [B] ? ([B] extends [A] ? true : false) : false;
export const sameExports: Same<keyof typeof N, keyof typeof W> = true;

type Signatures = {
	[K in keyof typeof N & keyof typeof W]: Same<(typeof N)[K], (typeof W)[K]>;
};
type Mismatched = {
	[K in keyof Signatures]: Signatures[K] extends true ? never : K;
}[keyof Signatures];
export const sameSignatures: [Mismatched] extends [never] ? true : Mismatched = "" as never;
