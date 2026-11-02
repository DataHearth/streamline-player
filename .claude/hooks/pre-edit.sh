#!/usr/bin/env bash
# PreToolUse hook for Edit/Write: block edits to generated files. Exits 2
# (the block code) with a stderr message pointing at the regen command.
#
# Args: $1 = project root (absolute path)
# Hook stdin: JSON with .tool_input.file_path (absolute path).

set -uo pipefail

root=${1:-}
[ -z "$root" ] && { echo "pre-edit.sh: missing project root arg" >&2; exit 1; }

f=$(jq -r '.tool_input.file_path // empty')
[ -z "$f" ] && exit 0

case "$f" in
"$root"/*) rel=${f#"$root/"} ;;
*) exit 0 ;;
esac

reason=""
case "$rel" in
build/* | .svelte-kit/*) reason="SvelteKit output — re-run 'pnpm build' or 'pnpm check' instead" ;;
src-tauri/gen/*) reason="tauri-generated — re-run 'cargo tauri dev' or 'cargo tauri <android|ios> init' instead" ;;
Cargo.lock) reason="cargo lockfile — change Cargo.toml and let cargo update it" ;;
pnpm-lock.yaml | package.json) reason="pnpm-managed — use 'pnpm add|remove' instead" ;;
*) exit 0 ;;
esac

echo "Refusing to edit generated file: $f" >&2
echo "Reason: $reason" >&2
exit 2
