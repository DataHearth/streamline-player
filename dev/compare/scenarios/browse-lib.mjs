// The Home scenarios, run on the desktop target (:1440 behind the Tauri shim) and on the web target (:1430, after SSO).
import { signInWithSso } from "./web-lib.mjs";

export const targets = {
  desktop: { target: "desktop", url: "http://localhost:1440", open: (ctx) => ctx.app.goto("http://localhost:1440/") },
  web: { target: "web", url: "http://localhost:1430", open: (ctx) => signInWithSso(ctx.app, ctx.wait) },
};

const MOCK = "http://127.0.0.1:8081";
const art = (state) => fetch(`${MOCK}/dev/art/${state}`, { method: "POST" });
const pathOf = (app) => app.evaluate(() => location.pathname + location.search);

export async function home(ctx, side) {
  const { app, shot, wait } = ctx;
  await app.setViewportSize({ width: 1440, height: 900 });
  await side.open(ctx);
  await wait(1500);
  // The billboard rotates on the page's clock, which also runs in real time: start from its first slide, held by the pointer.
  await app.locator('.hero button[aria-label^="Show "]').first().click();
  await wait(1200);
  await shot(`${side.target}-home`);
  const out = {};
  out.rows = await app.locator(".rows section h2").allTextContents();
  out.billboard = {
    title: await app.locator(".hero h1").textContent(),
    eyebrow: (await app.locator(".hero .eyebrow").textContent())?.trim(),
    primary: (await app.locator(".hero .cta").textContent())?.trim(),
    dots: await app.locator('.hero button[aria-label^="Show "]').count(),
  };
  out.continueCards = await app.locator(".rows section").first().locator(".resume").count();
  await app.locator('.hero button[aria-label^="Show "]').nth(1).click();
  await wait(1500);
  out.secondBillboard = {
    title: await app.locator(".hero h1").textContent(),
    eyebrow: (await app.locator(".hero .eyebrow").textContent())?.trim(),
  };
  await app.evaluate(() => document.querySelector(".hero").closest(".content").scrollTo(0, 99999));
  await wait(600);
  await shot(`${side.target}-home-scrolled`);
  await app.evaluate(() => document.querySelector(".hero").closest(".content").scrollTo(0, 0));
  await wait(600);
  await app.locator(".rows .resume").first().click();
  await wait(1500);
  out.playUrl = await pathOf(app);
  await app.goBack();
  await wait(2500);
  await app.locator(".rows .poster").first().click();
  await wait(1500);
  out.posterUrl = await pathOf(app);
  return out;
}

export async function empty(ctx, side) {
  const { app, shot, wait } = ctx;
  await app.setViewportSize({ width: 1440, height: 900 });
  await side.open(ctx);
  await wait(2500);
  await shot(`${side.target}-empty`);
  return {
    billboards: await app.locator(".hero").count(),
    rows: await app.locator(".rows section").count(),
    title: await app.locator(".home h2, .home div[class*=title]").first().textContent(),
    text: (await app.locator(".home").innerText()).replace(/\s+/g, " ").trim(),
  };
}

const fresh = (app) => app.evaluate(() => [...document.images].filter((i) => i.offsetParent !== null && i.naturalWidth === 0).length);
const posterBox = (app) => app.locator(".rows .poster .art").first().evaluate((el) => {
  const r = el.getBoundingClientRect();
  return [Math.round(r.width), Math.round(r.height)];
});

export async function noart(ctx, side) {
  const { app, shot, wait } = ctx;
  await app.setViewportSize({ width: 1440, height: 900 });
  await side.open(ctx);
  await wait(2500);
  const before = await posterBox(app);
  await art("off");
  await app.goto(`${side.url}/`);
  await wait(4500);
  await shot(`${side.target}-noart`);
  const out = {
    brokenImages: await fresh(app),
    posterBoxBefore: before,
    posterBoxAfter: await posterBox(app),
    posterCards: await app.locator(".rows .poster").count(),
  };
  await art("on");
  return out;
}
