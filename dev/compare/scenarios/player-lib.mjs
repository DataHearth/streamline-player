// Shared by the desktop player scenarios: the desktop build (:1440) behind the Tauri shim with the fake player plugin.
export const target = "desktop";
export const mockArgs = ["--demo-markers", "--clip", `${process.env.MEDIA_DIR ?? "/tmp/streamline-player"}/multi.mkv`];

export const DURATION = 3000;
export const CHAPTERS = [
  { start: 0, end: 300, title: "Cold open" },
  { start: 300, end: 1500, title: "The first act" },
  { start: 1500, end: 2700, title: "The second act" },
  { start: 2700, end: 3000, title: "Aftermath" },
];

// A title the fake player runs for 3000 s, with chapters, an intro and a credits marker the real clips lack.
export async function prepare({ app }, { autoplay = true } = {}) {
  await app.setViewportSize({ width: 1440, height: 900 });
  await app.addInitScript(
    ({ chapters, duration, autoplay }) => {
      window.__picture = true;
      window.__fakeDuration = duration;
      const sub = (index, language) => ({ index, codec: "subrip", language, forced: false, sdh: false, files: { vtt: `fake:${language}` } });
      window.__tracks = { chapters, preparing: false, subtitles: [sub(1, "eng"), sub(2, "fre")], markers: { intro: [30, 75], credits: [duration - 60, duration] } };
      if (!autoplay) localStorage.setItem("prefs", JSON.stringify({ autoplay: false }));
    },
    { chapters: CHAPTERS, duration: DURATION, autoplay },
  );
}

export async function open({ app, wait }, path) {
  await app.goto(`http://localhost:1440${path}`);
  for (let i = 0; i < 50 && !(await app.evaluate(() => window.__fake?.p.playing && window.__fake.p.dur > 0)); i++) await app.waitForTimeout(100);
  await wait(1200);
}

export const fake = (app) => app.evaluate(() => ({ pos: window.__fake.p.pos, playing: window.__fake.p.playing, volume: window.__fake.volume ?? null, lift: window.__fake.lift ?? null, fullscreen: !!window.__fake.fullscreen }));
export const jump = async ({ app, wait }, seconds) => {
  await app.evaluate((s) => { const p = window.__fake.p; p.pos = s; window.__fake.send("position", { position: s, buffered: Math.min(p.dur, s + 30), duration: p.dur }); }, seconds);
  await wait(600);
};
export const text = (app, selector) => app.evaluate((s) => document.querySelector(s)?.innerText.replace(/\s+/g, " ").trim() ?? null, selector);
export const where = (app) => app.evaluate(() => location.pathname + location.search);
export const stage = (app) => app.evaluate(() => {
  const o = (s) => { const el = document.querySelector(s); return el ? Number(getComputedStyle(el).opacity) : null; };
  return { chrome: o(".chrome"), cursor: document.querySelector(".stage") ? getComputedStyle(document.querySelector(".stage")).cursor : null };
});
export const bar = (app) => app.locator(".bottom button").evaluateAll((els) => els.map((e) => e.getAttribute("aria-label")));
export const shown = async ({ app, wait }) => {
  await app.mouse.move(700, 450);
  await app.mouse.move(720, 460);
  await wait(500);
};
