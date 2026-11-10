# Streamline Player

The player for Streamline (`../streamline`): a cross-platform client app and
the streaming engine that serves it. Streamline keeps all metadata (users,
sessions, watch state) and orchestrates; the engine only reads files and
serves HLS.

Docs are split by side. The client and engine designs live here
(`docs/superpowers/specs/2026-11-02-player-client-design.md`,
`docs/superpowers/specs/2026-11-02-player-engine-design.md`). Server contracts
live in `../streamline/docs/superpowers/specs/`: `2026-07-07-player-design.md`
(playback) and `2026-11-02-device-pairing-design.md` (login), with their plans
in `../streamline/docs/superpowers/plans/`. The server side is specified, not
built: code against those shapes, with a local mock server standing in until
it lands. Those server specs are outdated: this repo and the design are the
source of truth for the whole player, pairing included. When they disagree
with a server spec, build to ours and list the spec change for the user, who
updates `../streamline`.

## Stack

- **Tauri 2** shell in `src-tauri/` (root `Cargo.toml` is the workspace).
- **Engine** in `engine/` (planned): a Rust HTTP service driving external
  ffmpeg, VAAPI or CPU. Streamline is its only control-plane caller.
- **SvelteKit 3** (Svelte 5) UI in `src/`, as a static SPA: `adapter-static`
  with an `index.html` fallback, `ssr = false`, built to `build/`. Config lives
  in `vite.config.ts` (SvelteKit 3 has no `svelte.config.js`). No `load` from a
  server and no `+server.ts`: data comes from Rust via `invoke`. Components port
  from `../streamline/web/app` (plain Svelte + Routify); only routes differ.
- **Shaka Player + Media Chrome** for playback, as in the server spec.
- Targets: desktop (Linux/macOS/Windows), iOS/Android, Android TV.

## Constraints that shape the code

- **Every REST call goes through Rust**, never `fetch` from the webview.
  `/api/v1` sends no CORS headers, and the credential stays out of JavaScript.
  The webview only fetches HLS, from the engine: `/hls` has CORS and the
  playback token Streamline minted rides the URL.
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

The Taskfile holds every command; `task` lists them. Run them inside the
devshell (direnv), or `nix develop .#android` for the Android ones.

- `task dev:desktop` / `dev:web` / `dev:tv-web` / `dev:engine` / `dev:mock` /
  `dev:android` — run the app (Vite on :1420), the web build (Vite on :1430),
  the TV build in a browser (Vite on :1420, for the compare harness), the
  engine (:8097), the mock Streamline (:8081), the TV emulator
- `task build:<target>` — `engine`, `desktop:linux`, `desktop:windows`,
  `android`, `web`, `image`, … (`desktop:appimage` fails on NixOS by design)
- `task check` — everything CI runs: `fmt:check`, `lint` (clippy `-D warnings`,
  biome, svelte-check), `test`; `task fmt` writes the formatting
- `nix flake check` — the same checks plus package builds, the NixOS VM test
  and the home-manager evaluation
- Add JS deps with `pnpm add`, never by editing `package.json`. TypeScript stays
  on 6: svelte-check 4.7 refuses 7 without the tsgo dual install.
