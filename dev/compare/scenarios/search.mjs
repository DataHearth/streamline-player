export default async ({ app, shot, wait, key, focused }) => {
  const r = {};
  await app.goto("http://localhost:1420/search");
  await wait(3000);
  r.idleFocus = await focused();
  await shot("search-idle");
  await key("ArrowRight"); await key("ArrowDown"); await key("Enter"); // i
  await key("ArrowRight", 2); await key("Enter"); // k? depends on grid
  r.afterTyping = await app.evaluate(() => document.querySelector(".q").textContent);
  await app.evaluate(() => {}); 
  await shot("search-typed");
  r.text = (await app.evaluate(() => document.body.innerText)).replace(/\s+/g, " ").slice(0, 400);
  await key("ArrowUp", 8);
  r.upToNav = await focused();
  await key("ArrowRight"); r.rightOfSearch = await focused();
  return r;
};
