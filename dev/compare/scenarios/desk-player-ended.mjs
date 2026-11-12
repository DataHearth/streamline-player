// The two ended screens of the desktop player, with autoplay off. Expected: an episode ends on "Up next · S1 · E3" with
// "Play S1 E3" and "Done"; a film ends on "Finished", "Done", "Watch again" and six posters; "Watch again" restarts at 0;
// no key but Escape acts on an end screen.
import { fake, mockArgs, open, prepare, shown, target, text, where } from "./player-lib.mjs";

export { mockArgs, target };

export default async (ctx) => {
  const { app, shot, wait, key } = ctx;
  await prepare(ctx, { autoplay: false });
  const out = {};
  const toEnd = async () => {
    await app.evaluate(() => { const p = window.__fake.p; p.pos = p.dur - 4; window.__fakeTick = true; window.__fake.tickOn(); });
    await wait(5000);
  };
  const summary = () => app.evaluate(() => ({
    eyebrow: document.querySelector(".end .eyebrow")?.textContent.trim() ?? null,
    title: document.querySelector(".end .head")?.textContent.trim() ?? null,
    synopsis: document.querySelector(".end .syn")?.textContent.trim() ?? null,
    buttons: [...document.querySelectorAll(".end .actions button")].map((b) => b.textContent.trim()),
    posters: document.querySelectorAll(".end .poster").length,
    more: document.querySelector(".end .more-title")?.textContent.trim() ?? null,
    focus: document.activeElement?.textContent.trim().slice(0, 30) ?? null,
  }));

  await open(ctx, "/player/series/1?ep=2");
  await toEnd();
  await shot("desk-player-ended-episode");
  out.episode = await summary();
  // The focused button would take a Space or Enter itself; the keys under test are the player's.
  await app.evaluate(() => document.activeElement?.blur());
  await key(" ");
  await key("ArrowLeft");
  await key("f");
  await wait(400);
  out.keysIgnored = { playing: (await fake(app)).playing, fullscreen: (await fake(app)).fullscreen, url: await where(app), still: (await summary()).title };
  await app.getByRole("button", { name: "Play S1 E3" }).click();
  await wait(3000);
  out.playNext = await where(app);

  await open(ctx, "/player/movies/1");
  await toEnd();
  await shot("desk-player-ended-film");
  out.film = await summary();
  await app.getByRole("button", { name: "Watch again" }).click();
  await wait(2500);
  out.again = { ended: await app.locator(".end").count(), restarted: (await fake(app)).pos < 30 };
  await toEnd();
  await key("Escape");
  await wait(1500);
  out.escape = { url: await where(app), ended: await app.locator(".end").count() };
  return out;
};
