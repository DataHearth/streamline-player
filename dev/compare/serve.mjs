// node dev/compare/serve.mjs <dir> <base> <port>: serves a static build under a base path, index.html for any other route.
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, normalize } from "node:path";

const [, , dir, base = "", port = "1450"] = process.argv;
const types = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".json": "application/json", ".png": "image/png", ".woff2": "font/woff2" };

createServer(async (req, res) => {
	const path = decodeURIComponent(new URL(req.url, "http://x").pathname);
	if (!path.startsWith(`${base}/`) && path !== base) {
		res.writeHead(404).end();
		return;
	}
	const rel = normalize(path.slice(base.length)).replace(/^(\.\.[/\\])+/, "");
	for (const file of [join(dir, rel), join(dir, "index.html")]) {
		try {
			const body = await readFile(file);
			res.writeHead(200, { "content-type": types[extname(file)] ?? "application/octet-stream" }).end(body);
			return;
		} catch {}
	}
	res.writeHead(404).end();
}).listen(Number(port), "127.0.0.1");
