#!/usr/bin/env bash
# Hippo afterShellExecution hook: `hippo add`, `hippo update`, and
# `hippo consolidate --apply` are the memory writes. Commit and push the data
# repo as soon as one of those commands finishes.
# Quiet. Fails open.

input="$(cat)"

command="$(printf '%s' "$input" | python3 -c "import json,sys; print(json.load(sys.stdin).get('command',''))" 2>/dev/null || true)"
is_write=0
if printf '%s' "$command" | grep -Eq '(^|[[:space:];|&])hippo[[:space:]]+(add|update)([[:space:]]|$)'; then
  is_write=1
elif printf '%s' "$command" | grep -Eq '(^|[[:space:];|&])hippo[[:space:]]+consolidate([[:space:]]|$)' \
  && printf '%s' "$command" | grep -Eq -- '--apply'; then
  is_write=1
fi
[ "$is_write" -eq 1 ] || exit 0

code="$(printf '%s' "$input" | python3 -c "
import json, sys
data = json.load(sys.stdin)
for key in ('exit_code', 'exitCode', 'status'):
    if key in data and data[key] is not None:
        print(data[key])
        break
" 2>/dev/null || true)"
if [ -n "$code" ] && [ "$code" != "0" ]; then
  exit 0
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
"$script_dir/hippo-sync-memory.sh" >/dev/null 2>&1 || true
exit 0
