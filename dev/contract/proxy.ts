// Forwards to the mock and checks each /api/v1 exchange against the OpenAPI contract.
//   node dev/contract/proxy.ts --listen 127.0.0.1:8098 --upstream http://127.0.0.1:8081 \
//     --spec docs/contract/player.openapi.yaml --report <file>
import Ajv from "ajv";
import { appendFileSync, readFileSync } from "node:fs";
import { createServer, request } from "node:http";
import { parseArgs } from "node:util";
import { parse } from "yaml";

const { values: opt } = parseArgs({
	options: {
		listen: { type: "string" },
		upstream: { type: "string" },
		spec: { type: "string" },
		report: { type: "string" },
	},
});
if (!opt.listen || !opt.upstream || !opt.spec || !opt.report) {
	console.error("usage: proxy.ts --listen host:port --upstream url --spec file --report file");
	process.exit(2);
}

type Json = any;
type Where = "request" | "response" | "undocumented";

const spec: Json = parse(readFileSync(opt.spec, "utf8"));
const base: string = spec.servers[0].url;

// The one gap between OpenAPI 3.0 and JSON Schema that the contract uses.
function nullable(node: Json) {
	if (Array.isArray(node)) return node.forEach(nullable);
	if (node === null || typeof node !== "object") return;
	if (node.nullable === true && node.type) {
		node.type = [node.type, "null"].flat();
		if (node.enum) node.enum = [...node.enum, null];
	}
	for (const v of Object.values(node)) nullable(v);
}
nullable(spec);

const ajv = new Ajv({ strict: false, allErrors: true });
const compiled = new WeakMap<object, ReturnType<typeof ajv.compile>>();
function validator(schema: object) {
	let v = compiled.get(schema);
	if (!v) {
		v = ajv.compile({ ...schema, components: spec.components });
		compiled.set(schema, v);
	}
	return v;
}
const problems = (v: ReturnType<typeof ajv.compile>) =>
	(v.errors ?? []).map((e) => `${e.instancePath || "/"} ${e.message}`);

const deref = (node: Json): Json => {
	const ref: string | undefined = node?.$ref;
	if (!ref) return node;
	return ref
		.slice(2)
		.split("/")
		.reduce((at: Json, part) => at[part], spec);
};

const templates = Object.keys(spec.paths).map((path) => {
	const names: string[] = [];
	const re = path.replace(/\{([^}]+)\}/g, (_, n) => {
		names.push(n);
		return "([^/]+)";
	});
	return { path, names, re: new RegExp(`^${re}$`) };
});

function route(pathname: string) {
	const exact = templates.find((t) => t.path === pathname);
	if (exact) return { template: exact, params: {} as Record<string, string> };
	for (const template of templates) {
		const m = template.re.exec(pathname);
		if (m) {
			const params: Record<string, string> = {};
			template.names.forEach((n, i) => {
				params[n] = decodeURIComponent(m[i + 1]);
			});
			return { template, params };
		}
	}
	return null;
}

function scalar(type: string | undefined, raw: string): unknown {
	if (type === "integer" || type === "number") return raw.trim() === "" ? raw : Number(raw);
	if (type === "boolean") return raw === "true" ? true : raw === "false" ? false : raw;
	return raw;
}

function checkParams(
	parameters: Json[],
	where: "path" | "query",
	values: (n: string) => string | undefined,
) {
	const errors: string[] = [];
	for (const p of parameters.map(deref).filter((p) => p.in === where)) {
		const raw = values(p.name);
		if (raw === undefined) {
			if (p.required) errors.push(`${where} parameter ${p.name} is missing`);
			continue;
		}
		const schema = deref(p.schema ?? {});
		const v = validator(schema);
		if (!v(scalar(schema.type, raw)))
			errors.push(`${where} parameter ${p.name} ${problems(v).join(", ")}`);
	}
	return errors;
}

const mediaType = (header: string | string[] | undefined) =>
	String(header ?? "")
		.split(";")[0]
		.trim()
		.toLowerCase();
const isJson = (type: string) => type === "application/json" || type.endsWith("+json");

type Exchange = {
	method: string;
	pathname: string;
	search: URLSearchParams;
	requestType: string;
	requestBody: Buffer;
	status: number;
	responseType: string;
	responseBody: Buffer;
};

function check(x: Exchange): { where: Where; errors: string[] }[] {
	const found = route(x.pathname.slice(base.length) || "/");
	const op = found?.template && spec.paths[found.template.path][x.method.toLowerCase()];
	if (!found || !op)
		return [{ where: "undocumented", errors: ["no such operation in the contract"] }];
	const item = spec.paths[found.template.path];
	const parameters = [...(item.parameters ?? []), ...(op.parameters ?? [])];

	const requestErrors = [
		...checkParams(parameters, "path", (n) => found.params[n]),
		...checkParams(parameters, "query", (n) => x.search.get(n) ?? undefined),
	];
	const body = deref(op.requestBody);
	if (body) {
		const schema = body.content?.["application/json"]?.schema;
		if (x.requestBody.length === 0) {
			if (body.required) requestErrors.push("body is missing");
		} else if (schema && isJson(x.requestType)) {
			try {
				const v = validator(schema);
				if (!v(JSON.parse(x.requestBody.toString()))) requestErrors.push(...problems(v));
			} catch (e) {
				requestErrors.push(`body is not JSON: ${(e as Error).message}`);
			}
		}
	} else if (x.requestBody.length > 0) {
		requestErrors.push("the operation takes no body");
	}

	const responseErrors: string[] = [];
	const declared = deref(op.responses[String(x.status)] ?? op.responses.default);
	if (!declared) {
		responseErrors.push(`status ${x.status} is not in the contract`);
	} else if (declared.content) {
		const types = Object.keys(declared.content);
		if (!types.includes("*/*") && x.responseType && !types.includes(x.responseType))
			responseErrors.push(`content type ${x.responseType} is not one of ${types.join(", ")}`);
		const schema = declared.content["application/json"]?.schema;
		if (schema && isJson(x.responseType)) {
			try {
				const v = validator(schema);
				if (!v(JSON.parse(x.responseBody.toString() || "null")))
					responseErrors.push(...problems(v));
			} catch (e) {
				responseErrors.push(`body is not JSON: ${(e as Error).message}`);
			}
		}
	}
	return [
		...(requestErrors.length ? [{ where: "request" as const, errors: requestErrors }] : []),
		...(responseErrors.length ? [{ where: "response" as const, errors: responseErrors }] : []),
	];
}

const upstream = new URL(opt.upstream);
const HOP = new Set([
	"connection",
	"keep-alive",
	"transfer-encoding",
	"proxy-connection",
	"upgrade",
	"te",
	"trailer",
]);

const server = createServer((req, res) => {
	const chunks: Buffer[] = [];
	req.on("data", (c) => chunks.push(c));
	req.on("end", () => {
		const requestBody = Buffer.concat(chunks);
		const headers = Object.fromEntries(Object.entries(req.headers).filter(([k]) => !HOP.has(k)));
		headers.host = upstream.host;
		const out = request(
			{ host: upstream.hostname, port: upstream.port, method: req.method, path: req.url, headers },
			(back) => {
				const parts: Buffer[] = [];
				back.on("data", (c) => parts.push(c));
				back.on("end", () => {
					const responseBody = Buffer.concat(parts);
					const answer = Object.fromEntries(
						Object.entries(back.headers).filter(([k]) => !HOP.has(k)),
					);
					answer["content-length"] = String(responseBody.length);
					res.writeHead(back.statusCode ?? 502, answer);
					res.end(responseBody);
					const url = new URL(req.url ?? "/", "http://proxy");
					if (!url.pathname.startsWith(`${base}/`) || req.method === "OPTIONS") return;
					const lines = check({
						method: req.method ?? "GET",
						pathname: url.pathname,
						search: url.searchParams,
						requestType: mediaType(req.headers["content-type"]),
						requestBody,
						status: back.statusCode ?? 0,
						responseType: mediaType(back.headers["content-type"]),
						responseBody,
					});
					for (const l of lines)
						appendFileSync(
							opt.report as string,
							`${JSON.stringify({ method: req.method, path: url.pathname, status: back.statusCode, ...l })}\n`,
						);
				});
			},
		);
		out.on("error", (e) => {
			res.writeHead(502, { "content-type": "text/plain" });
			res.end(String(e));
		});
		out.end(requestBody);
	});
});

const [host, port] = (() => {
	const at = (opt.listen as string).lastIndexOf(":");
	return [(opt.listen as string).slice(0, at), Number((opt.listen as string).slice(at + 1))];
})();
server.listen(port, host, () => {
	const a = server.address() as { port: number };
	console.log(`listening on ${host}:${a.port}`);
});
