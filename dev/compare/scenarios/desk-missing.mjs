import { missing, targets } from "./browse-lib.mjs";

export const target = targets.desktop.target;
export const mockArgs = [];
export default (ctx) => missing(ctx, targets.desktop);
