#!/usr/bin/env bash
# Hippo stop hook: report leftover memory-repo drift or git state.
# The write hooks commit and push. This hook only checks. Quiet when clean.
# loop_limit=1 keeps a notice to one nudge. Fails open.

cat >/dev/null 2>&1

hippo_bin="$(command -v hippo 2>/dev/null || true)"
if [ -z "$hippo_bin" ] && [ -x "$HOME/.local/bin/hippo" ]; then
  hippo_bin="$HOME/.local/bin/hippo"
fi
[ -z "$hippo_bin" ] && { echo '{}'; exit 0; }

export HIPPO_STATUS_JSON="$("$hippo_bin" status --json 2>/dev/null || true)"

python3 <<'PY'
import json, os

raw = os.environ.get("HIPPO_STATUS_JSON", "").strip()
try:
    s = json.loads(raw)
except Exception:
    print("{}")
    raise SystemExit

git = s.get("git") or {}
dirty = bool(git.get("dirty"))
ahead = int(git.get("ahead") or 0)
drift = len(s.get("new", [])) + len(s.get("changed", [])) + len(s.get("removed", []))

if not (dirty or ahead or drift):
    print("{}")
    raise SystemExit

msg = (
    "Hippo: the memory repo still has unsynced work "
    f"(drift={drift}, uncommitted={dirty}, unpushed={ahead}). "
    "The write hooks should have committed and pushed this. "
    "Tell me briefly what is left. Do not commit or push unless I ask."
)
print(json.dumps({"followup_message": msg}))
PY
