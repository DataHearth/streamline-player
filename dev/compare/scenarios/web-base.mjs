// A build under BASE_PATH=/watch, served on :1450 (dev/compare/web-base.sh): a deep link survives a reload.
const ORIGIN = "http://localhost:1450";
const BASE = `${ORIGIN}/watch`;

export const target = "web";
export const ownServer = true;
export const mockArgs = ["--cors-origin", ORIGIN];

export default async ({ app, wait }) => {
  await app.setViewportSize({ width: 1440, height: 900 });
  const out = {};
  await app.goto(`${BASE}/signin?next=${encodeURIComponent("/detail/movies/3")}`);
  await app.getByRole("button", { name: "Continue with Authentik" }).click();
  await app.waitForURL(`${BASE}/detail/movies/3`, { timeout: 15000 });
  await wait(1500);
  out.afterSignIn = app.url();
  await app.reload();
  await wait(2000);
  out.afterReload = app.url();
  out.title = await app.title();
  out.heading = (await app.locator("h1").first().innerText()).trim();
  return out;
};
