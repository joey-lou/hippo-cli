#!/usr/bin/env bash
# Hippo afterFileEdit hook: when Cursor edits a memory markdown file, reindex
# it and commit + push the data repo. No output; fails open.
# This does not see `hippo add` (that writes via the CLI, not the editor).

input="$(cat)"

hippo_bin="$(command -v hippo 2>/dev/null || true)"
if [ -z "$hippo_bin" ] && [ -x "$HOME/.local/bin/hippo" ]; then
  hippo_bin="$HOME/.local/bin/hippo"
fi
[ -z "$hippo_bin" ] && exit 0

home="${HIPPO_HOME:-}"
if [ -z "$home" ] && [ -f "$HOME/.config/hippo/config.toml" ]; then
  home="$(python3 -c "import tomllib;print(tomllib.load(open('$HOME/.config/hippo/config.toml','rb')).get('home',''))" 2>/dev/null || true)"
fi
[ -z "$home" ] && exit 0

file_path="$(printf '%s' "$input" | python3 -c "import json,sys;print(json.load(sys.stdin).get('file_path',''))" 2>/dev/null || true)"

case "$file_path" in
  "$home"/memory/*.md)
    "$hippo_bin" reindex --changed >/dev/null 2>&1 || true
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    "$script_dir/hippo-sync-memory.sh" >/dev/null 2>&1 || true
    ;;
esac
exit 0
