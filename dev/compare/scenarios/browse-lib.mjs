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
  const out = {
    billboards: await app.locator(".hero").count(),
    rows: await app.locator(".rows section").count(),
    title: await app.locator(".home h2, .home div[class*=title]").first().textContent(),
    text: (await app.locator(".home").innerText()).replace(/\s+/g, " ").trim(),
  };
  for (const kind of ["movies", "series"]) {
    await app.goto(`${side.url}/library/${kind}`);
    await wait(2500);
    if (kind === "movies") await shot(`${side.target}-empty-library`);
    out[`library_${kind}`] = (await app.locator(".library").innerText()).replace(/\s+/g, " ").trim();
  }
  return out;
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

const grid = (app) => app.locator(".library .grid > .poster");
const firstTitles = (app, n) => app.locator(".library .poster .title, .library .row .title").evaluateAll((els, k) => els.slice(0, k).map((e) => e.textContent), n);
const searchOf = (app) => app.evaluate(() => location.search);

export async function library(ctx, side) {
  const { app, shot, wait } = ctx;
  await app.setViewportSize({ width: 1440, height: 900 });
  await side.open(ctx);
  await app.goto(`${side.url}/library/movies`);
  await wait(2500);
  await shot(`${side.target}-library`);
  const out = { heading: await app.locator(".library h1").textContent(), counter: await app.locator(".library .count").textContent() };
  out.cards = await grid(app).count();
  await app.locator('.library .flags button:text-is("4K")').click();
  await wait(600);
  out.uhd = { cards: await grid(app).count(), query: await searchOf(app) };
  await app.locator('.library .flags button:text-is("4K")').click();
  await wait(400);
  await app.locator(".library .genre-btn").click();
  await wait(500);
  await shot(`${side.target}-library-genre`);
  const options = await app.locator('.library [role="option"]').allTextContents();
  out.genreOptions = options.map((o) => o.replace(/\s+/g, " ").trim());
  await app.locator('.library .fmenu input').fill("dra");
  await wait(300);
  out.genreSearch = (await app.locator('.library [role="option"]').allTextContents()).map((o) => o.replace(/\s+/g, " ").trim());
  await app.locator('.library [role="option"]').first().click();
  await wait(700);
  out.genre = { cards: await grid(app).count(), query: await searchOf(app), button: (await app.locator(".library .genre-btn").textContent()).trim() };
  await app.locator(".library .sort .trigger").click();
  await wait(500);
  await shot(`${side.target}-library-sort`);
  await app.locator('.library [role="menuitemradio"]:text-is("Title")').click();
  await wait(700);
  out.sortedTitles = await firstTitles(app, 3);
  out.sortQuery = await searchOf(app);
  await app.locator('.library .seg button[aria-label="Details list"]').click();
  await wait(800);
  await shot(`${side.target}-library-list`);
  out.list = { rows: await app.locator(".library .table .row").count(), query: await searchOf(app), headers: await app.locator(".library .th").allTextContents() };
  await app.reload();
  await wait(2500);
  out.reloaded = { rows: await app.locator(".library .table .row").count(), first: (await firstTitles(app, 1))[0], query: await searchOf(app) };
  await app.locator('.library .flags button:text-is("In progress")').click();
  await wait(600);
  out.nothing = { rows: await app.locator(".library .table .row").count(), title: (await app.locator(".library .empty .title").textContent()) };
  await app.keyboard.press("g");
  await app.keyboard.press("s");
  await wait(1500);
  out.shows = { path: await app.evaluate(() => location.pathname), query: await searchOf(app), heading: await app.locator(".library h1").textContent(), cards: await grid(app).count() };
  await shot(`${side.target}-library-shows`);
  const labels = () => app.locator('.library [aria-label^="Play Severance"], .library [aria-label^="Mark Severance"], .library [aria-label^="Mark Shōgun"]').evaluateAll((els) => els.map((e) => e.getAttribute("aria-label")));
  out.showsLabels = await labels();
  await app.locator('.library [aria-label="Mark Shōgun as watched"]').click();
  await wait(1200);
  await app.keyboard.press("g");
  await app.keyboard.press("h");
  await wait(1500);
  await app.keyboard.press("g");
  await app.keyboard.press("s");
  await wait(2500);
  out.showsRevisited = await labels();
  out.layoutToggle = await app.locator(".library .seg").evaluate((el) => {
    const r = el.getBoundingClientRect();
    const b = el.querySelector("button").getBoundingClientRect();
    return [r.width, r.height, b.width, b.height].map(Math.round);
  });
  return out;
}
