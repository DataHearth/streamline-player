// A denied request says so and offers a new code.
import { body, codeOf, dev, openDesk, until, waitCode } from "./onboarding-lib.mjs";

export const target = "desktop";
export const mockArgs = [];

export default async ({ app, shot, wait }) => {
  const out = {};
  await openDesk(app);
  await waitCode(app, wait);
  const first = await codeOf(app);
  await dev(`deny/${first}`);
  await until(app, wait, async () => (await body(app)).includes("Request denied"), "the denial");
  out.denied = (await body(app)).replace([...first].join(" ").replace("-", "–"), "CODE");
  await shot("desk-pair-denied");
  await app.getByRole("button", { name: "New code" }).click();
  await until(app, wait, async () => (await codeOf(app)) !== first, "a fresh code");
  out.fresh = (await body(app)).includes("Waiting for approval…");
  out.noDenied = !(await body(app)).includes("Request denied");
  return out;
};
