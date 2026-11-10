#!/usr/bin/env bash
# Builds the web target under BASE_PATH=/watch, serves it on :1450 (serve.mjs: vite preview ignores a copied build) with a config.json pointing at the contract proxy
# (:8098), and runs web-base on all three engines: dev/compare/web-base.sh <out-dir>. Rebuilds build/ without a base after.
set -u
cd "$(dirname "$0")/../.."
out=$1
tmp=${MEDIA_DIR:-/tmp/streamline-player}
status=0
rm -rf "$tmp/web-base"
BASE_PATH=/watch pnpm build >"$tmp/web-base-build.log" 2>&1 || { cat "$tmp/web-base-build.log"; exit 1; }
cp -r build "$tmp/web-base"
echo '{"streamline":"http://localhost:8098"}' >"$tmp/web-base/config.json"
node dev/compare/serve.mjs "$tmp/web-base" /watch 1450 >"$tmp/web-base-preview.log" 2>&1 &
preview=$!
sleep 3
dev/compare/web.sh "$out" web-base || status=1
kill $preview
wait $preview 2>/dev/null
pnpm build >/dev/null 2>&1 || status=1
exit $status
