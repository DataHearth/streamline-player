#!/usr/bin/env bash
# Runs the web scenarios on webkit, firefox and chromium: dev/compare/web.sh <out-dir> [scenario...].
# Needs the web build on Vite (:1430) with VITE_DEV_STREAMLINE=http://localhost:8098, which is the contract proxy this
# script starts in front of a fresh mock (:8081) for every scenario, with the mock flags the scenario exports as mockArgs.
# A scenario that exports ownServer = true serves its own build (web-base on :1450) and skips that check.
# A scenario with target "desktop" runs on the desktop build instead (Vite on :1440, task dev:desktop-web), which reaches the
# mock through bridge.py (:8099), started here.
# Exits 1 when a scenario failed or the proxy reported an exchange off docs/contract/player.openapi.yaml.
set -u
cd "$(dirname "$0")/../.."
out=$1
shift
scenarios=${*:-web-shell web-signin web-sso-bad web-tabs web-idle web-revoked web-home web-empty web-noart web-library web-detail web-watched web-missing desk-home desk-empty desk-noart desk-library desk-detail desk-watched desk-missing web-search web-person desk-search desk-person web-settings web-account-refused desk-settings desk-account-refused desk-welcome desk-pair-expired desk-pair-denied desk-pair-unreachable desk-pair-realclock web-approve web-approve-denied web-approve-bad-code web-approve-signed-out}
tmp=${MEDIA_DIR:-/tmp/streamline-player}
[ -f "$tmp/multi.mkv" ] || dev/compare/make-multi.sh "$tmp" || exit 1
rm -rf "$out"
status=0
for s in $scenarios; do
  scenario_target=$(node -e "import('./dev/compare/scenarios/$s.mjs').then((m) => process.stdout.write(m.target ?? 'tv'))")
  own_server=$(node -e "import('./dev/compare/scenarios/$s.mjs').then((m) => process.stdout.write(m.ownServer ? 'yes' : ''))")
  case $scenario_target in
    desktop) vite=1440 ;;
    web) vite=1430 ;;
    *) echo "$s: target $scenario_target is not run by web.sh" >&2; status=1; continue ;;
  esac
  if [ -z "$own_server" ] && ! curl -s "localhost:$vite/src/lib/platform/target.ts" | grep -q TAURI_ENV_PLATFORM; then
    echo "$s: Vite on :$vite is not up. Start it with: task dev:$([ $vite = 1440 ] && echo desktop-web || echo web)" >&2
    status=1
    continue
  fi
  mapfile -t flags < <(node -e "import('./dev/compare/scenarios/$s.mjs').then((m) => process.stdout.write((m.mockArgs ?? []).join('\n')))")
  for engine in webkit firefox chromium; do
    # One retry on a fresh mock, said out loud: this machine's disk stalls for minutes at a time (the kernel logs
    # "waiting for writeback completion") and a browser launched during one times out. A real fault fails both times.
    for attempt in 1 2; do
      python3 dev/mock-streamline.py --listen 127.0.0.1:8081 --secret dev-secret-0123456789abcdef --now 1794225600 \
        --art "$tmp/art" --clip "$tmp/h.mkv" "${flags[@]}" >"$tmp/mock-web-$s-$engine.log" 2>&1 &
      mock=$!
      report=$tmp/contract-web-$s-$engine.jsonl
      rm -f "$report"
      node dev/contract/proxy.ts --listen 127.0.0.1:8098 --upstream http://127.0.0.1:8081 \
        --spec docs/contract/player.openapi.yaml --report "$report" >"$tmp/proxy-web-$s-$engine.log" 2>&1 &
      proxy=$!
      bridge=
      if [ "$scenario_target" = desktop ]; then
        python3 dev/compare/bridge.py >"$tmp/bridge-web-$s-$engine.log" 2>&1 &
        bridge=$!
      fi
      sleep 2
      echo "== $s ($engine)"
      SHOTS=$out timeout 150 node dev/compare/walk.mjs "$engine" "$s"
      walked=$?
      kill $mock $proxy $bridge
      wait $mock $proxy $bridge 2>/dev/null
      [ $walked = 0 ] && break
      if [ $attempt = 1 ]; then
        echo "RETRY: $s on $engine failed once; $(grep -E '^(Dirty|Writeback):' /proc/meminfo | tr -s ' \n' ' ')"
      else
        echo "FAILED: $s on $engine"
        status=1
      fi
    done
    if [ -s "$report" ]; then
      echo "== contract violations ($s, $engine)"
      cat "$report"
      status=1
    fi
  done
done
exit $status
