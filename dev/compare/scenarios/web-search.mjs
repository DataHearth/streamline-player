import { search, targets } from "./browse-lib.mjs";

export const target = targets.web.target;
export const mockArgs = [];
export default (ctx) => search(ctx, targets.web);
