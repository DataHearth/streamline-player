// The desktop player with the fake plugin. Expected: the page is a hole over the picture; top bar, one scrubber segment
// per chapter, the control row in the design's order; controls and cursor hide after 3.2 s and a move brings them back;
// pause shows the card; a click on the picture toggles; scrub hover and click seek; volume and mute reach the player;
// skip intro, up next and watch credits; the subtitle lift follows the controls; Back ends the session.
import { bar, CHAPTERS, fake, jump, mockArgs, open, prepare, shown, stage, target, text, where } from "./player-lib.mjs";

export { mockArgs, target };

export default async (ctx) => {
  const { app, shot, wait, key } = ctx;
  await prepare(ctx);
  const out = {};
  await open(ctx, "/player/series/1?ep=2");
  await shown(ctx);
  await shot("desk-player-playing");
  out.holes = await app.evaluate(() => ({
    shell: getComputedStyle(document.querySelector(".shell")).backgroundColor,
    stage: getComputedStyle(document.querySelector(".stage")).backgroundColor,
    body: getComputedStyle(document.body).backgroundColor,
  }));
  out.top = { name: await text(app, ".top .name"), sub: await text(app, ".top .sub"), stats: await app.locator(".top .pill").getAttribute("aria-label"), mode: await text(app, ".top .pill"), back: await app.locator(".top .back").getAttribute("aria-label") };
  out.segments = { count: await app.locator(".scrub .seg").count(), chapters: CHAPTERS.length };
  out.controls = await bar(app);
  out.time = await text(app, ".bar .time");
  out.lift = (await fake(app)).lift;

  await wait(3600);
  out.hidden = await stage(app);
  out.liftHidden = (await fake(app)).lift;
  await app.mouse.move(600, 400);
  await wait(500);
  out.shownAgain = await stage(app);

  await jump(ctx, 10);
  await key("c");
  await wait(800);
  await shot("desk-player-cue");
  out.cue = { text: await text(app, ".cue"), bottom: await app.evaluate(() => getComputedStyle(document.querySelector(".cue-wrap")).bottom) };
  await shown(ctx);
  await app.locator(".bar .big").click();
  await wait(800);
  await shot("desk-player-paused");
  out.paused = { playing: (await fake(app)).playing, card: await text(app, ".paused"), cue: await app.locator(".cue").count() };
  await app.mouse.click(720, 300);
  out.centreFlash = await app.evaluate(() => document.querySelector(".pf .disc")?.className.split(" ")[1]);
  await wait(250);
  out.afterSurfaceClick = (await fake(app)).playing;
  await wait(1200);

  const track = await app.locator(".scrub").boundingBox();
  await app.mouse.move(track.x + track.width * 0.05, track.y + track.height / 2);
  await wait(300);
  out.hoverFirst = await text(app, ".peek .when");
  await app.mouse.move(track.x + track.width * 0.5, track.y + track.height / 2);
  await wait(300);
  await shot("desk-player-scrub");
  out.hover = { when: await text(app, ".peek .when"), frame: await app.locator(".peek img").count() };
  await app.mouse.down();
  await app.mouse.up();
  await wait(600);
  out.seeked = Math.round((await fake(app)).pos / 10) * 10;

  const slider = await app.locator(".vol .slider").boundingBox();
  await app.mouse.click(slider.x + slider.width / 2, slider.y + slider.height / 2);
  await wait(300);
  out.volume = (await fake(app)).volume;
  await app.locator(".vol .mute").click();
  await wait(300);
  out.muted = (await fake(app)).volume;
  await app.locator(".vol .mute").click();
  await wait(300);

  await jump(ctx, 40);
  await app.mouse.move(600, 420);
  await wait(900);
  out.skip = await text(app, ".slot");
  await app.getByRole("button", { name: "Skip intro" }).click();
  await wait(600);
  out.afterSkip = Math.round((await fake(app)).pos);

  await jump(ctx, 2942);
  await app.mouse.move(620, 430);
  await wait(900);
  await shot("desk-player-upnext");
  out.upNext = await text(app, ".up");
  await app.getByRole("button", { name: "Watch credits" }).click();
  await wait(500);
  out.upNextAfter = await app.locator(".up").count();

  await app.mouse.move(600, 400);
  await wait(300);
  await app.locator(".top .back").click();
  await wait(1500);
  out.after = { url: await where(app), stopped: await app.evaluate(() => window.__calls.includes("stop_session")), playing: (await fake(app)).playing };
  return out;
};
