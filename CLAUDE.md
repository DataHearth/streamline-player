# Streamline Player

Cross-platform client for Streamline's built-in player (`../streamline`).

Docs are split by side. The client's design lives here
(`docs/superpowers/specs/2026-11-02-player-client-design.md`). Server contracts
live in `../streamline/docs/superpowers/specs/`: `2026-07-07-player-design.md`
(playback) and `2026-11-02-device-pairing-design.md` (login), with their plans
in `../streamline/docs/superpowers/plans/`. The server side is specified, not
built: code against those shapes, with a local mock server standing in until
it lands. When this repo and a server spec disagree, the server spec wins:
change it there first.

## Stack

- **Tauri 2** shell in `src-tauri/` (root `Cargo.toml` is the workspace).
- **Svelte 5** UI in `web/`, bundled by esbuild into `web/static/dist`, the same
  setup as `../streamline/web/app` so components and the design system port over.
- **Shaka Player + Media Chrome** for playback, as in the server spec.
- Targets: desktop (Linux/macOS/Windows), iOS/Android, Android TV.

## Constraints that shape the code

- **Every REST call goes through Rust**, never `fetch` from the webview.
  `/api/v1` sends no CORS headers, and the credential stays out of JavaScript.
  The webview only fetches HLS: `/hls` has CORS and the playback token rides
  the URL.
- **Auth is device pairing**, the app's only login on every platform. The
  `device` API key lives in OS secure storage, never in plaintext. No password
  ever passes through the app.
- **Capabilities come from the running webview** (`MediaCapabilities`,
  `isTypeSupported`, `matchMedia`), never from a per-platform table: the
  server picks copy vs transcode from them. WebKitGTK's codecs are whatever
  GStreamer plugins it finds, so a Linux bundle must ship them.
- **Out of reach in v1**: tvOS (no WKWebView; Tauri closed it as not planned,
  tauri-apps/tauri#6364), casting (the Cast Web Sender SDK runs only in
  Chrome), and AirPlay (deferred server-side).
- **UI gate**: before writing any UI, ask the user for the latest Claude-design
  artifact covering it, and implement it 1:1 (same rule as the server repo).

## Commands

- `cargo tauri dev` — run the app (starts esbuild in watch mode)
- `pnpm build` / `pnpm check` — bundle / svelte-check
- `lint` — clippy, `-D warnings`; `nix flake check` — rustfmt
- Add JS deps with `pnpm add`, never by editing `package.json`. TypeScript stays
  on 6: svelte-check 4.7 refuses 7 without the tsgo dual install.
