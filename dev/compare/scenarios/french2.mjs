export default async ({ app, shot, wait, key }) => {
  const r = {};
  await app.addInitScript(() => localStorage.setItem("PARAGLIDE_LOCALE", "fr"));
  const text = () => app.evaluate(() => document.body.innerText.replace(/\s+/g, " ").slice(0, 300));
  await app.goto("http://localhost:1420/library/series"); await wait(3000); await shot("fr-library"); r.library = await text();
  await app.goto("http://localhost:1420/player/movies/1"); await wait(5000); await shot("fr-player"); r.player = await text();
  await key("ArrowUp"); await wait(600); await shot("fr-player-controls"); r.controls = await text();
  await app.evaluate(() => [...document.querySelectorAll("button")].find((b) => /Sous-titres|Audio/.test(b.textContent))?.click()); await wait(800);
  await shot("fr-player-menu"); r.menu = await text();
  return r;
};
