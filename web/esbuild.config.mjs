import esbuild from "esbuild";
import sveltePlugin from "esbuild-svelte";

const ctx = await esbuild.context({
	entryPoints: ["web/main.ts"],
	bundle: true,
	minify: true,
	format: "esm",
	outdir: "web/static/dist",
	tsconfig: "web/tsconfig.json",
	conditions: ["svelte", "browser", "module", "import"],
	mainFields: ["svelte", "browser", "module", "main"],
	plugins: [sveltePlugin({ filterWarnings: (w) => !w.filename?.includes("node_modules") })],
	logLevel: "info",
});

if (process.argv.includes("--watch")) {
	await ctx.watch();
} else {
	await ctx.rebuild();
	await ctx.dispose();
}
