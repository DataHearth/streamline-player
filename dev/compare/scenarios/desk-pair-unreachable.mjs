// A saved server that stops answering at the pair step: an error page naming the host, no code and no QR frame.
import { body, codeOf, openDesk, until, waitCode } from "./onboarding-lib.mjs";

export const target = "desktop";
export const mockArgs = [];

const pairParts = (app) =>
  app.evaluate(() => ({ code: !!document.querySelector(".code"), qr: !!document.querySelector(".qr") }));

export default async ({ app, shot, wait }) => {
  const out = {};
  await openDesk(app, { down: true });
  await until(app, wait, async () => (await body(app)).includes("Can’t reach"), "the error page");
  out.error = await body(app);
  out.parts = await pairParts(app);
  out.buttons = await app.locator("button").allInnerTexts();
  await shot("desk-pair-unreachable");
  await app.getByRole("button", { name: "Try again" }).click();
  await wait(500);
  out.stillDown = (await body(app)).includes("Can’t reach") && !(await pairParts(app)).code;
  await app.getByRole("button", { name: "Change server" }).click();
  await wait(300);
  out.changeServer = /step 1 of 2/i.test(await body(app)) && (await app.locator("input").count()) === 1;
  out.partsAfterChange = await pairParts(app);

  await app.goto("http://localhost:1440/");
  await until(app, wait, async () => (await body(app)).includes("Can’t reach"), "the error page again");
  await app.evaluate(() => { window.__pairDown = false; });
  await app.getByRole("button", { name: "Try again" }).click();
  await waitCode(app, wait);
  out.recovered = /^[A-Z0-9]{3}-[A-Z0-9]{3}$/.test(await codeOf(app));
  await shot("desk-pair-recovered");
  return out;
};
