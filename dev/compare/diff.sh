#!/usr/bin/env bash
# Compares two walk.mjs output dirs: JSON by text, PNGs byte for byte.
set -u
before=$1 after=$2
bad=0
for f in "$before"/*.json; do
  n=$(basename "$f")
  # Svelte's scoped class hash follows the file's path, so moving a component changes it.
  diff -u <(sd 'svelte-[a-z0-9]+' svelte-x <"$f") <(sd 'svelte-[a-z0-9]+' svelte-x <"$after/$n") >/dev/null 2>&1 || { echo "differs: $n"; bad=1; }
done
for f in "$before"/*.png; do
  n=$(basename "$f")
  cmp -s "$f" "$after/$n" || { echo "differs: $n"; bad=1; }
done
exit $bad
