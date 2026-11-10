// A browser signed out from another device lands on sign-in at its next call.
import { API, here, refreshKey, signInWithSso } from "./web-lib.mjs";

export const target = "web";

export default async ({ app, wait }) => {
  await app.setViewportSize({ width: 1440, height: 900 });
  const out = {};
  await signInWithSso(app, wait);
  const device = await app.evaluate(() => localStorage.getItem("auth:device"));
  const res = await fetch(`${API}/devices/${device}`, { method: "DELETE" });
  out.revoke = res.status;
  await app.waitForTimeout(500);
  await app.getByRole("button", { name: "Movies" }).click();
  await app.waitForURL(/\/signin/, { timeout: 15000 });
  await wait(800);
  out.url = await here(app);
  out.refresh = await refreshKey(app);
  return out;
};
