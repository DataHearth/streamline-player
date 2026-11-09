export default async ({ app, shot, wait }) => {
  const r = {};
  const started = async () => {
    for (let i = 0; i < 50 && !(await app.evaluate(() => window.__fake.p.playing && window.__fake.p.dur > 0)); i++) await app.waitForTimeout(100);
    await wait(500);
  };
  await app.goto("http://localhost:1420/player/series/1?ep=2");
  await wait(3000);
  await started();
  await app.addStyleTag({ content: "html,body{background:#000 url(http://127.0.0.1:8099/artwork/series/1/backdrop.jpg) center/cover no-repeat !important}" });
  r.dur = await app.evaluate(() => window.__fake.p.dur);
  await app.evaluate(() => { const p = window.__fake.p; p.pos = Math.max(0, p.dur - 12); window.__fakeTick = true; window.__fake.tickOn(); });
  await wait(4000);
  await shot("upnext");
  r.upnext = await app.evaluate(() => document.querySelector(".sl-upnext")?.innerText.replace(/\s+/g, " ") ?? document.body.innerText.replace(/\s+/g, " ").slice(0, 300));
  r.focusUpnext = await app.evaluate(() => document.activeElement?.textContent.trim().slice(0, 40));
  await wait(10000);
  await shot("ended");
  r.after = [app.url(), await app.evaluate(() => document.querySelector(".sl-upnext")?.innerText.replace(/\s+/g, " ") ?? null)];
  // a film's end
  await app.goto("http://localhost:1420/player/movies/1003");
  await wait(3000);
  await started();
  await app.evaluate(() => { const p = window.__fake.p; p.pos = Math.max(0, p.dur - 2); window.__fakeTick = true; window.__fake.tickOn(); });
  await wait(5000);
  await shot("film-end");
  r.filmEnd = await app.evaluate(() => ({ text: document.body.innerText.replace(/\s+/g, " ").slice(0, 300), focus: document.activeElement?.textContent.trim().slice(0, 40) }));
  return r;
};
