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
own file. --art points at p/<slug>.jpg and b/<slug>.jpg artwork, and c/<person>.jpg headshots.

    python3 dev/mock-streamline.py --secret <engine secret> --art /path/to/img \
        --clip /path/to/clip.mkv [--clip ...] [--media film=/path/to/film.mkv ...]

The public URL defaults to 10.0.2.2, the Android emulator's alias for the host.
"""

import argparse
import base64
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
import unicodedata
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
p.add_argument("--now", type=float, help="epoch seconds shown timestamps count from (token expiry stays real), to pin dates for screenshot runs")
p.add_argument("--cors-origin", action="append", help="origin a browser may call from, repeatable")
p.add_argument("--access-ttl", type=int, default=900, help="seconds a browser's access token lasts")
p.add_argument("--refresh-ttl", type=int, default=2592000, help="seconds a browser's refresh token lasts")
p.add_argument("--password", default="streamline", help="password of the email sign-in")
p.add_argument("--empty", action="store_true", help="serve a library with no titles in it")
p.add_argument("--demo-progress", action="store_true", help="start with Dune: Part Two and Severance S2 E4 partly watched")
p.add_argument("--sso-deny", action="store_true", help="have the SSO provider refuse every sign-in")
p.add_argument("--heartbeat-timeout", type=float, default=90, help="seconds without a heartbeat before a session is stopped")
args = p.parse_args()
CORS_ORIGINS = args.cors_origin or ["http://localhost:1430", "http://127.0.0.1:1430"]
USER_EMAIL = "antoine@example.net"
TAKEN_EMAIL = "taken@example.net"
PICTURE_MAX = 2 * 1024 * 1024
PICTURE_TYPES = ("image/jpeg", "image/png", "image/webp")
ME = {"name": "Antoine Langlois", "email": USER_EMAIL, "role": "Admin", "library_language": "English"}
PICTURE = {"data": None, "type": ""}
ART_ON = {"on": True}
ARRIVAL_DAYS = 7
BITRATE = {"2160p": 40_000_000, "1080p": 12_000_000}


def shown_now():
    return time.time() if args.now is None else args.now


def stamp():
    return datetime.datetime.fromtimestamp(shown_now(), datetime.UTC).isoformat()

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
# Browser sign-in: one-time codes, access tokens, and a device per browser holding its refresh token chain.
CODES = {}
ACCESS = {}
BROWSERS = {}
REFRESH = {}
SSO = [{"id": "authentik", "label": "Authentik"}]
WATCH = {}
WATCHED_AT = 0.9

with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "mock-library.json")) as f:
    LIBRARY = json.load(f)
MOVIES = [] if args.empty else LIBRARY["movies"]
SERIES = [] if args.empty else LIBRARY["series"]
PEOPLE = LIBRARY.get("people", {})
REFS = {}
clips = itertools.cycle(args.clip or list(FILES.values()))
for number, media_id in enumerate(FILES):
    MOVIES.insert(number, {
        "id": 1000 + number, "slug": media_id, "title": media_id, "original_title": media_id, "year": 2026,
        "overview": "", "runtime": 0, "status": "available", "monitored": True, "tmdb_id": 0, "genres": ["Test"],
        "rating": 0, "added_at": stamp(), "file_summary": {"file_count": 1},
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


if args.demo_progress and not args.empty:
    dune = next(m for m in MOVIES if m["slug"] == "dune2")
    severance = next(s for s in SERIES if s["slug"] == "severance")
    episode = severance["seasons"][1]["episodes"][3]
    for offset, (media_id, runtime, fraction) in enumerate([
        (episode["media_file_id"], episode["runtime"], 0.3), (dune["media_files"][0]["id"], dune["runtime"], 0.4),
    ]):
        duration = runtime * 60
        WATCH[media_id] = {
            **REFS[media_id], "media_id": media_id, "position_seconds": int(duration * fraction), "duration_seconds": duration,
            "watched": False, "updated_at": datetime.datetime.fromtimestamp(shown_now() - offset * 3600, datetime.UTC).isoformat(),
        }


def file_size(title, runtime):
    return runtime * 60 * BITRATE.get(title.get("file_summary", {}).get("resolution"), 8_000_000) // 8


for movie in MOVIES:
    if "resolution" in movie.get("file_summary", {}):
        movie["file_summary"]["size"] = file_size(movie, movie["runtime"])
for show in SERIES:
    show["file_summary"]["size"] = file_size(show, sum(e["runtime"] for s in show["seasons"] for e in s["episodes"] if e["has_file"]))


def initials(name):
    words = name.split()
    return "".join(w[0] for w in (words[:1] + words[-1:] if len(words) > 1 else words)).upper()


def profile():
    return {**ME, "picture": PICTURE["data"] is not None, "initials": initials(ME["name"])}


def home():
    """Featured titles and the last week's arrivals, from the library and the watch state."""
    window = (datetime.datetime.fromtimestamp(shown_now(), datetime.UTC) - datetime.timedelta(days=ARRIVAL_DAYS)).date()
    by_key = {("movies", m["id"]): m for m in MOVIES} | {("series", s["id"]): s for s in SERIES}
    featured, arrivals = [], []
    for w in sorted(WATCH.values(), key=lambda w: w["updated_at"], reverse=True):
        key = ("movies", w["movie_id"]) if "movie_id" in w else ("series", w.get("series_id"))
        if key in by_key and not w["watched"] and w["position_seconds"] > 0 and all(f["kind"] != key[0] or f["id"] != key[1] for f in featured):
            featured.append({"kind": key[0], "id": key[1], "reason": "continue", **({"season": w["season"]} if "season" in w else {})})
    for show in sorted(SERIES, key=lambda s: s["added_at"], reverse=True):
        added = datetime.datetime.fromisoformat(show["added_at"].replace("Z", "+00:00"))
        if added.date() < window:
            continue
        latest = show["seasons"][-1]
        if len(show["seasons"]) > 1:
            numbers = [e["number"] for e in latest["episodes"][-2:]]
            arrivals.append((added, {"kind": "series", "id": show["id"], "at": added.isoformat(), "season": latest["number"], "episodes": numbers}))
            if all(f["kind"] != "series" or f["id"] != show["id"] for f in featured):
                featured.append({"kind": "series", "id": show["id"], "reason": "new_episodes", "season": latest["number"], "episodes": numbers})
        else:
            arrivals.append((added, {"kind": "series", "id": show["id"], "at": added.isoformat()}))
    for movie in sorted(MOVIES, key=lambda m: m["added_at"], reverse=True):
        added = datetime.datetime.fromisoformat(movie["added_at"].replace("Z", "+00:00"))
        if added.date() >= window:
            arrivals.append((added, {"kind": "movies", "id": movie["id"], "at": added.isoformat()}))
    taken = {(f["kind"], f["id"]) for f in featured}
    for movie in sorted(MOVIES, key=lambda m: m["added_at"], reverse=True):
        if ("movies", movie["id"]) not in taken:
            featured.append({"kind": "movies", "id": movie["id"], "reason": "added"})
    days = {}
    for added, item in sorted(arrivals, key=lambda a: a[0], reverse=True):
        days.setdefault(added.date().isoformat(), []).append(item)
    return {"featured": featured[:3], "arrivals": [{"date": d, "items": items} for d, items in days.items()]}


def summary(item):
    return {k: v for k, v in item.items() if k not in ("media_files", "seasons", "cast")}


def norm_title(s):
    return re.sub(r"[^a-z0-9]+", "", unicodedata.normalize("NFD", s).encode("ascii", "ignore").decode().lower())


def person(pid):
    """A person page: their credits on this server, and what they are known for (TMDB-style)."""
    if pid not in PEOPLE:
        return None
    credits, here = [], {}
    for kind, items in (("movies", MOVIES), ("series", SERIES)):
        for t in items:
            here[norm_title(t["title"])] = (kind, t["id"])
            for c in t.get("cast", []):
                if c["id"] == pid:
                    credit = {"kind": kind, "id": t["id"], "title": t["title"], "year": t["year"], "character": c["character"]}
                    if kind == "series":
                        credit.update(seasons=c.get("seasons", []), total_seasons=t.get("total_seasons"))
                    credits.append(credit)
    known = []
    for k in PEOPLE[pid]["known_for"]:
        found = here.get(norm_title(k["title"]))
        known.append({**k, **({"kind": found[0], "id": found[1]} if found else {})})
    return {"id": pid, "name": PEOPLE[pid]["name"], "department": "Acting", "credits": credits, "known_for": known}


def art(kind, item_id, name):
    items = MOVIES if kind == "movies" else SERIES
    slug = next((i["slug"] for i in items if str(i["id"]) == item_id), None)
    folder = {"poster": "p", "backdrop": "b"}[name]
    path = os.path.join(args.art, folder, f"{slug}.jpg")
    if not ART_ON["on"]:
        return None
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
        "updated_at": stamp(),
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


def issue_tokens(device_id):
    """A new access token and the next refresh token of a browser device."""
    device = BROWSERS[device_id]
    access = secrets.token_urlsafe(32)
    ACCESS[access] = {"device": device_id, "exp": time.monotonic() + args.access_ttl}
    device["used"].add(device["refresh"])
    device["refresh"] = secrets.token_urlsafe(32)
    device["expires"] = time.time() + args.refresh_ttl
    REFRESH[device["refresh"]] = device_id
    return {"access_token": access, "token_type": "Bearer", "expires_in": args.access_ttl,
            "refresh_token": device["refresh"], "refresh_expires_in": args.refresh_ttl, "device_id": device_id}


def new_browser(name):
    device_id = "web-" + secrets.token_hex(8)
    BROWSERS[device_id] = {"name": name[:64] or "Web browser", "refresh": None, "used": set(), "expires": 0, "seen": time.time()}
    BROWSERS[device_id]["refresh"] = secrets.token_urlsafe(32)
    return issue_tokens(device_id)


def revoke_browser(device_id):
    if BROWSERS.pop(device_id, None) is None:
        return False
    for token, owner in list(ACCESS.items()):
        if owner["device"] == device_id:
            del ACCESS[token]
    return True


def bearer_device(headers):
    """The browser device a Bearer header names, or None when absent, expired or revoked."""
    value = headers.get("Authorization", "")
    if not value.startswith("Bearer "):
        return None
    found = ACCESS.get(value[7:])
    if found is None or time.monotonic() > found["exp"] or found["device"] not in BROWSERS:
        return None
    return found["device"]


def pkce_matches(verifier, challenge):
    digest = hashlib.sha256(verifier.encode()).digest()
    return base64.urlsafe_b64encode(digest).rstrip(b"=").decode() == challenge


class Handler(BaseHTTPRequestHandler):
    def end_headers(self):
        origin = self.headers.get("Origin")
        if origin in CORS_ORIGINS and self.path.startswith("/api/v1"):
            self.send_header("Access-Control-Allow-Origin", origin)
            self.send_header("Vary", "Origin")
            if self.command == "OPTIONS":
                self.send_header("Access-Control-Allow-Headers", "Authorization, Content-Type")
                self.send_header("Access-Control-Allow-Methods", "GET, POST, PUT, PATCH, DELETE")
        super().end_headers()

    def do_OPTIONS(self):
        self.reply(204)

    def reply(self, status, body=None):
        data = b"" if body is None else json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def image(self, data, content_type):
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def body(self):
        n = int(self.headers.get("Content-Length") or 0)
        return json.loads(self.rfile.read(n) or b"{}")

    def refused(self):
        if self.path.startswith("/api/v1/auth/"):
            return False
        if self.headers.get("Authorization") is not None:
            if bearer_device(self.headers) is not None:
                return False
            self.reply(401, {"error": {"code": "unauthorized"}})
            return True
        key = self.headers.get("X-API-Key")
        if key is None or key in KEYS:
            return False
        self.reply(401, {"error": {"code": "unauthorized"}})
        return True

    def authorize(self, q):
        """The SSO provider's side: send the browser back to its redirect_uri with a one-time code, or the denial."""
        one = lambda name: q.get(name, [""])[0]
        target = urllib.parse.urlsplit(one("redirect_uri"))
        origin = f"{target.scheme}://{target.netloc}"
        if one("provider") not in [s["id"] for s in SSO] or origin not in CORS_ORIGINS or one("code_challenge_method") != "S256" or not one("code_challenge"):
            return self.reply(400, {"error": {"code": "invalid_request"}})
        if args.sso_deny:
            params = {"error": "access_denied", "state": one("state")}
        else:
            code = secrets.token_urlsafe(32)
            CODES[code] = {"challenge": one("code_challenge"), "redirect_uri": one("redirect_uri")}
            params = {"code": code, "state": one("state")}
        self.send_response(302)
        self.send_header("Location", f"{one('redirect_uri')}?{urllib.parse.urlencode(params)}")
        self.send_header("Content-Length", "0")
        self.end_headers()
        return None

    def token(self):
        b = self.body()
        invalid = lambda: self.reply(400, {"error": {"code": "invalid_grant"}})
        grant = b.get("grant_type")
        if grant == "authorization_code":
            found = CODES.pop(b.get("code", ""), None)
            if not found or found["redirect_uri"] != b.get("redirect_uri") or not pkce_matches(b.get("code_verifier", ""), found["challenge"]):
                return invalid()
            return self.reply(200, new_browser(b.get("device_name", "")))
        if grant == "password":
            if b.get("email") != USER_EMAIL or b.get("password") != args.password:
                return self.reply(401, {"error": {"code": "invalid_credentials"}})
            return self.reply(200, new_browser(b.get("device_name", "")))
        if grant == "refresh_token":
            token = b.get("refresh_token", "")
            device_id = REFRESH.get(token)
            device = BROWSERS.get(device_id)
            if device is None:
                return invalid()
            if device["refresh"] != token:
                revoke_browser(device_id)
                return invalid()
            if time.time() > device["expires"]:
                return invalid()
            return self.reply(200, issue_tokens(device_id))
        return self.reply(400, {"error": {"code": "unsupported_grant_type"}})

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
        m = re.fullmatch(r"/api/v1/people/([a-z0-9-]+)", self.path)
        if m:
            found = person(m[1])
            return self.reply(200, found) if found else self.reply(404, {"error": {"code": "not_found"}})
        if self.path == "/api/v1/home":
            return self.reply(200, home())
        url = urllib.parse.urlsplit(self.path)
        if url.path == "/api/v1/posters/users/me/photo.jpg":
            if PICTURE["data"] is None:
                return self.reply(404, {"error": {"code": "not_found"}})
            return self.image(PICTURE["data"], PICTURE["type"])
        m = re.fullmatch(r"/api/v1/posters/(movies|series|people)/([\w-]+)/(poster|backdrop|photo)\.jpg", self.path)
        if m:
            if not ART_ON["on"] or (m[1] == "people") != (m[3] == "photo"):
                path = None
            elif m[1] == "people":
                path = os.path.join(args.art, "c", f"{m[2]}.jpg")
                path = path if os.path.exists(path) else None
            else:
                path = art(m[1], m[2], m[3])
            if not path:
                return self.reply(404, {"error": {"code": "not_found"}})
            with open(path, "rb") as image:
                return self.image(image.read(), "image/jpeg")
        if self.path == "/api/v1/playback/status":
            st, _ = engine("GET", "/v1/health")
            return self.reply(200, {"available": st == 200, "media": sorted(MEDIA), "network": args.network})
        if url.path == "/api/v1/auth/methods":
            return self.reply(200, {"sso": SSO, "email": True, "register_url": None})
        if url.path == "/api/v1/auth/authorize":
            return self.authorize(urllib.parse.parse_qs(url.query))
        if self.path == "/api/v1/me":
            return self.reply(200, profile())
        if self.path == "/api/v1/devices":
            now = shown_now()
            ago = lambda s: time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(now - s))
            me = KEYS.get(self.headers.get("X-API-Key"), {"name": "Living room", "platform": "android-tv"})
            caller = bearer_device(self.headers)
            browsers = [
                {"id": i, "kind": "browser", "name": b["name"], "platform": "Web browser", "last_seen": ago(0), "streaming": False,
                 "current": i == caller, "expires_at": datetime.datetime.fromtimestamp(b["expires"], datetime.UTC).isoformat()}
                for i, b in BROWSERS.items()
            ]
            return self.reply(200, {"items": [
                {"id": "dev-self", "kind": "app", "name": me["name"], "platform": me["platform"], "last_seen": ago(0), "streaming": False, "current": caller is None},
                {"id": "dev-phone", "kind": "app", "name": "Pixel 9", "platform": "Android", "last_seen": ago(120), "streaming": True, "current": False},
                {"id": "dev-laptop", "kind": "app", "name": "framework-13", "platform": "Linux", "last_seen": ago(3 * 86400), "streaming": False, "current": False},
                {"id": "dev-parents", "kind": "app", "name": "Parents' TV", "platform": "Android TV", "last_seen": ago(9 * 86400), "streaming": False, "current": False},
                *browsers,
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
        m = re.fullmatch(r"/dev/art/(on|off)", self.path)
        if m:
            ART_ON["on"] = m[1] == "on"
            return self.reply(204)
        if self.path == "/dev/revoke":
            KEYS.clear()
            return self.reply(204)
        if self.path == "/api/v1/auth/token":
            return self.token()
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
                return self.reply(404, {"error": {"code": "media_not_found", "message": "no such media"}})
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
        if self.path == "/api/v1/me/picture":
            data = self.rfile.read(int(self.headers.get("Content-Length") or 0))
            kind = self.headers.get("Content-Type", "").split(";")[0].strip()
            if kind not in PICTURE_TYPES:
                return self.reply(415, {"error": {"code": "unsupported_type"}})
            if len(data) > PICTURE_MAX:
                return self.reply(413, {"error": {"code": "too_large", "message": "The picture is over 2 MB."}})
            PICTURE.update(data=data, type=kind)
            return self.reply(204)
        m = re.fullmatch(r"/api/v1/watchstate/(.+)", self.path)
        if not m or m[1] not in MEDIA:
            return self.reply(404, {"error": {"code": "not_found"}})
        WATCH[m[1]] = {
            **REFS.get(m[1], {}), "media_id": m[1], "position_seconds": 0, "duration_seconds": 0,
            "watched": self.body().get("watched", False), "updated_at": stamp(),
        }
        self.reply(200, WATCH[m[1]])

    def do_PATCH(self):
        if self.refused():
            return
        if self.path == "/api/v1/me":
            b = self.body()
            email = b.get("device_email")
            if email is not None and email != "":
                if not re.fullmatch(r"[^@\s]+@[^@\s]+\.[^@\s]+", email):
                    return self.reply(422, {"error": {"code": "invalid_email", "message": "That is not an email address."}})
                if email == TAKEN_EMAIL:
                    return self.reply(409, {"error": {"code": "email_taken", "message": "That email is already in use."}})
            if b.get("display_name") is not None:
                ME["name"] = b["display_name"]
            if email is not None:
                ME["email"] = email
            return self.reply(200, profile())
        m = re.fullmatch(r"/api/v1/playback/sessions/([0-9a-f]{32})", self.path)
        if not m:
            return self.reply(404, {"error": {"code": "not_found"}})
        st, resp = engine("PATCH", f"/v1/sessions/{m[1]}", self.body())
        self.reply(st, with_url(m[1], resp) if st < 300 else resp)

    def do_DELETE(self):
        if self.refused():
            return
        if self.path == "/api/v1/me/picture":
            PICTURE.update(data=None, type="")
            return self.reply(204)
        if self.path == "/api/v1/devices/current":
            caller = bearer_device(self.headers)
            if caller:
                revoke_browser(caller)
            else:
                KEYS.pop(self.headers.get("X-API-Key"), None)
            return self.reply(204)
        m = re.fullmatch(r"/api/v1/devices/([\w-]+)", self.path)
        if m:
            if revoke_browser(m[1]) or m[1] in ("dev-phone", "dev-laptop", "dev-parents"):
                return self.reply(204)
            return self.reply(404, {"error": {"code": "not_found"}})
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


host, port = args.listen.rsplit(":", 1)
server = ThreadingHTTPServer((host, int(port)), Handler)
print(f"listening on {host}:{server.server_address[1]}", flush=True)

threading.Thread(target=reap_silent, daemon=True).start()

for media_path in set(MEDIA.values()):
    threading.Thread(target=prepare, args=(media_path,), daemon=True).start()

for show in SERIES:
    for season in show["seasons"]:
        season_paths = sorted({MEDIA[e["media_file_id"]] for e in season["episodes"]})
        threading.Thread(target=prepare_season, args=(season_paths,), daemon=True).start()
        break
    break

print(f"mock streamline on {args.listen}, engine {args.engine}, media {sorted(MEDIA)}", flush=True)
server.serve_forever()
