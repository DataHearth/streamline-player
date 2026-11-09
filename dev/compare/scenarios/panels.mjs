export default async ({ app, shot, wait, key, focused }) => {
  const out = {};
  for (const [name, idx] of [["audio", 2], ["quality", 3], ["speed", 4], ["stats", 5]]) {
    await app.goto("http://localhost:1420/player/movies/1002");
    await wait(3500);
    await app.addStyleTag({ content: "html,body{background:#000 url(http://127.0.0.1:8099/artwork/movies/1/backdrop.jpg) center/cover no-repeat !important}" });
    await key("Enter");
    for (let i = 0; i < idx; i++) await key("ArrowRight");
    out[name] = await focused();
    await key("Enter");
    await wait(500);
    await shot(`panel-${name}`);
  }
  return out;
};
