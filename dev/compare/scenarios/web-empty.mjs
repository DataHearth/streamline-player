import { empty, targets } from "./browse-lib.mjs";

export const target = targets.web.target;
export const mockArgs = ["--empty"];
export default (ctx) => empty(ctx, targets.web);
