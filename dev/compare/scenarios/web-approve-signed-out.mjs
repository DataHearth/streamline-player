// A signed-out browser opening an approval link signs in first, and is back on the same code, still needing the click.
import { approvalBody, hideCode, pollDevice, startDevice } from "./onboarding-lib.mjs";
import { ORIGIN, here } from "./web-lib.mjs";

export const target = "web";
export const mockArgs = [];

export default async ({ app, wait }) => {
  await app.setViewportSize({ width: 1440, height: 900 });
  const out = {};
  const device = await startDevice();
  const link = `/pair?code=${device.code}`;
  await app.goto(`${ORIGIN}${link}`);
  await app.waitForURL(/\/signin/, { timeout: 15000 });
  await wait(500);
  out.gate = (await here(app)).replace(device.code, "CODE");
  await app.getByRole("button", { name: "Continue with Authentik" }).click();
  await app.waitForURL(`${ORIGIN}${link}`, { timeout: 15000 });
  await wait(1000);
  out.back = (await here(app)).replace(device.code, "CODE");
  out.request = hideCode(await approvalBody(app), device.code);
  out.device = await pollDevice(device);
  return out;
};
