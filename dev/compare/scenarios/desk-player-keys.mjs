// The player's keys on the desktop. Expected: Space, the arrows, M, C, I, N, P and F act as the design says, with their
// OSD words; Escape closes full screen first and leaves second; Ctrl K and g h do nothing while the player is up.
import { fake, jump, mockArgs, open, prepare, shown, stage, target, text, where } from "./player-lib.mjs";

export { mockArgs, target };

export default async (ctx) => {
  const { app, shot, wait, key } = ctx;
  await prepare(ctx);
  const out = {};
  await open(ctx, "/player/series/1?ep=2");
  await shown(ctx);
  const osd = async () => text(app, ".osd");
  // The OSD lasts 900 ms of a clock that also runs in real time, so it is read right after the press.
  const flash = async (k, n = 1) => {
    for (let i = 0; i < n; i++) { await app.keyboard.press(k); await app.waitForTimeout(40); }
    return osd();
  };

  const before = (await fake(app)).playing;
  await key(" ");
  await wait(300);
  out.space = { before, after: (await fake(app)).playing };
  await key(" ");
  await wait(600);
  await jump(ctx, 500);
  await key("ArrowRight");
  await wait(300);
  out.forward = Math.round((await fake(app)).pos);
  await key("ArrowLeft", 2);
  await wait(300);
  out.back = Math.round((await fake(app)).pos);
  out.volumeUp = await flash("ArrowUp");
  out.volumeDown = await flash("ArrowDown", 2);
  out.volumeSent = (await fake(app)).volume;
  out.muteOn = await flash("m");
  out.muteSent = (await fake(app)).volume;
  out.muteOff = await flash("m");
  await wait(1200);
  out.subsOn = await flash("c");
  await wait(1200);
  out.subsOff = await flash("c");
  await wait(1200);
  await key("i");
  await wait(400);
  out.stats = await app.locator(".top .pill.on").count();
  await key("i");
  await wait(400);
  out.statsOff = await app.locator(".top .pill.on").count();

  await key("f");
  await wait(500);
  out.fullscreen = { on: (await fake(app)).fullscreen, label: await app.locator('.bottom button[aria-label$="full screen"]').getAttribute("aria-label") };
  await shot("desk-player-fullscreen");
  await key("Control+k");
  await wait(500);
  out.ctrlK = await where(app);
  await key("g");
  await key("h");
  await wait(500);
  out.goHome = await where(app);
  await key("Escape");
  await wait(500);
  out.escapeOne = { fullscreen: (await fake(app)).fullscreen, url: await where(app) };

  await key("n");
  await wait(2500);
  out.next = await where(app);
  await key("p");
  await wait(2500);
  out.previous = await where(app);
  await key("p");
  await wait(2500);
  out.previousOfFirst = await where(app);
  await shown(ctx);
  await key("Escape");
  await wait(1500);
  out.escapeTwo = { url: await where(app), stopped: await app.evaluate(() => window.__calls.includes("stop_session")) };
  return out;
};
