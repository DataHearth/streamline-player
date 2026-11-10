// A refused or forged SSO return shows the error and stores nothing.
import { ORIGIN, here, refreshKey } from "./web-lib.mjs";

export const target = "web";
export const mockArgs = ["--sso-deny"];

export default async ({ app, wait }) => {
  await app.setViewportSize({ width: 1440, height: 900 });
  const out = {};
  await app.goto(`${ORIGIN}/signin`);
  await app.getByRole("button", { name: "Continue with Authentik" }).click();
  await app.waitForURL(/\/signin\?error=/, { timeout: 15000 });
  await wait(800);
  out.denied = (await app.getByRole("alert").innerText()).trim();
  out.deniedUrl = await here(app);
  await app.goto(`${ORIGIN}/signin/callback?code=x&state=forged`);
  await app.waitForURL(/\/signin\?error=/, { timeout: 15000 });
  await wait(800);
  out.forged = (await app.getByRole("alert").innerText()).trim();
  out.forgedUrl = await here(app);
  out.refresh = await refreshKey(app);
  return out;
};
