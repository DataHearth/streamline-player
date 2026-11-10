import { home, targets } from "./browse-lib.mjs";

export const target = targets.desktop.target;
export const mockArgs = ["--demo-progress"];
export default (ctx) => home(ctx, targets.desktop);
