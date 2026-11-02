#!/usr/bin/env bash
# PostToolUse hook for Edit/Write: format the changed file based on its path.
#
# Args: $1 = project root (absolute path)
# Hook stdin: JSON with .tool_input.file_path (absolute path).

set -uo pipefail

root=${1:-}
[ -z "$root" ] && { echo "post-edit.sh: missing project root arg" >&2; exit 1; }

f=$(jq -r '.tool_input.file_path // empty')
[ -z "$f" ] && exit 0

case "$f" in
"$root"/*) ;;
*) exit 0 ;;
esac

# ponytail: format only. Clippy stays in `lint`: with -D warnings it would
# block every half-finished edit. web/ gets a step once it has a formatter.
case "$f" in
*.rs) rustfmt --edition 2024 "$f" ;;
esac
