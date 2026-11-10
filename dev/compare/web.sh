#!/usr/bin/env bash
# Runs the web scenarios on webkit, firefox and chromium: dev/compare/web.sh <out-dir> [scenario...].
# Needs the web build on Vite (:1430) with VITE_DEV_STREAMLINE=http://localhost:8098, which is the contract proxy this
# script starts in front of a fresh mock (:8081) for every scenario, with the mock flags the scenario exports as mockArgs.
# Exits 1 when a scenario failed or the proxy reported an exchange off docs/contract/player.openapi.yaml.
set -u
cd "$(dirname "$0")/../.."
out=$1
shift
scenarios=${*:-web-shell web-signin web-sso-bad web-tabs web-idle web-revoked}
tmp=${MEDIA_DIR:-/tmp/streamline-player}
rm -rf "$out"
status=0
for s in $scenarios; do
  mapfile -t flags < <(node -e "import('./dev/compare/scenarios/$s.mjs').then((m) => process.stdout.write((m.mockArgs ?? []).join('\n')))")
  for engine in webkit firefox chromium; do
    python3 dev/mock-streamline.py --listen 127.0.0.1:8081 --secret dev-secret-0123456789abcdef --now 1794225600 \
      --art "$tmp/art" --clip "$tmp/h.mkv" "${flags[@]}" >"$tmp/mock-web-$engine.log" 2>&1 &
    mock=$!
    report=$tmp/contract-web-$s-$engine.jsonl
    rm -f "$report"
    node dev/contract/proxy.ts --listen 127.0.0.1:8098 --upstream http://127.0.0.1:8081 \
      --spec docs/contract/player.openapi.yaml --report "$report" >"$tmp/proxy-web-$engine.log" 2>&1 &
    proxy=$!
    sleep 2
    echo "== $s ($engine)"
    SHOTS=$out timeout 150 node dev/compare/walk.mjs "$engine" "$s" || { echo "FAILED: $s on $engine"; status=1; }
    kill $mock $proxy
    wait $mock $proxy 2>/dev/null
    if [ -s "$report" ]; then
      echo "== contract violations ($s, $engine)"
      cat "$report"
      status=1
    fi
  done
done
exit $status
