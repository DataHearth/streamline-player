// One-shot walkthrough: node walk.mjs <browser> <scenario>. Saves screenshots and the scenario JSON to $SHOTS.
import { mkdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
const require = createRequire(import.meta.url);
const pw = require(
  process.env.PLAYWRIGHT_CORE ??
    "/nix/store/aj40wwnxy1jjd4vrb1pp0l0ch7v3h4fi-playwright-test-1.64.1/lib/node_modules/playwright-core",
);
const [, , engine = "webkit", scenario = "home"] = process.argv;
const browser = await pw[engine].launch({ headless: true });
const ctx = await browser.newContext({ viewport: { width: 1920, height: 1080 } });
const app = await ctx.newPage();
await app.addInitScript({ path: fileURLToPath(new URL("./qrcode-init.js", import.meta.url)) });
await app.addInitScript({ path: fileURLToPath(new URL("./shim.js", import.meta.url)) });
await app.clock.install({ time: new Date("2026-11-09T12:00:00Z") });
const log = [];
app.on("console", (m) => m.type() === "error" && log.push(m.text()));
app.on("pageerror", (e) => log.push("PAGEERR " + e.message));
const dir = process.env.SHOTS ?? "/tmp/streamline-player/shots";
mkdirSync(dir, { recursive: true });
const shot = (name) => app.screenshot({ path: `${dir}/${engine}-${name}.png`, animations: "disabled", mask: [app.locator(".count, .upnext-timer, .thumb, .times")] });
const STEP = 100;
let inflight = 0;
app.on("request", () => inflight++);
app.on("requestfinished", () => inflight--);
app.on("requestfailed", () => inflight--);
const idle = async () => {
  for (let i = 0; i < 50 && inflight > 0; i++) await app.waitForTimeout(10);
};
// The page's timers run on a virtual clock that only moves while no request is in flight, so
// timing does not depend on how fast the mock answers.
const wait = async (ms) => {
  for (let left = ms; left > 0; left -= STEP) {
    await idle();
    await app.clock.runFor(Math.min(STEP, left));
    await app.waitForTimeout(100);
  }
};
const goto = app.goto.bind(app);
app.goto = async (...a) => {
  const r = await goto(...a);
  await app.waitForTimeout(2500);
  return r;
};
const key = async (k, n = 1) => { for (let i = 0; i < n; i++) { await app.keyboard.press(k); await wait(350); } };
const focused = () => app.evaluate(() => { const a = document.activeElement; return a ? (a.getAttribute("aria-label") || a.getAttribute("data-fid") || a.textContent.trim().slice(0, 40)) : null; });
const steps = (await import(`./scenarios/${scenario}.mjs`)).default;
try {
  const out = JSON.stringify(await steps({ app, shot, wait, key, focused, log }), null, 1).replace(/\b(in|dans) \d+ s\b/g, "$1 N s");
  writeFileSync(`${dir}/${engine}-${scenario}.json`, `${out}\n`);
  console.log(out);
} finally {
  if (log.length) console.log("console errors:", [...new Set(log)].slice(0, 15));
  await browser.close();
}
