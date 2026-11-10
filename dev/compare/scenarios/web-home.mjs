import { home, targets } from "./browse-lib.mjs";

export const target = targets.web.target;
export const mockArgs = ["--demo-progress"];
export default (ctx) => home(ctx, targets.web);
