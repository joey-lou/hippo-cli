#!/usr/bin/env bash
# Install the Hippo Cursor adapter into ~/.cursor:
#   - symlinks the hook scripts (live-editable from this repo)
#   - merges hook entries into ~/.cursor/hooks.json (preserving existing hooks)
#   - symlinks the use-memory skill
#   - verifies `hippo`, python3, and data-home resolution
# Idempotent: safe to re-run.

set -euo pipefail

ADAPTER_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CURSOR_DIR="$HOME/.cursor"
HOOKS_DIR="$CURSOR_DIR/hooks"
SKILLS_DIR="$CURSOR_DIR/skills"
HOOKS_JSON="$CURSOR_DIR/hooks.json"

info() { printf '  %s\n' "$1"; }
fail() { printf 'ERROR: %s\n' "$1" >&2; exit 1; }

echo "Installing Hippo Cursor adapter…"

# --- prerequisites --------------------------------------------------------
command -v python3 >/dev/null 2>&1 || fail "python3 not found on PATH."

hippo_bin="$(command -v hippo 2>/dev/null || true)"
[ -z "$hippo_bin" ] && [ -x "$HOME/.local/bin/hippo" ] && hippo_bin="$HOME/.local/bin/hippo"
[ -z "$hippo_bin" ] && fail "hippo not found. Install it with: cargo install --path <hippo-cli repo>"
info "hippo: $hippo_bin"

# --- symlink hook scripts -------------------------------------------------
mkdir -p "$HOOKS_DIR"
for script in hippo-session-start.sh hippo-after-file-edit.sh hippo-after-shell.sh hippo-sync-memory.sh hippo-stop.sh; do
  chmod +x "$ADAPTER_DIR/hooks/$script"
  ln -sfn "$ADAPTER_DIR/hooks/$script" "$HOOKS_DIR/$script"
done
info "hooks: symlinked into $HOOKS_DIR"

# --- merge hooks.json -----------------------------------------------------
python3 - "$HOOKS_JSON" <<'PY'
import json, os, sys

path = sys.argv[1]
try:
    with open(path) as f:
        data = json.load(f)
except FileNotFoundError:
    data = {}
except json.JSONDecodeError:
    sys.exit(f"ERROR: {path} is not valid JSON; fix or remove it and re-run.")

data.setdefault("version", 1)
hooks = data.setdefault("hooks", {})

OURS = {
    "sessionStart": {"command": "./hooks/hippo-session-start.sh", "timeout": 20},
    "afterFileEdit": {"command": "./hooks/hippo-after-file-edit.sh", "timeout": 30},
    "afterShellExecution": {"command": "./hooks/hippo-after-shell.sh", "matcher": "hippo\\s+(add|update|consolidate)\\b", "timeout": 30},
    "stop": {"command": "./hooks/hippo-stop.sh", "timeout": 15, "loop_limit": 1},
}

for event, entry in OURS.items():
    existing = hooks.get(event, [])
    # Drop any prior Hippo entry for this event, keep the user's others.
    existing = [e for e in existing if "hippo-" not in (e.get("command") or "")]
    existing.append(entry)
    hooks[event] = existing

os.makedirs(os.path.dirname(path), exist_ok=True)
with open(path, "w") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
print("  hooks.json: merged (existing hooks preserved)")
PY

# --- install skill --------------------------------------------------------
mkdir -p "$SKILLS_DIR"
ln -sfn "$ADAPTER_DIR/skills/use-memory" "$SKILLS_DIR/use-memory"
info "skill: symlinked use-memory into $SKILLS_DIR"

# --- verify data home -----------------------------------------------------
if "$hippo_bin" status >/dev/null 2>&1; then
  info "data home: resolved OK ($("$hippo_bin" status 2>/dev/null))"
else
  info "data home: NOT resolved yet — set HIPPO_HOME or ~/.config/hippo/config.toml"
fi

echo "Done. Restart Cursor (or it will reload hooks.json on save) to activate."
