export default async ({ app, shot, wait, key, focused }) => {
  await app.goto("http://localhost:1420/");
  await wait(3500);
  await shot("home");
  const navBox = await app.evaluate(() => [...document.querySelectorAll("[data-tv-nav] button")].map((b) => { const r = b.getBoundingClientRect(); const s = getComputedStyle(b); return { t: b.textContent.trim(), h: r.height, top: r.top, font: s.fontSize, lh: s.lineHeight }; }));
  await key("ArrowUp");
  const up = await focused();
  await shot("nav-focus");
  return { navBox, up };
};
