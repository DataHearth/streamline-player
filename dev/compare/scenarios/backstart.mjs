// Back while start_session is in flight must stop the session it started and never load it.
export default async ({ app, wait }) => {
  await app.addInitScript(() => { window.__holdStart = true; });
  await app.goto("http://localhost:1420/player/movies/1003");
  await wait(2000);
  const held = await app.evaluate(() => typeof window.__releaseStart === "function");
  // The first Back only hides the controls when they are up.
  let presses = 0;
  while (presses < 3 && app.url().includes("/player/")) {
    await app.keyboard.press("Escape");
    presses++;
    await wait(800);
  }
  const url = app.url();
  await app.evaluate(() => { window.__calls.length = 0; window.__releaseStart(); });
  await wait(2500);
  const calls = await app.evaluate(() => window.__calls);
  return { held, presses, url, stopped: calls.includes("stop_session"), loaded: calls.includes("plugin:player|load") };
};
