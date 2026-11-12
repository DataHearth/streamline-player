// A code that runs out on screen is replaced at once, and the new one pairs.
import { approveOnScreen, body, codeOf, openDesk, until, waitCode } from "./onboarding-lib.mjs";

export const target = "desktop";
export const mockArgs = ["--pair-ttl", "4"];

export default async ({ app, shot, wait }) => {
  const out = {};
  await openDesk(app);
  await waitCode(app, wait);
  const first = await codeOf(app);
  const seen = [];
  await until(
    app,
    wait,
    async () => {
      seen.push(await app.locator(".expires").innerText().catch(() => ""));
      return (await codeOf(app).catch(() => first)) !== first;
    },
    "a new code",
    30,
  );
  out.replaced = (await codeOf(app)) !== first;
  out.neverDead = !seen.some((t) => /0:00/.test(t));
  out.status = (await body(app)).includes("Waiting for approval…");
  await shot("desk-pair-expired");
  await approveOnScreen(app, wait);
  out.pairedAfterReplace = (await body(app)).includes("You're paired");
  return out;
};
