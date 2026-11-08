"""Stands in for Streamline's playback API until the server side lands.

Proxies /api/v1/playback/* to the engine's control plane and mints the HLS
token like Streamline will (HMAC-SHA256 over "hls|<id>|<exp>"). Pairing hands out
device keys; a request with an unknown key gets 401, one with no key passes (the
headless harness sends none). Approve, deny and revoke from the command line:
POST /dev/approve, /dev/deny, /dev/revoke. Watch state lives in memory. Files are analyzed and their subtitles extracted at start,
as Streamline does at import; a session opened before that finishes probes instead,
and its tracks read as still preparing.

The library is dev/mock-library.json, in Streamline's shapes, with its titles played
by the --clip files in turn. A --media id shows up as a movie of that name, playing its
own file. --art points at img/p/<slug>.jpg and img/b/<slug>.jpg artwork.

    python3 dev/mock-streamline.py --secret <engine secret> --art /path/to/img \
        --clip /path/to/clip.mkv [--clip ...] [--media film=/path/to/film.mkv ...]

The public URL defaults to 10.0.2.2, the Android emulator's alias for the host.
"""

import argparse
import hashlib
import hmac
import datetime
import itertools
import json
import os
import re
import secrets
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

TOKEN_TTL = 6 * 3600

p = argparse.ArgumentParser()
p.add_argument("--listen", default="0.0.0.0:8080")
p.add_argument("--engine", default="http://127.0.0.1:8097")
p.add_argument("--public", default="http://10.0.2.2:8097", help="engine URL as the device sees it")
p.add_argument("--secret", required=True)
p.add_argument("--media", action="append", default=[], help="id=path, repeatable")
p.add_argument("--clip", action="append", default=[], help="file the library's titles play, repeatable")
p.add_argument("--demo-markers", action="store_true", help="give episodes an intro and credits, to see the cards")
p.add_argument("--art", default="", help="directory holding the artwork")
p.add_argument("--network", choices=["home", "away"], default="away", help="where Streamline places this device; away is what a missing answer means")
p.add_argument("--heartbeat-timeout", type=float, default=90, help="seconds without a heartbeat before a session is stopped")
args = p.parse_args()
FILES = dict(m.split("=", 1) for m in args.media)
MEDIA = dict(FILES)
ANALYSIS = {}
SUBTITLES = {}
MARKERS = {}
SESSIONS = {}
LAST_BEAT = {}
# What an admin did, for the next heartbeat to tell: sid -> (by, message), and a restart's end time.
STOPPED = {}
RESTART = {"until": 0.0, "by": "", "message": ""}
ADMIN = "Antoine Langlois"
PAIRINGS = {}
KEYS = {}
PAIR_TTL = 600
PAIR_INTERVAL = 5
# The design's codes: 6 characters shown as XXX-XXX, no vowels, no 0/O or 1/I.
CODE_ALPHABET = "BCDFGHJKLMNPQRSTVWXZ23456789"
WATCH = {}
WATCHED_AT = 0.9

with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "mock-library.json")) as f:
    LIBRARY = json.load(f)
MOVIES = LIBRARY["movies"]
SERIES = LIBRARY["series"]
REFS = {}
clips = itertools.cycle(args.clip or list(FILES.values()))
for number, media_id in enumerate(FILES):
    MOVIES.insert(number, {
        "id": 1000 + number, "slug": media_id, "title": media_id, "original_title": media_id, "year": 2026,
        "overview": "", "runtime": 0, "status": "available", "monitored": True, "tmdb_id": 0, "genres": ["Test"],
        "rating": 0, "added_at": datetime.datetime.now(datetime.UTC).isoformat(), "file_summary": {"file_count": 1},
        "media_files": [{"id": media_id}],
    })
for movie in MOVIES:
    media_id = movie["media_files"][0]["id"]
    MEDIA.setdefault(media_id, next(clips))
    REFS[media_id] = {"movie_id": movie["id"]}
for show in SERIES:
    for season in show["seasons"]:
        for episode in season["episodes"]:
            MEDIA.setdefault(episode["media_file_id"], next(clips))
            REFS[episode["media_file_id"]] = {"series_id": show["id"], "season": season["number"], "episode_id": episode["id"]}


def summary(item):
    return {k: v for k, v in item.items() if k not in ("media_files", "seasons")}


def art(kind, item_id, name):
    items = MOVIES if kind == "movies" else SERIES
    slug = next((i["slug"] for i in items if str(i["id"]) == item_id), None)
    folder = {"poster": "p", "backdrop": "b"}[name]
    path = os.path.join(args.art, folder, f"{slug}.jpg")
    return path if slug and os.path.exists(path) else None


def note_progress(sid, position):
    media_id = SESSIONS.get(sid)
    if media_id is None:
        return
    analysis = ANALYSIS.get(MEDIA[media_id], {})
    duration = analysis.get("duration", 0)
    WATCH[media_id] = {
        **REFS.get(media_id, {}), "media_id": media_id, "position_seconds": position, "duration_seconds": duration,
        "watched": duration > 0 and position >= WATCHED_AT * duration,
        "updated_at": datetime.datetime.now(datetime.UTC).isoformat(),
    }


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


def run_job(kind, **params):
    job = secrets.token_hex(16)
    engine("POST", "/v1/jobs", {"job_id": job, "type": kind, "priority": "background", **params})
    while True:
        st, status = engine("GET", f"/v1/jobs/{job}")
        if st != 200 or status["state"] == "failed":
            print(f"{kind} {params}: {status}", flush=True)
            return None
        if status["state"] == "done":
            print(f"{kind} done: {params}", flush=True)
            return job, status["result"]
        time.sleep(1)


def prepare(path):
    done = run_job("analyze", path=path)
    if done:
        ANALYSIS[path] = done[1]
    done = run_job("subtitles", path=path)
    if done:
        SUBTITLES[path] = done


def prepare_season(paths):
    done = run_job("intro", paths=paths)
    for found in done[1] if done else []:
        MARKERS[found["path"]] = found


def markers(media_id, path):
    if args.demo_markers and "episode_id" in REFS.get(media_id, {}):
        duration = ANALYSIS.get(path, {}).get("duration", 0)
        return {"intro": [2, 7], "credits": [duration - 8, duration]}
    found = MARKERS.get(path, {})
    return {"intro": found.get("intro"), "credits": found.get("credits")}


def tracks(media_id, path, base):
    analysis = ANALYSIS.get(path, {})
    job, result = SUBTITLES.get(path, (None, {"tracks": [], "fonts": []}))
    url = lambda name: f"{base}/api/v1/playback/media/{media_id}/subtitles/{urllib.parse.quote(name)}"
    files = {t["index"]: {f.rsplit(".", 1)[1]: url(f) for f in t["files"]} for t in result["tracks"]}
    return {
        "preparing": job is None,
        "container": analysis.get("container", ""),
        "video": next(iter(analysis.get("video", [])), None),
        "chapters": analysis.get("chapters", []),
        "markers": markers(media_id, path),
        "audio": analysis.get("audio", []),
        "subtitles": [{**s, "files": files.get(s["index"], {})} for s in analysis.get("subtitles", [])],
        "fonts": [url(f) for f in result["fonts"]],
    }


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

    def refused(self):
        key = self.headers.get("X-API-Key")
        if key is None or key in KEYS:
            return False
        self.reply(401, {"error": {"code": "unauthorized"}})
        return True

    def pairing(self, code):
        """The pairing a /dev route names, or the newest one still waiting."""
        waiting = [p for p in PAIRINGS.values() if p["status"] == "pending" and (code is None or p["user_code"] == code)]
        return max(waiting, key=lambda p: p["expires"], default=None)

    def do_GET(self):
        if self.refused():
            return
        if self.path == "/health":
            return self.reply(200, {"status": "ok"})
        if self.path == "/api/v1/movies":
            return self.reply(200, {"items": [summary(m) for m in MOVIES], "total": len(MOVIES)})
        if self.path == "/api/v1/series":
            return self.reply(200, {"items": [summary(s) for s in SERIES], "total": len(SERIES)})
        m = re.fullmatch(r"/api/v1/(movies|series)/(\d+)", self.path)
        if m:
            found = next((i for i in (MOVIES if m[1] == "movies" else SERIES) if str(i["id"]) == m[2]), None)
            return self.reply(200, found) if found else self.reply(404, {"error": {"code": "not_found"}})
        if self.path == "/api/v1/watchstate":
            return self.reply(200, {"items": sorted(WATCH.values(), key=lambda w: w["updated_at"], reverse=True)})
        m = re.fullmatch(r"/api/v1/posters/(movies|series)/(\d+)/(poster|backdrop)\.jpg", self.path)
        if m:
            path = art(m[1], m[2], m[3])
            if not path:
                return self.reply(404, {"error": {"code": "not_found"}})
            with open(path, "rb") as image:
                data = image.read()
            self.send_response(200)
            self.send_header("Content-Type", "image/jpeg")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            return None
        if self.path == "/api/v1/playback/status":
            st, _ = engine("GET", "/v1/health")
            return self.reply(200, {"available": st == 200, "media": sorted(MEDIA), "network": args.network})
        if self.path == "/api/v1/me":
            return self.reply(200, {"name": "Antoine Langlois", "email": "antoine@example.net", "role": "Admin", "library_language": "English"})
        if self.path == "/api/v1/devices":
            now = time.time()
            ago = lambda s: time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(now - s))
            me = KEYS.get(self.headers.get("X-API-Key"), {"name": "Living room", "platform": "android-tv"})
            return self.reply(200, {"items": [
                {"id": "dev-self", "name": me["name"], "platform": me["platform"], "last_seen": ago(0), "streaming": False, "current": True},
                {"id": "dev-phone", "name": "Pixel 9", "platform": "Android", "last_seen": ago(120), "streaming": True, "current": False},
                {"id": "dev-laptop", "name": "framework-13", "platform": "Linux", "last_seen": ago(3 * 86400), "streaming": False, "current": False},
                {"id": "dev-parents", "name": "Parents' TV", "platform": "Android TV", "last_seen": ago(9 * 86400), "streaming": False, "current": False},
            ]})
        m = re.fullmatch(r"/api/v1/playback/media/([^/]+)/tracks", self.path)
        if m and m[1] in MEDIA:
            return self.reply(200, tracks(m[1], MEDIA[m[1]], f"http://{self.headers['Host']}"))
        m = re.fullmatch(r"/api/v1/playback/media/([^/]+)/subtitles/(.+)", self.path)
        if m and MEDIA.get(m[1]) in SUBTITLES:
            job = SUBTITLES[MEDIA[m[1]]][0]
            req = urllib.request.Request(
                f"{args.engine}/v1/jobs/{job}/files/{m[2]}",
                headers={"Authorization": f"Bearer {args.secret}"},
            )
            with urllib.request.urlopen(req, timeout=120) as r:
                data = r.read()
            self.send_response(200)
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            return None
        m = re.fullmatch(r"/api/v1/playback/sessions/([0-9a-f]{32})", self.path)
        if m:
            _, resp = engine("GET", "/v1/sessions")
            found = next((i for i in resp["sessions"] if i["session_id"] == m[1]), None)
            return self.reply(200, found) if found else self.reply(404, {"error": {"code": "not_found"}})
        self.reply(404, {"error": {"code": "not_found"}})

    def do_POST(self):
        if self.refused():
            return
        # Dev only: what an admin does from Streamline's web app. Both answer the next heartbeat.
        m = re.fullmatch(r"/dev/stop(?:/([0-9a-f]{32}))?", self.path)
        if m:
            message = self.body().get("message", "")
            for sid in [m[1]] if m[1] else list(LAST_BEAT):
                STOPPED[sid] = message
                LAST_BEAT.pop(sid, None)
                engine("DELETE", f"/v1/sessions/{sid}")
            return self.reply(204)
        m = re.fullmatch(r"/dev/(approve|deny)(?:/([A-Z0-9-]+))?", self.path)
        if m:
            found = self.pairing(m[2])
            if not found:
                return self.reply(404, {"error": {"code": "no_pending_pairing"}})
            found["status"] = "approved" if m[1] == "approve" else "denied"
            return self.reply(200, {"user_code": found["user_code"], "name": found["name"]})
        if self.path == "/dev/revoke":
            KEYS.clear()
            return self.reply(204)
        if self.path == "/api/v1/auth/device/code":
            b = self.body()
            code = "".join(secrets.choice(CODE_ALPHABET) for _ in range(6))
            code = f"{code[:3]}-{code[3:]}"
            device_code = secrets.token_urlsafe(32)
            PAIRINGS[device_code] = {"user_code": code, "expires": time.monotonic() + PAIR_TTL, "status": "pending",
                                     "client_id": b.get("client_id"), "name": b.get("name", "")[:64], "platform": b.get("platform", "")}
            base = f"http://{self.headers['Host']}"
            return self.reply(200, {"device_code": device_code, "user_code": code, "verification_uri": f"{base}/pair",
                                    "verification_uri_complete": f"{base}/pair?code={code}", "expires_in": PAIR_TTL, "interval": PAIR_INTERVAL})
        if self.path == "/api/v1/auth/device/token":
            found = PAIRINGS.get(self.body().get("device_code", ""))
            if not found or time.monotonic() > found["expires"]:
                return self.reply(400, {"error": {"code": "expired_token"}})
            if found["status"] == "pending":
                return self.reply(400, {"error": {"code": "authorization_pending"}})
            if found["status"] == "denied":
                return self.reply(400, {"error": {"code": "access_denied"}})
            for k, v in list(KEYS.items()):
                if v["client_id"] == found["client_id"]:
                    del KEYS[k]
            key = secrets.token_hex(24)
            KEYS[key] = found
            PAIRINGS.pop(next(d for d, p in PAIRINGS.items() if p is found))
            return self.reply(200, {"api_key": key, "key_id": len(KEYS)})
        if self.path == "/dev/restart":
            b = self.body()
            RESTART.update(until=time.monotonic() + b.get("seconds", 10), message=b.get("message", ""))
            return self.reply(204)
        if time.monotonic() < RESTART["until"] and self.path.startswith("/api/v1/playback/sessions"):
            left = max(1, round(RESTART["until"] - time.monotonic()))
            return self.reply(503, {"error": {"code": "server_restarting", "message": RESTART["message"], "by": ADMIN, "retry_after": left}})
        if self.path == "/api/v1/playback/decisions":
            b = self.body()
            path = MEDIA.get(b.pop("media_id", None))
            if path is None:
                return self.reply(404, {"code": "media_not_found", "message": "no such media"})
            if path in ANALYSIS:
                b["analysis"] = ANALYSIS[path]
            st, resp = engine("POST", "/v1/decisions", {"path": path, **b})
            return self.reply(st, resp)
        if self.path == "/api/v1/playback/sessions":
            b = self.body()
            media_id = b.pop("media_id", None)
            path = MEDIA.get(media_id)
            if path is None:
                return self.reply(404, {"error": {"code": "media_not_found"}})
            sid = secrets.token_hex(16)
            if path in ANALYSIS:
                b["analysis"] = ANALYSIS[path]
            SESSIONS[sid] = media_id
            st, resp = engine("POST", "/v1/sessions", {"session_id": sid, "path": path, **b})
            if st < 300:
                LAST_BEAT[sid] = time.monotonic()
            return self.reply(st, with_url(sid, resp) if st < 300 else resp)
        m = re.fullmatch(r"/api/v1/playback/sessions/([0-9a-f]{32})/progress", self.path)
        if m:
            if m[1] in STOPPED:
                return self.reply(410, {"error": {"code": "session_stopped", "message": STOPPED.pop(m[1]), "by": ADMIN}})
            if m[1] in LAST_BEAT:
                LAST_BEAT[m[1]] = time.monotonic()
            note_progress(m[1], self.body().get("position", 0))
            return self.reply(204)
        self.reply(404, {"error": {"code": "not_found"}})

    def do_PUT(self):
        if self.refused():
            return
        m = re.fullmatch(r"/api/v1/watchstate/(.+)", self.path)
        if not m or m[1] not in MEDIA:
            return self.reply(404, {"error": {"code": "not_found"}})
        WATCH[m[1]] = {
            **REFS.get(m[1], {}), "media_id": m[1], "position_seconds": 0, "duration_seconds": 0,
            "watched": self.body().get("watched", False), "updated_at": datetime.datetime.now(datetime.UTC).isoformat(),
        }
        self.reply(200, WATCH[m[1]])

    def do_PATCH(self):
        if self.refused():
            return
        m = re.fullmatch(r"/api/v1/playback/sessions/([0-9a-f]{32})", self.path)
        if not m:
            return self.reply(404, {"error": {"code": "not_found"}})
        st, resp = engine("PATCH", f"/v1/sessions/{m[1]}", self.body())
        self.reply(st, with_url(m[1], resp) if st < 300 else resp)

    def do_DELETE(self):
        if self.refused():
            return
        if self.path == "/api/v1/devices/current":
            KEYS.pop(self.headers.get("X-API-Key"), None)
            return self.reply(204)
        m = re.fullmatch(r"/api/v1/playback/sessions/([0-9a-f]{32})", self.path)
        if not m:
            return self.reply(404, {"error": {"code": "not_found"}})
        LAST_BEAT.pop(m[1], None)
        st, resp = engine("DELETE", f"/v1/sessions/{m[1]}")
        self.reply(st, resp)


def reap_silent():
    """Streamline stops a session whose client stopped heartbeating (a crash, a killed app)."""
    while True:
        time.sleep(5)
        cutoff = time.monotonic() - args.heartbeat_timeout
        for sid, beat in list(LAST_BEAT.items()):
            if beat < cutoff and LAST_BEAT.pop(sid, None) is not None:
                st, _ = engine("DELETE", f"/v1/sessions/{sid}")
                print(f"stopped silent session {sid}: engine {st}", flush=True)


threading.Thread(target=reap_silent, daemon=True).start()

for media_path in set(MEDIA.values()):
    threading.Thread(target=prepare, args=(media_path,), daemon=True).start()

for show in SERIES:
    for season in show["seasons"]:
        season_paths = sorted({MEDIA[e["media_file_id"]] for e in season["episodes"]})
        threading.Thread(target=prepare_season, args=(season_paths,), daemon=True).start()
        break
    break

host, port = args.listen.rsplit(":", 1)
print(f"mock streamline on {args.listen}, engine {args.engine}, media {sorted(MEDIA)}", flush=True)
ThreadingHTTPServer((host, int(port)), Handler).serve_forever()
