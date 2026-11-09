export default async ({ app, shot, wait, key, focused }) => {
  await app.goto("http://localhost:1420/player/movies/1002");
  await wait(4000);
  await app.addStyleTag({ content: "html,body{background:#000 url(http://127.0.0.1:8099/artwork/movies/1/backdrop.jpg) center/cover no-repeat !important}" });
  await key("Enter");
  await key("ArrowRight");
  const f = await focused();
  await key("Enter");
  await wait(500);
  await shot("player-subs");
  const items = await app.evaluate(() => [...document.querySelectorAll("[data-menu-panel] button, [data-menu-panel] [data-tv]")].map((b) => b.textContent.trim().replace(/\s+/g, " ")));
  await key("Escape");
  await wait(400);
  for (let i = 0; i < 2; i++) await key("ArrowRight");
  await key("Enter");
  await wait(500);
  await shot("player-audio");
  return { f, items };
};
