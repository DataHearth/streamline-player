// Deny: the approver is told and leaves the page, and the device is refused.
import { WEB, pollDevice, startDevice } from "./onboarding-lib.mjs";
import { here, signInWithSso } from "./web-lib.mjs";

export const target = "web";
export const mockArgs = [];

export default async ({ app, wait }) => {
  await app.setViewportSize({ width: 1440, height: 900 });
  const out = {};
  const device = await startDevice();
  await signInWithSso(app, wait, `/pair?code=${device.code}`);
  await wait(500);
  await app.getByRole("button", { name: "Deny" }).click();
  await app.waitForURL(`${WEB}/`, { timeout: 15000 });
  await wait(300);
  out.toast = (await app.locator(".toasts").innerText()).replace(/\s+/g, " ").trim();
  out.url = await here(app);
  out.device = await pollDevice(device);
  return out;
};
