import { person, targets } from "./browse-lib.mjs";

export const target = targets.desktop.target;
export const mockArgs = [];
export default (ctx) => person(ctx, targets.desktop);
