// Two tabs whose access tokens expire together refresh once between them and both stay signed in.
import { API, ORIGIN, here, refreshKey } from "./web-lib.mjs";
import { signInWithSso } from "./web-lib.mjs";

export const target = "web";
export const mockArgs = ["--access-ttl", "2"];

export default async ({ app, ctx, wait }) => {
  await app.setViewportSize({ width: 1440, height: 900 });
  const out = {};
  await signInWithSso(app, wait);
  const second = await ctx.newPage();
  await second.goto(`${ORIGIN}/`);
  await second.waitForTimeout(2000);
  await app.waitForTimeout(2500);
  await Promise.all([app.reload(), second.reload()]);
  await app.waitForTimeout(3000);
  out.first = await here(app);
  out.second = await here(second);
  out.refresh = (await refreshKey(app)) !== null;
  const device = await app.evaluate(() => localStorage.getItem("auth:device"));
  const list = await (await fetch(`${API}/devices`)).json();
  out.browserDevices = list.items.filter((d) => d.kind === "browser").length;
  out.deviceKept = list.items.some((d) => d.id === device);
  return out;
};
