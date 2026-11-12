// The pair step on the page's own timers (no fake clock): the code shows, and the countdown moves.
import { body, codeOf, openDesk } from "./onboarding-lib.mjs";

export const target = "desktop";
export const mockArgs = [];
export const realClock = true;

export default async ({ app, log }) => {
  const out = {};
  await openDesk(app);
  await app.locator('.code[role="img"]').waitFor({ timeout: 15000 });
  out.codeShape = /^[A-Z0-9]{3}-[A-Z0-9]{3}$/.test(await codeOf(app));
  const left = async () => (await body(app)).match(/expires in (\d+):(\d\d)/)?.slice(1).reduce((m, s) => m * 60 + Number(s));
  const first = await left();
  await app.waitForTimeout(2500);
  out.countdownMoved = first > (await left());
  out.pageErrors = log.filter((l) => l.startsWith("PAGEERR"));
  if (!out.codeShape || !out.countdownMoved || out.pageErrors.length) throw new Error(JSON.stringify(out));
  return out;
};
