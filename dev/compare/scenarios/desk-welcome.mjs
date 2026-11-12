// The desktop app's onboarding with no saved server: the address, the pair step, the browser button, the paired step,
// and its three exits (the countdown, "Start watching", "Sign out").
import { approveOnScreen, body, codeOf, openDesk, squash, until, waitCode } from "./onboarding-lib.mjs";

export const target = "desktop";
export const mockArgs = [];

const spaced = (code) => [...code].join(" ").replace("-", "–");
const path = (app) => app.evaluate(() => location.pathname);

export default async ({ app, shot, wait }) => {
  const out = {};
  await openDesk(app, { server: null });
  out.landed = await path(app);
  out.serverStep = await body(app);
  out.focused = await app.evaluate(() => document.activeElement?.tagName);
  out.prefix = await app.locator(".scheme").innerText();
  out.placeholder = await app.locator("input").getAttribute("placeholder");
  await shot("desk-welcome-server");
  await app.keyboard.press("Enter");
  out.empty = await app.locator(".help").innerText();
  await app.keyboard.type("not an address!");
  await app.keyboard.press("Enter");
  out.bad = await app.locator(".help").innerText();
  await shot("desk-welcome-error");
  await app.locator("input").fill("");
  await app.keyboard.type("127.0.0.1:8081");
  out.typed = await app.locator("input").inputValue();
  await app.keyboard.press("Enter");
  await wait(200);
  out.checking = squash(await app.locator("button[type=submit]").innerText());
  await until(app, wait, () => app.locator(".found").count(), "the found row", 30, 50);
  out.found = squash(await app.locator(".found").innerText());
  out.connected = squash(await app.locator("button[type=submit]").innerText());
  await shot("desk-welcome-found");
  await waitCode(app, wait);
  const first = await codeOf(app);
  out.codeShape = /^[A-Z0-9]{3}-[A-Z0-9]{3}$/.test(first);
  out.pairStep = (await body(app)).replace(spaced(first), "CODE").replace(/expires in \d+:\d\d/, "expires in M:SS");
  out.qrAlt = await app.locator(".qr img").getAttribute("alt");
  await shot("desk-welcome-pair");
  await app.getByRole("button", { name: "Approve in this computer's browser" }).click();
  await wait(200);
  const opened = await app.evaluate(() => window.__opened);
  out.opened = opened?.map((u) => u.replace(first, "CODE"));
  out.openedComplete = opened?.length === 1 && opened[0].endsWith(`/pair?code=${first}`);
  out.afterBrowser = squash(await app.locator(".status").innerText()).replace(/expires in \d+:\d\d/, "expires in M:SS");
  await shot("desk-welcome-browser");
  await approveOnScreen(app, wait);
  out.paired = await body(app);
  out.goLabel = /^\d s$/.test(await app.locator(".secs").innerText());
  await shot("desk-welcome-paired");
  await until(app, wait, async () => (await path(app)) === "/", "the countdown to end", 40);
  out.afterCountdown = await path(app);

  await openDesk(app);
  await waitCode(app, wait);
  out.savedServerStart = (await body(app)).includes("streamline.home.arpa");
  await approveOnScreen(app, wait);
  await app.getByRole("button", { name: "Start watching" }).click();
  await wait(500);
  out.afterStartWatching = await path(app);

  await openDesk(app);
  await waitCode(app, wait);
  const before = await codeOf(app);
  await approveOnScreen(app, wait);
  await app.getByRole("button", { name: "Sign out" }).click();
  await waitCode(app, wait);
  out.afterSignOut = {
    step: /step 2 of 2/i.test(await body(app)),
    newCode: (await codeOf(app)) !== before,
    toast: squash(await app.locator(".toasts").innerText()),
  };
  return out;
};
