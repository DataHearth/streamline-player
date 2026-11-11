import { person, targets } from "./browse-lib.mjs";

export const target = targets.web.target;
export const mockArgs = [];
export default (ctx) => person(ctx, targets.web);
