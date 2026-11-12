// Shared by the onboarding scenarios: the desktop app's welcome flow behind the Tauri shim (:1440), and the web
// approval page (:1430). Pairings are started and approved through the mock, as the real devices and browsers do.
export const DESK = "http://localhost:1440";
export const MOCK = "http://127.0.0.1:8081";
const HOST = "streamline.home.arpa";

export const squash = (s) => s.replace(/\s+/g, " ").trim();
export const body = async (app, sel = ".page") => squash(await app.locator(sel).innerText());
export const codeOf = (app) => app.locator('.code[role="img"]').getAttribute("aria-label");
export const dev = (path) => fetch(`${MOCK}/dev/${path}`, { method: "POST" });

export async function until(app, wait, test, what, tries = 60, step = 500) {
  for (let i = 0; i < tries; i++) {
    if (await test()) return;
    await wait(step);
  }
  throw new Error(`timed out waiting for ${what}`);
}

/** Opens the desktop app: with `server` saved it starts on the pair step, without it on the server step. */
export async function openDesk(app, { server = HOST, down = false } = {}) {
  await app.setViewportSize({ width: 1440, height: 900 });
  await app.addInitScript(
    (a) => {
      window.__realPair = true;
      window.__pairDown = a.down;
      window.__deviceInfo = { platform: "Linux · GNOME" };
      window.__auth = { paired: false, server: a.server, keystore: "Secret Service (GNOME Keyring)" };
    },
    { server: server ? `https://${server}` : null, down },
  );
  await app.goto(`${DESK}/`);
}

export const waitCode = (app, wait) => until(app, wait, () => app.locator('.code[role="img"]').count(), "a code");

/** Approves the code on screen through the mock and waits for the paired step. */
export async function approveOnScreen(app, wait) {
  const code = await codeOf(app);
  await dev(`approve/${code}`);
  await until(app, wait, async () => (await body(app)).includes("You're paired"), "the paired step");
  return code;
}
