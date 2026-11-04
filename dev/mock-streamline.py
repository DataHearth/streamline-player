"""Stands in for Streamline's playback API until the server side lands.

Proxies /api/v1/playback/* to the engine's control plane and mints the HLS
token like Streamline will (HMAC-SHA256 over "hls|<id>|<exp>"). No auth, no
watch state: the heartbeat is accepted and dropped.

    python3 dev/mock-streamline.py --secret <engine secret> \
        --media demo=/path/to/clip.mkv [--media other=/path ...]

The public URL defaults to 10.0.2.2, the Android emulator's alias for the host.
"""

import argparse
import hashlib
import hmac
import json
import re
import secrets
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

TOKEN_TTL = 6 * 3600

p = argparse.ArgumentParser()
p.add_argument("--listen", default="0.0.0.0:8080")
p.add_argument("--engine", default="http://127.0.0.1:8097")
p.add_argument("--public", default="http://10.0.2.2:8097", help="engine URL as the device sees it")
p.add_argument("--secret", required=True)
p.add_argument("--media", action="append", default=[], help="id=path, repeatable")
args = p.parse_args()
MEDIA = dict(m.split("=", 1) for m in args.media)


def engine(method, path, body=None):
    req = urllib.request.Request(
        args.engine + path,
        method=method,
        data=None if body is None else json.dumps(body).encode(),
        headers={"Authorization": f"Bearer {args.secret}", "Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            return r.status, json.loads(r.read() or b"null")
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read() or b"null")


def with_url(sid, resp):
    exp = int(time.time()) + TOKEN_TTL
    sig = hmac.new(args.secret.encode(), f"hls|{sid}|{exp}".encode(), hashlib.sha256).hexdigest()
    path = f"hls/{sid}/master.m3u8" if resp["url_path"] != "file" else f"file/{sid}"
    return {"session_id": sid, **resp, "url": f"{args.public}/{path}?exp={exp}&sig={sig}"}


class Handler(BaseHTTPRequestHandler):
    def reply(self, status, body=None):
        data = b"" if body is None else json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def body(self):
        n = int(self.headers.get("Content-Length") or 0)
        return json.loads(self.rfile.read(n) or b"{}")

    def do_GET(self):
        if self.path == "/api/v1/playback/status":
            st, _ = engine("GET", "/v1/health")
            return self.reply(200, {"available": st == 200, "media": sorted(MEDIA)})
        self.reply(404, {"error": {"code": "not_found"}})

    def do_POST(self):
        if self.path == "/api/v1/playback/sessions":
            b = self.body()
            path = MEDIA.get(b.pop("media_id", None))
            if path is None:
                return self.reply(404, {"error": {"code": "media_not_found"}})
            sid = secrets.token_hex(16)
            st, resp = engine("POST", "/v1/sessions", {"session_id": sid, "path": path, **b})
            return self.reply(st, with_url(sid, resp) if st < 300 else resp)
        if re.fullmatch(r"/api/v1/playback/sessions/[0-9a-f]{32}/progress", self.path):
            self.body()
            return self.reply(204)
        self.reply(404, {"error": {"code": "not_found"}})

    def do_PATCH(self):
        m = re.fullmatch(r"/api/v1/playback/sessions/([0-9a-f]{32})", self.path)
        if not m:
            return self.reply(404, {"error": {"code": "not_found"}})
        st, resp = engine("PATCH", f"/v1/sessions/{m[1]}", self.body())
        self.reply(st, with_url(m[1], resp) if st < 300 else resp)

    def do_DELETE(self):
        m = re.fullmatch(r"/api/v1/playback/sessions/([0-9a-f]{32})", self.path)
        if not m:
            return self.reply(404, {"error": {"code": "not_found"}})
        st, resp = engine("DELETE", f"/v1/sessions/{m[1]}")
        self.reply(st, resp)


host, port = args.listen.rsplit(":", 1)
print(f"mock streamline on {args.listen}, engine {args.engine}, media {sorted(MEDIA)}", flush=True)
ThreadingHTTPServer((host, int(port)), Handler).serve_forever()
