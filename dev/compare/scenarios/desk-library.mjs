import { library, targets } from "./browse-lib.mjs";

export const target = targets.desktop.target;
export const mockArgs = ["--demo-progress"];
export default (ctx) => library(ctx, targets.desktop);
