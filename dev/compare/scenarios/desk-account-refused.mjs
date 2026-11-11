import { accountRefused, targets } from "./browse-lib.mjs";

export const target = targets.desktop.target;
export const mockArgs = [];
export default (ctx) => accountRefused(ctx, targets.desktop);
