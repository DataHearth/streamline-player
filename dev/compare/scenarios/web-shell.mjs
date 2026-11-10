// The same shell in a browser: no window buttons, no downloads button, and below 900 px a note instead of the content.
export const target = "web";

export default async ({ app, shot, wait, key }) => {
  await app.setViewportSize({ width: 1440, height: 900 });
  await app.goto("http://localhost:1430/");
  await wait(1000);
  await shot("web-shell");
  const out = {};
  out.windowButtons = await app.locator('.win button, .gnome button').count();
  out.downloadsButton = await app.locator('button[aria-label="Downloads"]').count();
  await app.locator('button[aria-label="Account menu"]').click();
  await wait(400);
  out.menu = await app.locator(".sl-acct__label").allTextContents();
  await shot("web-shell-menu");
  await key("Escape");
  await wait(300);
  await key("?");
  await wait(400);
  out.sheetOpen = (await app.locator('[role="dialog"]').count()) > 0;
  await shot("web-shell-sheet");
  await key("Escape");
  await wait(500);
  out.sheetClosed = (await app.locator('[role="dialog"]').count()) === 0;
  await key("Control+k");
  await wait(500);
  out.afterCtrlK = await app.evaluate(() => {
    const a = document.activeElement;
    return { url: location.pathname, tag: a?.tagName, label: a?.getAttribute("aria-label") ?? a?.textContent?.trim().slice(0, 40) };
  });
  await key("g");
  await key("h");
  await wait(500);
  out.url = await app.evaluate(() => location.pathname);
  await app.setViewportSize({ width: 800, height: 900 });
  await wait(500);
  out.narrow = (await app.locator("p").first().textContent())?.trim();
  await shot("web-shell-narrow");
  return out;
};
