#!/usr/bin/env bash
# Hippo afterFileEdit hook: a Cursor edit of a memory markdown file bypasses the
# CLI, so `hippo sync --file` reindexes it and commits + pushes the data repo.
# Other files are skipped by hippo. `hippo add` and `update` sync themselves.
# No output; fails open.

input="$(cat)"

hippo_bin="$(command -v hippo 2>/dev/null || true)"
if [ -z "$hippo_bin" ] && [ -x "$HOME/.local/bin/hippo" ]; then
  hippo_bin="$HOME/.local/bin/hippo"
fi
[ -z "$hippo_bin" ] && exit 0

file_path="$(printf '%s' "$input" | python3 -c "import json,sys;print(json.load(sys.stdin).get('file_path',''))" 2>/dev/null || true)"
[ -z "$file_path" ] && exit 0

"$hippo_bin" sync --file "$file_path" >/dev/null 2>&1 || true
exit 0
