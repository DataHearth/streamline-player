import { paraglideVitePlugin } from "@inlang/paraglide-js";
import adapter from "@sveltejs/adapter-static";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";

// `cargo tauri android|ios dev` sets this to the host's LAN address: the app on
// the device loads the dev server over the network, not from localhost.
const host = process.env.TAURI_DEV_HOST;

// The target is fixed at build time: Tauri sets TAURI_ENV_PLATFORM, a plain Vite build leaves it unset.
const platform = process.env.TAURI_ENV_PLATFORM
	? "src/lib/platform/native.ts"
	: "src/lib/platform/web.ts";

export default defineConfig({
	envPrefix: ["VITE_", "TAURI_ENV_"],
	plugins: [
		// Same strategy as `pnpm check` and as Streamline: the Settings choice, else the TV's language.
		paraglideVitePlugin({
			project: "./project.inlang",
			outdir: "./src/lib/paraglide",
			strategy: ["localStorage", "preferredLanguage", "baseLocale"],
			isServer: "false",
		}),
		sveltekit({
			// Tauri serves files, not a Node server: an SPA with one fallback page.
			adapter: adapter({ fallback: "index.html" }),
			alias: { "#platform": platform },
		}),
	],
	clearScreen: false,
	server: {
		port: 1420,
		strictPort: true,
		host: host || false,
		hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
		watch: { ignored: ["**/src-tauri/**", "**/engine/**"] },
	},
});
