import { detail, targets } from "./browse-lib.mjs";

export const target = targets.web.target;
export const mockArgs = ["--demo-progress", "--clip", `${process.env.MEDIA_DIR ?? "/tmp/streamline-player"}/multi.mkv`];
export default (ctx) => detail(ctx, targets.web);
