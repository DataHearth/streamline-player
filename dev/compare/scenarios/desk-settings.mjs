import { settings, targets } from "./browse-lib.mjs";

export const target = targets.desktop.target;
export const mockArgs = [];
export default (ctx) => settings(ctx, targets.desktop);
