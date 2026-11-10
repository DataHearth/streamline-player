# Stands in for the Rust side when the UI runs in a plain browser: forwards /api/v1 to the mock with CORS,
# through the contract proxy (dev/contract/proxy.ts) that run.sh starts.
import http.server, urllib.request, sys
MOCK = "http://127.0.0.1:8098"
class H(http.server.BaseHTTPRequestHandler):
    def cors(self):
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "*")
        self.send_header("Access-Control-Allow-Headers", "*")
    def do_OPTIONS(self):
        self.send_response(204); self.cors(); self.end_headers()
    def fwd(self):
        path = self.path
        if path.startswith("/raw/"): path = path[len("/raw/"):]
        elif path.startswith("/artwork/"): path = "/api/v1/posters/" + path[len("/artwork/"):]
        n = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(n) if n else None
        req = urllib.request.Request(MOCK + path if path.startswith("/") else path, data=body, method=self.command, headers={"Content-Type": "application/json"} if body else {})
        try:
            r = urllib.request.urlopen(req, timeout=60); code, data, ct = r.status, r.read(), r.headers.get("Content-Type", "")
        except urllib.error.HTTPError as e:
            code, data, ct = e.code, e.read(), e.headers.get("Content-Type", "")
        except Exception as e:
            code, data, ct = 502, str(e).encode(), "text/plain"
        self.send_response(code); self.cors()
        if ct: self.send_header("Content-Type", ct)
        self.send_header("Content-Length", str(len(data))); self.end_headers(); self.wfile.write(data)
    do_GET = do_POST = do_PUT = do_PATCH = do_DELETE = fwd
    def log_message(self, *a): pass
http.server.ThreadingHTTPServer(("127.0.0.1", 8099), H).serve_forever()
