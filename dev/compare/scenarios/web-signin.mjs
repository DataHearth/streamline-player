// Sign in on the web: the gate, the SSO round trip, a reload, sign-out, and the email form.
import { ORIGIN, here, refreshKey } from "./web-lib.mjs";

export const target = "web";

export default async ({ app, shot, wait }) => {
  await app.setViewportSize({ width: 1440, height: 900 });
  const out = {};
  await app.goto(`${ORIGIN}/library/movies`);
  await wait(500);
  out.gate = await here(app);
  await shot("web-signin-idle");
  await app.getByRole("button", { name: "Sign in with email" }).click();
  await wait(400);
  await shot("web-signin-email");
  await app.getByRole("button", { name: "Sign in with email" }).click();
  await app.getByRole("button", { name: "Continue with Authentik" }).click();
  await app.waitForURL(`${ORIGIN}/library/movies`, { timeout: 15000 });
  await wait(1500);
  out.afterSso = await here(app);
  await app.reload();
  await wait(2000);
  out.afterReload = await here(app);
  await app.locator('button[aria-label="Account menu"]').click();
  await wait(400);
  await app.getByText("Sign out", { exact: true }).click();
  await wait(600);
  out.afterSignOut = await here(app);
  out.toast = (await app.locator(".toasts").innerText()).replace(/\s+/g, " ").trim();
  out.refreshAfterSignOut = await refreshKey(app);
  await app.getByRole("button", { name: "Sign in with email" }).click();
  await app.getByLabel("Email").fill("antoine@example.net");
  await app.getByLabel("Password").fill("wrong");
  await app.getByRole("button", { name: "Sign in", exact: true }).click();
  await wait(800);
  out.wrongPassword = (await app.getByRole("alert").innerText()).trim();
  await shot("web-signin-error");
  await app.getByLabel("Password").fill("streamline");
  await app.getByRole("button", { name: "Sign in", exact: true }).click();
  await app.waitForURL(`${ORIGIN}/`, { timeout: 15000 });
  await wait(1000);
  out.afterEmail = await here(app);
  return out;
};
