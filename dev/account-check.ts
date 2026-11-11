// node dev/account-check.ts
import { deepStrictEqual as eq } from "node:assert";
import {
	changed,
	PICTURE_MAX,
	PICTURE_TYPES,
	pictureProblem,
	refusal,
	validate,
} from "../src/lib/core/account.ts";
import { chips } from "../src/lib/core/capabilities.ts";
import type { Me } from "../src/lib/core/types.ts";

const me: Me = {
	name: "Antoine Langlois",
	email: "antoine@example.fr",
	role: "Admin",
	picture: false,
	initials: "AL",
};

eq(changed(me, { name: " Antoine Langlois ", email: me.email }), {});
eq(changed(me, { name: "A. Langlois", email: me.email }), { name: "A. Langlois" });
eq(changed(me, { name: me.name, email: " new@example.fr " }), { email: "new@example.fr" });
eq(validate({ name: "", email: me.email })?.field, "name");
eq(validate({ name: "   ", email: me.email })?.field, "name");
eq(validate({ name: "A", email: "nope" })?.field, "email");
eq(validate({ name: "A", email: "a@b" })?.field, "email");
eq(validate({ name: "A", email: "a@b.fr" }), null);
eq(refusal('{"error":{"code":"email_taken","message":"In use."}}'), {
	field: "email",
	message: "In use.",
});
eq(refusal('{"error":{"code":"invalid_email"}}').field, "email");
eq(refusal('{"error":{"code":"too_large"}}').field, "picture");
eq(refusal('{"error":{"code":"unsupported_type"}}').field, "picture");
eq(refusal("not json").field, "name");
eq(refusal('{"error":{"code":"boom"}}').field, "name");
eq(PICTURE_TYPES, ["image/jpeg", "image/png", "image/webp"]);
eq(PICTURE_MAX, 2 * 1024 * 1024);
eq(pictureProblem({ type: "image/png", size: 1024 }), null);
eq(pictureProblem({ type: "image/gif", size: 1024 }), "unsupported_type");
eq(pictureProblem({ type: "image/png", size: PICTURE_MAX + 1 }), "too_large");
const shim = {
	video: [{ codec: "h264" }, { codec: "hevc" }, { codec: "av1" }],
	hdr: { formats: ["hdr10", "hlg"] },
	audio: { codecs: ["aac", "ac3", "truehd", "dts", "flac"], passthrough: ["eac3"] },
	subtitles: ["srt", "ass"],
};
eq(
	chips(shim)
		.filter((c) => c.ok)
		.map((c) => c.name),
	["H.264", "HEVC", "AV1", "HDR10", "AAC", "AC3", "EAC3", "TrueHD", "DTS", "FLAC", "ASS", "SRT"],
);
eq(chips({ ...shim, subtitles: undefined }).length, 12);
console.log("account-check: ok");
