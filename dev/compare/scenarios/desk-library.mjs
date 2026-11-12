import { library, targets } from "./browse-lib.mjs";

export const target = targets.desktop.target;
export const mockArgs = ["--demo-progress", "--clip", `${process.env.MEDIA_DIR ?? "/tmp/streamline-player"}/multi.mkv`];
export default (ctx) => library(ctx, targets.desktop);
