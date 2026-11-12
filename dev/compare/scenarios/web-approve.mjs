// The approval page: the request, an explicit Approve click, and the device getting its key.
import { WEB, approvalBody, hideCode, pollDevice, startDevice } from "./onboarding-lib.mjs";
import { here, signInWithSso } from "./web-lib.mjs";

export const target = "web";
export const mockArgs = [];

export default async ({ app, shot, wait }) => {
  await app.setViewportSize({ width: 1440, height: 900 });
  const out = {};
  const device = await startDevice();
  await signInWithSso(app, wait, `/pair?code=${device.code}`);
  await wait(500);
  out.url = (await here(app)).replace(device.code, "CODE");
  out.request = hideCode(await approvalBody(app), device.code);
  out.code = await app.locator('.code[role="img"]').getAttribute("aria-label").then((c) => c === device.code);
  out.buttons = await app.locator("button").allInnerTexts();
  out.beforeClick = await pollDevice(device);
  await shot("web-approve-request");
  await app.getByRole("button", { name: "Approve", exact: true }).click();
  await wait(300);
  out.approving = (await app.getByRole("button", { name: "Approved" }).count()) === 1;
  await wait(1200);
  out.done = await approvalBody(app);
  await shot("web-approve-done");
  out.afterClick = await pollDevice(device);
  await app.getByRole("button", { name: "Back to Streamline" }).click();
  await app.waitForURL(`${WEB}/`, { timeout: 15000 });
  out.back = await here(app);
  return out;
};
