export default async ({ app, wait, key, focused }) => {
  await app.goto("http://localhost:1420/detail/movies/1003"); await wait(2500);
  const f = await focused();
  await key("Enter"); await wait(3000);
  return { focus: f, url: app.url(), text: (await app.evaluate(() => document.body.innerText)).replace(/\s+/g, " ").slice(0, 200) };
};
