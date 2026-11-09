export default async ({ app, wait, key }) => {
  await app.goto("http://localhost:1420/person/pedro-pascal");
  await wait(2500);
  await key("ArrowDown", 2);
  return app.evaluate(() => { const t = document.activeElement.querySelector(".t"); const cs = getComputedStyle(t); const b = getComputedStyle(document.activeElement); return { color: cs.color, opacity: cs.opacity, btnOpacity: b.opacity, filter: b.filter, card: document.activeElement.className }; });
};
