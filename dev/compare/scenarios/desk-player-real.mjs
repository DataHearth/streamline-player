// The desktop player on the page's own timers (no fake clock): controls and cursor hide after 3.2 s idle and a move
// brings them back, the OSD goes away by itself, the subtitle lift follows the controls, and leaving by the browser's
// back ends the session.
import { fake, mockArgs, open, prepare, stage, target, text, where } from "./player-lib.mjs";

export { mockArgs, target };
export const realClock = true;

export default async (ctx) => {
  const { app, log } = ctx;
  const wait = (ms) => app.waitForTimeout(ms);
  await prepare(ctx);
  const out = {};
  await app.goto("http://localhost:1440/detail/movies/1");
  await app.getByRole("button", { name: /^Play/ }).first().click();
  for (let i = 0; i < 80 && !(await app.evaluate(() => window.__fake?.p.playing)); i++) await wait(100);
  await wait(500);
  out.start = { ...(await stage(app)), lift: (await fake(app)).lift };
  await wait(4500);
  out.hidden = { ...(await stage(app)), lift: (await fake(app)).lift };
  await app.mouse.move(500, 300);
  await app.mouse.move(520, 310);
  await wait(600);
  out.moved = { ...(await stage(app)), lift: (await fake(app)).lift };
  await wait(4500);
  out.hiddenAgain = await stage(app);
  await app.keyboard.press("m");
  await wait(150);
  out.osd = await text(app, ".osd");
  await wait(1500);
  out.osdGone = await text(app, ".osd");
  await app.goBack();
  await wait(800);
  out.back = { url: await where(app), stopped: await app.evaluate(() => window.__calls.includes("stop_session")), playing: (await fake(app)).playing };
  out.pageErrors = log.filter((l) => l.startsWith("PAGEERR"));
  const ok =
    out.start.chrome === 1 && out.start.lift.px > 0 &&
    out.hidden.chrome === 0 && out.hidden.cursor === "none" && out.hidden.lift.px === 0 &&
    out.moved.chrome === 1 && out.moved.lift.px > 0 &&
    out.hiddenAgain.chrome === 0 &&
    out.osd === "Muted" && out.osdGone === null &&
    out.back.url.startsWith("/detail/movies/1") && out.back.stopped && !out.back.playing &&
    !out.pageErrors.length;
  if (!ok) throw new Error(JSON.stringify(out));
  return out;
};
