// Shared by the web sign-in scenarios.
export const ORIGIN = "http://localhost:1430";
export const API = "http://localhost:8098/api/v1";

export const here = (app) => app.evaluate(() => location.pathname + location.search);
export const refreshKey = (app) => app.evaluate(() => localStorage.getItem("auth:refresh"));

export async function signInWithSso(app, wait, next = "/") {
  await app.goto(`${ORIGIN}/signin?next=${encodeURIComponent(next)}`);
  await app.getByRole("button", { name: "Continue with Authentik" }).click();
  await app.waitForURL(`${ORIGIN}${next}`, { timeout: 15000 });
  await wait(1500);
}
