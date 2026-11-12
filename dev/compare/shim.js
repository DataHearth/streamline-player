// A plain-browser stand-in for Tauri: the Rust commands go to the mock through bridge.py, and the
// player plugin is a fake that reports playback without decoding anything.
(() => {
  const B = "http://127.0.0.1:8099";
  const cbs = new Map();
  let next = 1;
  const listeners = {};
  const send = (event, payload) => (listeners[event] || []).forEach((l) => cbs.get(l.id)?.({ message: payload, index: l.i++ }));
  const call = async (method, path, body) => {
    const r = await fetch(B + "/api/v1" + path, { method, headers: body ? { "Content-Type": "application/json" } : {}, body: body ? JSON.stringify(body) : undefined });
    const text = await r.text();
    const v = text ? JSON.parse(text) : null;
    if (!r.ok) throw JSON.stringify(v);
    return v;
  };
  const caps = { video: ["h264", "hevc", "av1"].map((codec) => ({ codec, profiles: codec === "h264" ? ["baseline", "constrained baseline", "main", "high", "high 10"] : codec === "hevc" ? ["main", "main 10"] : ["main"], max_level: null, max_width: 3840, max_height: 2160, max_fps: 60 })), hdr: { formats: ["hdr10", "hdr10plus", "hlg"], dv_profiles: [] }, audio: { codecs: ["aac", "ac3", "dts", "eac3", "flac", "mp3", "opus", "truehd", "vorbis"], max_channels: 8, passthrough: [] }, containers: ["mp4", "mkv"], subtitles: ["webvtt", "srt", "ass", "pgs"], hls_keyframe_exact: false };
  const p = { pos: 0, dur: 0, playing: false, timer: 0, speed: 1 };
  const tickOn = () => {
    clearInterval(p.timer);
    p.timer = setInterval(() => {
      if (!p.playing || !window.__fakeTick) return;
      p.pos = Math.min(p.dur, p.pos + 0.5 * p.speed);
      send("position", { position: p.pos, buffered: Math.min(p.dur, p.pos + 30), duration: p.dur });
      if (p.pos >= p.dur) { p.playing = false; send("state", { state: "ended" }); }
    }, 500);
  };
  window.__fake = { p, send, tickOn };
  const plugin = {
    capabilities: () => caps,
    register_listener: ({ event, handler }) => { (listeners[event] ||= []).push({ id: typeof handler === "object" ? handler.id : Number(String(handler).split(":")[1]), i: 0 }); },
    remove_listener: ({ event, channelId }) => { listeners[event] = (listeners[event] || []).filter((l) => l.id !== channelId); },
    load: async ({ url }) => { p.pos = 0; p.dur = window.__fakeDuration || 5400; send("state", { state: "buffering" }); await new Promise((r) => setTimeout(r, 300)); send("state", { state: "paused" }); send("position", { position: 0, buffered: 30, duration: p.dur }); },
    play: () => { p.playing = true; send("state", { state: "playing" }); tickOn(); },
    pause: () => { p.playing = false; send("state", { state: "paused" }); },
    seek: ({ seconds }) => { p.pos = seconds; send("position", { position: p.pos, buffered: p.pos + 30, duration: p.dur }); },
    speed: ({ speed }) => { p.speed = speed; },
    stop: () => { p.playing = false; clearInterval(p.timer); send("state", { state: "idle" }); },
    stats: () => ({ player: "fake · browser", version: "fake", buffer: 30, videoBitrate: 8e6, audioBitrate: 6.4e5, rendered: Math.round(p.pos * 24), dropped: 0, displayHz: 60, width: 3840, height: 2160 }),
    subtitle: () => null,
    timing: () => null,
    voice: () => ({ text: "" }),
    frameRate: () => null,
    restoreDisplay: () => null,
    deviceInfo: () => ({ name: "Living room", model: "Ugoos AM9 Pro", platform: "Android TV", player: "ExoPlayer 1.8.0", display: { name: "HDMI", width: 3840, height: 2160, refresh_hz: 60, hdr: ["HDR10", "HLG", "Dolby Vision"] }, ...(window.__deviceInfo || {}) }),
  };
  const kp = (k) => (k === "movies" ? "/movies" : "/series");
  // __holdStart parks start_session until __releaseStart(), to press Back while it is in flight.
  const startSession = ({ mediaId, capabilities, position, tracks }) => window.__failStart ? Promise.reject(window.__failStart) : call("POST", "/playback/sessions", { media_id: mediaId, capabilities, position, ...(tracks || {}) });
  const fakePairStart = () => {
    const code = "K7F-Q2M", uri = "https://streamline.home.arpa/pair";
    const q = window.qrcode(0, "M"); q.addData(`${uri}?code=${code}`); q.make();
    return { code, name: "Living room · SHIELD", uri, uri_complete: `${uri}?code=${code}`, expires_in: 600, interval: 1, qr: q.createSvgTag({ cellSize: 8, margin: 0 }) };
  };
  // window.__realPair: pairing goes to the mock as the Rust side does it; __pairDown makes it fail as a dead network does.
  let deviceCode = null;
  const realPairStart = async () => {
    if (window.__pairDown) throw "error sending request for url (http://127.0.0.1:8081/api/v1/auth/device/code)";
    const name = "framework-13";
    const r = await call("POST", "/auth/device/code", { client_id: "shim-desktop", name, platform: "Linux" });
    deviceCode = r.device_code;
    const q = window.qrcode(0, "M"); q.addData(r.verification_uri_complete); q.make();
    return { code: r.user_code, name, uri: r.verification_uri, uri_complete: r.verification_uri_complete, expires_in: r.expires_in, interval: r.interval, qr: q.createSvgTag({ cellSize: 8, margin: 0 }) };
  };
  const realPairPoll = async () => {
    if (!deviceCode) return "expired";
    try {
      await call("POST", "/auth/device/token", { device_code: deviceCode });
    } catch (e) {
      const code = JSON.parse(e)?.error?.code;
      const answer = { authorization_pending: "pending", slow_down: "slow_down", access_denied: "denied", expired_token: "expired" }[code];
      if (answer) return answer;
      throw e;
    }
    deviceCode = null;
    window.__auth = { ...(window.__auth || {}), paired: true };
    return "paired";
  };
  const cmds = {
    server_host: () => "127.0.0.1:8081",
    auth_state: () => ({ server: null, paired: true, keystore: "Android Keystore", ...(window.__auth || {}) }),
    connect: ({ url }) => (/bad/.test(url) ? Promise.reject(`Can’t reach ${url}`) : new Promise((r) => setTimeout(() => r(38), 600))),
    pair_start: () => window.__realPair ? realPairStart() : fakePairStart(),
    open_external: ({ url }) => { (window.__opened ||= []).push(url); return null; },
    pair_poll: () => window.__realPair ? realPairPoll() : (window.__pairAnswer || "pending"),
    sign_out: () => { if (window.__realPair) window.__auth = { ...(window.__auth || {}), paired: false }; return null; },
    "plugin:event|listen": () => 1,
    "plugin:event|unlisten": () => null,
    library: ({ kind }) => call("GET", kp(kind)),
    title: ({ kind, id }) => call("GET", `${kp(kind)}/${id}`),
    person: ({ id }) => call("GET", `/people/${id}`),
    watch_state: () => call("GET", "/watchstate"),
    set_watched: ({ mediaId, watched }) => call("PUT", `/watchstate/${mediaId}`, { watched }),
    me: () => call("GET", "/me"),
    home: () => call("GET", "/home"),
    update_me: ({ name, email }) => call("PATCH", "/me", { ...(name === undefined ? {} : { display_name: name }), ...(email === undefined ? {} : { device_email: email }) }),
    set_picture: (bytes, options) => fetch(`${B}/api/v1/me/picture`, { method: "PUT", headers: { "Content-Type": options.headers["x-picture-type"] }, body: bytes }).then(async (r) => { if (!r.ok) throw await r.text(); return null; }),
    remove_picture: () => call("DELETE", "/me/picture"),
    revoke_device: ({ id }) => call("DELETE", `/devices/${encodeURIComponent(id)}`),
    devices: () => call("GET", "/devices"),
    playback_status: () => call("GET", "/playback/status"),
    start_session: (a) => window.__holdStart ? new Promise((r) => { window.__releaseStart = r; }).then(() => startSession(a)) : startSession(a),
    decisions: ({ mediaId, capabilities, options }) => call("POST", "/playback/decisions", { media_id: mediaId, capabilities, options }),
    media_tracks: ({ mediaId }) => call("GET", `/playback/media/${mediaId}/tracks`).then((t) => { if (t?.duration) window.__fakeDuration = t.duration; return t; }),
    subtitle_text: ({ url }) => fetch(`${B}/raw/${url}`).then((r) => r.text()),
    patch_session: ({ id, changes }) => call("PATCH", `/playback/sessions/${id}`, changes),
    session_info: ({ id }) => call("GET", `/playback/sessions/${id}`),
    stop_session: ({ id }) => call("DELETE", `/playback/sessions/${id}`).then(() => null),
    heartbeat: ({ id, position, paused }) => call("POST", `/playback/sessions/${id}/progress`, { position, paused }).then(() => null),
  };

  window.__calls = [];
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
    transformCallback(cb, once) { const id = next++; cbs.set(id, (m) => { if (once) cbs.delete(id); cb(m); }); return id; },
    unregisterCallback(id) { cbs.delete(id); },
    convertFileSrc(path, proto) { return `${B}/${proto}/${path}`; },
    async invoke(cmd, args = {}, options) {
      window.__calls.push(cmd);
      if (cmd.startsWith("plugin:window|")) return null;
      if (cmd.startsWith("plugin:player|")) { const f = plugin[cmd.slice(14)]; if (!f) throw `no ${cmd}`; return f(args); }
      const f = cmds[cmd];
      if (!f) throw `unknown command ${cmd}`;
      return f(args, options);
    },
  };
})();
