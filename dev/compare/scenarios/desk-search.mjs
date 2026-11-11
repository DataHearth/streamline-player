import { search, targets } from "./browse-lib.mjs";

export const target = targets.desktop.target;
export const mockArgs = [];
export default (ctx) => search(ctx, targets.desktop);
