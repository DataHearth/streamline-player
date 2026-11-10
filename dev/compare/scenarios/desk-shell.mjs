// The desktop shell inside the app (Tauri shim, Linux window chrome). Expected: menu ["Settings", "Keyboard shortcuts",
// "Sign out"], the sheet open and then closed, focus after Ctrl K recorded, and the URL back at "/" after g then h.
export const target = "desktop";

export default async ({ app, shot, wait, key }) => {
  await app.setViewportSize({ width: 1440, height: 900 });
  await app.goto("http://localhost:1440/");
  await wait(1000);
  await shot("desk-shell");
  const out = {};
  out.windowButtons = await app.locator('.win button, .gnome button').count();
  out.downloadsButton = await app.locator('button[aria-label="Downloads"]').count();
  await app.locator('.gnome button').first().click();
  out.minimizeCalled = await app.evaluate(() => window.__calls.includes("plugin:window|minimize"));
  await app.locator('button[aria-label="Account menu"]').click();
  await wait(400);
  out.menu = await app.locator(".sl-acct__label").allTextContents();
  await shot("desk-shell-menu");
  await key("Escape");
  await wait(300);
  out.menuClosed = (await app.locator(".sl-acct").count()) === 0;
  await key("?");
  await wait(400);
  out.sheetOpen = (await app.locator('[role="dialog"]').count()) > 0;
  await shot("desk-shell-sheet");
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
  return out;
};
