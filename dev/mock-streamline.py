"""Stands in for Streamline's playback API until the server side lands.

Proxies /api/v1/playback/* to the engine's control plane and mints the HLS
token like Streamline will (HMAC-SHA256 over "hls|<id>|<exp>"). No auth; watch
state lives in memory. Files are analyzed and their subtitles extracted at start,
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
args = p.parse_args()
FILES = dict(m.split("=", 1) for m in args.media)
MEDIA = dict(FILES)
ANALYSIS = {}
SUBTITLES = {}
MARKERS = {}
SESSIONS = {}
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

    def do_GET(self):
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
            return self.reply(200, {"available": st == 200, "media": sorted(MEDIA)})
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
            return self.reply(st, with_url(sid, resp) if st < 300 else resp)
        m = re.fullmatch(r"/api/v1/playback/sessions/([0-9a-f]{32})/progress", self.path)
        if m:
            note_progress(m[1], self.body().get("position", 0))
            return self.reply(204)
        self.reply(404, {"error": {"code": "not_found"}})

    def do_PUT(self):
        m = re.fullmatch(r"/api/v1/watchstate/(.+)", self.path)
        if not m or m[1] not in MEDIA:
            return self.reply(404, {"error": {"code": "not_found"}})
        WATCH[m[1]] = {
            **REFS.get(m[1], {}), "media_id": m[1], "position_seconds": 0, "duration_seconds": 0,
            "watched": self.body().get("watched", False), "updated_at": datetime.datetime.now(datetime.UTC).isoformat(),
        }
        self.reply(200, WATCH[m[1]])

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
