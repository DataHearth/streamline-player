import { library, targets } from "./browse-lib.mjs";

export const target = targets.web.target;
export const mockArgs = ["--demo-progress"];
export default (ctx) => library(ctx, targets.web);
