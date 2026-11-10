// A tab left open past its access token's life loads the next screen after one silent refresh.
import { ORIGIN, here, signInWithSso } from "./web-lib.mjs";

export const target = "web";
export const mockArgs = ["--access-ttl", "2"];

export default async ({ app, wait }) => {
  await app.setViewportSize({ width: 1440, height: 900 });
  const out = {};
  await signInWithSso(app, wait);
  let toSignIn = false;
  app.on("framenavigated", (f) => {
    if (f === app.mainFrame() && f.url().includes("/signin")) toSignIn = true;
  });
  await app.waitForTimeout(3000);
  await app.getByRole("button", { name: "Movies" }).click();
  await app.waitForURL(`${ORIGIN}/library/movies`, { timeout: 15000 });
  await wait(1500);
  out.url = await here(app);
  out.sawSignIn = toSignIn;
  out.cards = await app.locator("img").count() > 0;
  return out;
};
