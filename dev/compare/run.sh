#!/usr/bin/env bash
# Runs every scenario against a fresh mock: dev/compare/run.sh <out-dir>.
# Needs the engine (:8097), bridge.py (:8099) and Vite (:1420) up; starts and stops the mock itself,
# because its watch state carries over between runs.
set -u
cd "$(dirname "$0")/../.."
out=$1
rm -rf "$out"
tmp=${MEDIA_DIR:-/tmp/streamline-player}
python3 dev/mock-streamline.py --listen 127.0.0.1:8081 --secret dev-secret-0123456789abcdef --now 1794225600 \
  --art "$tmp/art" --demo-markers \
  --media film0="$tmp/h.mkv" --media film1="$tmp/h2.mkv" --media film2="$tmp/h3.mkv" --media film3="$tmp/a.mp4" \
  --clip "$tmp/h.mkv" >"$tmp/mock-run.log" 2>&1 &
mock=$!
trap 'kill $mock' EXIT
sleep 2
for s in pages nav panels halts french french2 play flows search settings welcome cast person4 submenu ending backstart; do
  echo "== $s"
  SHOTS=$out timeout 150 node dev/compare/walk.mjs webkit "$s" || echo "FAILED: $s"
done
