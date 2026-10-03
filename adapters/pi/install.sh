#!/usr/bin/env bash
# Install the Hippo Pi adapter:
#   - skill  ~/.pi/agent/skills/use-memory
#   - extension  ~/.pi/agent/extensions/hippo.ts
# Idempotent. Requires `hippo` on PATH.

set -euo pipefail

ADAPTER_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PI_DIR="${PI_AGENT_DIR:-$HOME/.pi/agent}"

info() { printf '  %s\n' "$1"; }
fail() { printf 'ERROR: %s\n' "$1" >&2; exit 1; }

echo "Installing Hippo Pi adapter…"

hippo_bin="$(command -v hippo 2>/dev/null || true)"
[ -z "$hippo_bin" ] && [ -x "$HOME/.local/bin/hippo" ] && hippo_bin="$HOME/.local/bin/hippo"
[ -z "$hippo_bin" ] && fail "hippo not found. Install it with: cargo install hippo-cli"
info "hippo: $hippo_bin"

mkdir -p "$PI_DIR/skills" "$PI_DIR/extensions"
ln -sfn "$ADAPTER_DIR/skills/use-memory" "$PI_DIR/skills/use-memory"
ln -sfn "$ADAPTER_DIR/extensions/hippo.ts" "$PI_DIR/extensions/hippo.ts"
info "skill: $PI_DIR/skills/use-memory"
info "extension: $PI_DIR/extensions/hippo.ts"

echo "Done. Start a new Pi session so the digest loads."
