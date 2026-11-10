#!/usr/bin/env bash
# Runs every scenario against a fresh mock: dev/compare/run.sh <out-dir>.
# Needs the engine (:8097), bridge.py (:8099) and the TV build on Vite (:1420, task dev:tv-web) up; starts and stops the mock and the
# contract proxy (:8098) itself, because the mock's watch state carries over between runs.
# Exits 1 when the proxy reported an exchange off docs/contract/player.openapi.yaml.
set -u
cd "$(dirname "$0")/../.."
out=$1
if ! curl -s localhost:1420/src/lib/platform/target.ts | grep -q '"TAURI_ENV_PLATFORM": "android"'; then
  echo "Vite on :1420 is not serving the TV target. Start it with: task dev:tv-web" >&2
  exit 1
fi
rm -rf "$out"
tmp=${MEDIA_DIR:-/tmp/streamline-player}
python3 dev/mock-streamline.py --listen 127.0.0.1:8081 --secret dev-secret-0123456789abcdef --now 1794225600 \
  --art "$tmp/art" --demo-markers \
  --media film0="$tmp/h.mkv" --media film1="$tmp/h2.mkv" --media film2="$tmp/h3.mkv" --media film3="$tmp/a.mp4" \
  --clip "$tmp/h.mkv" >"$tmp/mock-run.log" 2>&1 &
mock=$!
report=$tmp/contract-$(basename "$out").jsonl
rm -f "$report"
node dev/contract/proxy.ts --listen 127.0.0.1:8098 --upstream http://127.0.0.1:8081 \
  --spec docs/contract/player.openapi.yaml --report "$report" >"$tmp/proxy-run.log" 2>&1 &
proxy=$!
trap 'kill $mock $proxy' EXIT
sleep 2
for s in pages nav panels halts french french2 play flows search settings welcome cast person4 submenu ending backstart; do
  echo "== $s"
  SHOTS=$out timeout 150 node dev/compare/walk.mjs webkit "$s" || echo "FAILED: $s"
done
if [ -s "$report" ]; then
  echo "== contract violations"
  cat "$report"
  exit 1
fi
