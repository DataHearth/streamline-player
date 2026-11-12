// No code, a wrong one and a used one: a field to type it, a clear message, and no Approve button.
import { WEB, approvalBody, hideCode, pollDevice, startDevice, typeCode } from "./onboarding-lib.mjs";
import { signInWithSso } from "./web-lib.mjs";

export const target = "web";
export const mockArgs = [];

const approveButtons = (app) => app.getByRole("button", { name: "Approve", exact: true }).count();

export default async ({ app, shot, wait }) => {
  await app.setViewportSize({ width: 1440, height: 900 });
  const out = {};
  const device = await startDevice();
  await signInWithSso(app, wait, "/pair");
  await wait(500);
  out.noCode = await approvalBody(app);
  out.noCodeApprove = await approveButtons(app);
  await shot("web-approve-no-code");
  await app.getByLabel("Pairing code").fill("k7f");
  await app.getByRole("button", { name: "Find device" }).click();
  await wait(300);
  out.shortCode = await approvalBody(app);
  await app.getByLabel("Pairing code").fill(typeCode(device.code));
  await app.getByRole("button", { name: "Find device" }).click();
  await wait(800);
  out.typed = hideCode(await approvalBody(app), device.code);
  out.typedApprove = await approveButtons(app);

  await app.goto(`${WEB}/pair?code=ZZZ-ZZZ`);
  await wait(500);
  out.unknown = await approvalBody(app);
  out.unknownApprove = await approveButtons(app);
  await shot("web-approve-not-found");

  await app.goto(`${WEB}/pair?code=${device.code}`);
  await wait(500);
  await app.getByRole("button", { name: "Approve", exact: true }).click();
  await wait(1200);
  out.device = await pollDevice(device);
  await app.goto(`${WEB}/pair?code=${device.code}`);
  await wait(500);
  out.used = await approvalBody(app);
  out.usedApprove = await approveButtons(app);
  return out;
};
