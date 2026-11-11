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
  await app.goto(`${side.url}/search`);
  await wait(2500);
  await shot(`${side.target}-empty-search`);
  out.search = (await app.locator(".search").innerText()).replace(/\s+/g, " ").trim();
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

const text = async (loc) => (await loc.innerText()).replace(/\s+/g, " ").trim();
const rect = (loc) => loc.evaluate((el) => { const r = el.getBoundingClientRect(); return [r.width, r.height].map(Math.round); });

export async function detail(ctx, side) {
  const { app, shot, wait } = ctx;
  await app.setViewportSize({ width: 1440, height: 900 });
  await side.open(ctx);
  await app.goto(`${side.url}/detail/movies/1`);
  await wait(2500);
  await shot(`${side.target}-detail-movie`);
  const out = { movie: {} };
  out.movie.title = await app.locator(".detail h1").textContent();
  out.movie.primary = (await app.locator(".detail .cta").textContent())?.trim();
  out.movie.meta = await text(app.locator(".detail .meta"));
  out.movie.restart = await app.locator(".detail .restart").count();
  out.movie.cast = await app.locator(".detail .castc").count();
  out.movie.similar = await app.locator(".detail .similar > button").count();
  await app.locator(".detail .tech-toggle").click();
  await wait(700);
  await shot(`${side.target}-detail-movie-tech`);
  out.movie.techRows = await app.locator(".detail .tech .k").allTextContents();
  out.movie.audioTracks = await app.locator(".detail .tech .tracks div").count();
  await app.locator(".detail .castc").first().click();
  await wait(1500);
  out.castUrl = await pathOf(app);
  await app.goBack();
  await wait(2500);
  await app.locator(".detail .similar > button").first().click();
  await wait(1500);
  out.similarUrl = await pathOf(app);
  await app.goto(`${side.url}/detail/movies/2`);
  await wait(3500);
  await app.locator('.detail [data-lang-trigger="hero"]').click();
  await wait(700);
  await shot(`${side.target}-detail-movie-lang`);
  out.lang = {
    title: await app.locator('.detail [role="dialog"]').getAttribute("aria-label"),
    audio: await app.locator('.detail [role="dialog"] .audio button').allTextContents(),
    subs: await app.locator('.detail [role="dialog"] .sub').allTextContents(),
    hint: await text(app.locator('.detail [role="dialog"] .hint')),
  };
  await app.locator('.detail [role="dialog"] .sub').filter({ hasText: "Off" }).click();
  await app.locator('.detail [role="dialog"] .apply').click();
  await wait(600);
  out.lang.closed = await app.locator('.detail [role="dialog"]').count();
  out.lang.label = await app.locator('.detail [data-lang-trigger="hero"]').getAttribute("aria-label");
  await app.reload();
  await wait(2500);
  out.lang.afterReload = await app.locator('.detail [data-lang-trigger="hero"]').getAttribute("aria-label");

  await app.goto(`${side.url}/detail/series/1`);
  await wait(2500);
  await shot(`${side.target}-detail-show`);
  out.show = {};
  await app.evaluate(() => { const el = document.querySelector(".detail .season-note"); el.scrollIntoView({ block: "start" }); el.closest(".content").scrollBy(0, -96); });
  await wait(500);
  await shot(`${side.target}-detail-show-eps`);
  await app.evaluate(() => document.querySelector(".detail").closest(".content").scrollTo(0, 0));
  await wait(300);
  out.show.title = await app.locator(".detail h1").textContent();
  out.show.primary = (await app.locator(".detail .cta").textContent())?.trim();
  out.show.seasons = await app.locator(".detail .seasons button").allTextContents();
  out.show.selected = await app.locator('.detail .seasons button[aria-pressed="true"]').textContent();
  out.show.note = (await app.locator(".detail .season-note").textContent())?.trim();
  out.show.episodes = await app.locator(".detail .ep").count();
  out.show.states = await app.locator(".detail .ep").evaluateAll((els) => [...new Set(els.map((e) => e.getAttribute("data-state")))].sort());
  out.show.firstEpisode = await text(app.locator(".detail .ep").first());
  out.show.newBadges = await app.locator(".detail .ep .new").count();
  out.show.cast = await app.locator(".detail .castc").count();
  out.show.castTitle = await text(app.locator(".detail .cast-head"));
  await app.locator(".detail .seasons button").first().click();
  await wait(700);
  await shot(`${side.target}-detail-show-s1`);
  out.show.s1 = { query: await app.evaluate(() => location.search), episodes: await app.locator(".detail .ep").count(), castTitle: await text(app.locator(".detail .cast-head")) };
  await app.locator(".detail .ep .mark").first().click();
  await wait(700);
  out.show.epToast = await text(app.locator('[role="status"]'));
  await app.locator(".detail .season-mark").click();
  await wait(700);
  out.show.seasonToast = await text(app.locator('[role="status"]'));
  await app.locator(".detail .season-mark").click();
  await wait(300);
  await app.locator('.detail [data-lang-trigger="season"]').click();
  await wait(700);
  await shot(`${side.target}-detail-show-lang`);
  out.show.lang = { title: await app.locator('.detail [role="dialog"]').getAttribute("aria-label"), hint: await text(app.locator('.detail [role="dialog"] .hint')) };
  await app.keyboard.press("Escape");
  await wait(500);
  out.show.langClosed = await app.locator('.detail [role="dialog"]').count();
  await app.goto(`${side.url}/detail/series/4?s=2`);
  await wait(2500);
  await shot(`${side.target}-detail-wanted`);
  out.wanted = { states: await app.locator(".detail .ep").evaluateAll((els) => els.map((e) => e.getAttribute("data-state"))), text: await text(app.locator('.detail .ep[data-state="wanted"]')), playable: await app.locator('.detail .ep[data-state="wanted"] .play-ep').count() };
  await app.locator(".detail .play-ep").first().click();
  await wait(1500);
  out.show.playUrl = await pathOf(app);
  return out;
}

export async function watched(ctx, side) {
  const { app, shot, wait } = ctx;
  await app.setViewportSize({ width: 1440, height: 900 });
  await side.open(ctx);
  await wait(2500);
  const out = {};
  const continuing = () => app.locator(".rows section").first().locator(".resume .title").evaluateAll((els) => els.map((e) => e.firstChild.textContent.trim()));
  out.before = await continuing();
  const movie = out.before.find((t) => t === "Dune: Part Two") ?? out.before[0];
  let loads = 0;
  app.on("load", () => loads++);
  await app.locator(".rows section").first().locator(".resume").filter({ hasText: movie }).click();
  await wait(1200);
  await app.keyboard.press("Escape");
  out.opened = await pathOf(app);
  await app.evaluate(() => history.back());
  await wait(1500);
  await app.locator(".rows .poster", { hasText: movie }).first().click();
  await wait(2000);
  out.detailPath = await pathOf(app);
  await app.locator(".detail .mark-title").click();
  await wait(700);
  out.toast = await text(app.locator('[role="status"]'));
  await shot(`${side.target}-detail-watched`);
  await app.keyboard.press("g");
  await app.keyboard.press("h");
  await wait(1500);
  out.after = await continuing();
  out.goneFromContinue = !out.after.includes(movie);
  out.reloads = loads;
  return out;
}

export async function missing(ctx, side) {
  const { app, shot, wait } = ctx;
  await app.setViewportSize({ width: 1440, height: 900 });
  await side.open(ctx);
  const out = {};
  for (const [name, path] of [["direct", "/detail/movies/999999"], ["series", "/detail/series/999999"]]) {
    await app.goto(`${side.url}${path}`);
    await wait(2500);
    if (name === "direct") await shot(`${side.target}-detail-missing`);
    out[name] = { text: await text(app.locator(".detail .empty")), button: await app.locator(".detail .empty button").textContent() };
    if (name === "direct") {
      await app.reload();
      await wait(2500);
      out.reloaded = await text(app.locator(".detail .empty"));
      await app.locator(".detail .empty button").click();
      await wait(1500);
      out.backTo = await pathOf(app);
    }
  }
  await app.goto(`${side.url}/person/nobody`);
  await wait(2500);
  await shot(`${side.target}-person-missing`);
  out.person = { text: await text(app.locator(".person .empty")), button: await app.locator(".person .empty button").textContent() };
  await app.reload();
  await wait(2500);
  out.personReloaded = await text(app.locator(".person .empty"));
  await app.locator(".person .empty button").click();
  await wait(1500);
  out.personBackTo = await pathOf(app);
  return out;
}

const activeField = (app) => app.evaluate(() => {
  const a = document.activeElement;
  return { tag: a?.tagName, placeholder: a?.getAttribute("placeholder"), value: a?.value };
});
const resultTitles = (app) => app.locator(".search .results .poster .title").allTextContents();

export async function search(ctx, side) {
  const { app, shot, wait, key } = ctx;
  await app.setViewportSize({ width: 1440, height: 900 });
  await side.open(ctx);
  await wait(2000);
  await key("Control+k");
  await wait(900);
  const out = { opened: { url: await pathOf(app), focus: await activeField(app) } };
  await shot(`${side.target}-search`);
  out.recent = await app.locator(".search .recent button").allTextContents();
  out.genres = (await app.locator(".search .genre-tile").allTextContents()).map((t) => t.replace(/\s+/g, " ").trim());
  await app.keyboard.type("dun", { delay: 40 });
  await wait(900);
  await shot(`${side.target}-search-results`);
  out.typed = { url: await pathOf(app), titles: await resultTitles(app), count: await text(app.locator(".search .found")) };
  out.top = await text(app.locator(".search .top"));
  out.groups = await app.locator(".search .results h2").allTextContents();
  await app.reload();
  await wait(2500);
  out.reloaded = { url: await pathOf(app), field: (await activeField(app)).value ?? null, value: await app.locator("[data-search-field]").inputValue(), titles: await resultTitles(app) };
  await app.locator(".search .found").click();
  out.blurred = (await activeField(app)).tag;
  await key("Control+k");
  await wait(300);
  out.refocused = await activeField(app);
  await app.locator("[data-search-field]").fill("zzzz");
  await wait(900);
  await shot(`${side.target}-search-none`);
  out.none = { url: await pathOf(app), text: await text(app.locator(".search .empty")) };
  await app.locator("[data-search-field]").fill("");
  await wait(600);
  out.cleared = await pathOf(app);
  await app.locator("[data-search-field]").focus();
  await key("Escape");
  await wait(1500);
  out.escape = await pathOf(app);
  await key("Control+k");
  await wait(700);
  await app.keyboard.type("severance", { delay: 30 });
  await wait(900);
  await app.locator(".search .results .poster .hit").first().click();
  await wait(1500);
  out.opened_result = await pathOf(app);
  await app.goBack();
  await wait(1500);
  out.back = { url: await pathOf(app), value: await app.locator("[data-search-field]").inputValue() };
  await app.locator("[data-search-field]").fill("");
  await wait(500);
  out.recentAfter = await app.locator(".search .recent button").allTextContents();
  await app.locator(".search .genre-tile").first().click();
  await wait(900);
  out.genreClick = { url: await pathOf(app), titles: (await resultTitles(app)).length };
  return out;
}

export async function person(ctx, side) {
  const { app, shot, wait } = ctx;
  await app.setViewportSize({ width: 1440, height: 900 });
  await side.open(ctx);
  await app.goto(`${side.url}/detail/movies/1`);
  await wait(3000);
  await app.locator(".detail .castc").first().click();
  await wait(2000);
  await shot(`${side.target}-person`);
  const out = { url: await pathOf(app) };
  out.name = await app.locator(".person h1").textContent();
  out.department = (await app.locator(".person .eyebrow").textContent())?.trim();
  out.summary = (await app.locator(".person .sum").textContent())?.trim();
  out.known = await app.locator(".person .known-item").evaluateAll((els) => els.map((e) => e.className.includes("out") ? "out" : "on"));
  out.sections = await text(app.locator(".person .credits").first());
  out.credits = await app.locator(".person .credits .poster").count();
  out.titles = await app.locator(".person .credits .poster .title").allTextContents();
  await app.locator(".person .credits .poster .hit").first().click();
  await wait(1500);
  out.creditUrl = await pathOf(app);
  await app.goBack();
  await wait(1500);
  await app.locator(".person .back").click();
  await wait(1200);
  out.backTo = await pathOf(app);
  await app.goto(`${side.url}/person/zendaya`);
  await wait(2500);
  await app.locator(".person .known-item:not(.out)").first().click();
  await wait(1500);
  out.knownUrl = await pathOf(app);
  await app.goto(`${side.url}/person/britt-lower`);
  await wait(2500);
  await shot(`${side.target}-person-series`);
  out.series = {
    summary: (await app.locator(".person .sum").textContent())?.trim(),
    sections: await app.locator(".person .credits h2").allTextContents(),
    subs: await app.locator(".person .credits .poster .sub").allTextContents(),
  };
  await app.locator(".person .credits .poster .hit").first().click();
  await wait(1500);
  out.series.url = await pathOf(app);
  return out;
}
