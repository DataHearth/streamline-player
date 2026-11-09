export default async ({ app, shot, wait }) => {
  const r = {};
  await app.addInitScript(() => { window.__failStart = JSON.stringify({ error: { code: "transcode_limit", message: "busy", retry_after: 2 } }); });
  await app.goto("http://localhost:1420/player/movies/1003");
  await wait(2500);
  await shot("halt-busy");
  r.busy = await app.evaluate(() => { const h = document.querySelector(".halt"); if (h) h.dataset.mark = "first"; return h?.innerText.replace(/\s+/g, " ").slice(0, 200); });
  await wait(4500);
  r.afterRetries = await app.evaluate(() => ({ same: document.querySelector(".halt")?.dataset.mark === "first", text: document.querySelector(".halt")?.innerText.replace(/\s+/g, " ").slice(0, 120), retried: window.__calls.filter((c) => c === "start_session").length >= 4 }));
  await app.evaluate(() => { window.__failStart = null; });
  await wait(2600);
  r.recovered = await app.evaluate(() => ({ halt: !!document.querySelector(".halt"), leaving: !!document.querySelector(".halt.leaving"), toast: document.body.innerText.match(/Conversion started|Reconnected[^\n]*/)?.[0] ?? null }));
  await wait(400);
  r.gone = await app.evaluate(() => !document.querySelector(".halt"));
  // unreachable: not JSON
  await app.evaluate(() => { window.__failStart = "error sending request"; });
  await app.goto("http://localhost:1420/player/movies/1003");
  await wait(2500);
  await shot("halt-unreachable");
  r.unreachable = await app.evaluate(() => document.querySelector(".halt")?.innerText.replace(/\s+/g, " ").slice(0, 200));
  await app.keyboard.press("Escape");
  await wait(1500);
  r.backFromHalt = app.url();
  return r;
};
