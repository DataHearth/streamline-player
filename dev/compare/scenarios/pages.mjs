export default async ({ app, shot, wait }) => {
  const out = {};
  for (const [name, url] of [["home", "/"], ["library-movies", "/library/movies"], ["library-shows", "/library/series"], ["detail-movie", "/detail/movies/1"], ["detail-show", "/detail/series/1"], ["player", "/player/movies/1"]]) {
    await app.goto("http://localhost:1420" + url);
    await wait(name === "player" ? 5000 : 3000);
    await shot(name);
    out[name] = (await app.evaluate(() => document.body.innerText)).replace(/\s+/g, " ").slice(0, 400);
  }
  return out;
};
