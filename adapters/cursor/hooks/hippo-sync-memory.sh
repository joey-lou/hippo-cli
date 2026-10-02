#!/usr/bin/env bash
# Commit and push memory markdown in the data repo. Quiet on success.
# Exit 0 always (fail open). Prints a one-line error to stderr on failure.

set +e

hippo_bin="$(command -v hippo 2>/dev/null || true)"
if [ -z "$hippo_bin" ] && [ -x "$HOME/.local/bin/hippo" ]; then
  hippo_bin="$HOME/.local/bin/hippo"
fi
[ -z "$hippo_bin" ] && exit 0

home="${HIPPO_HOME:-}"
if [ -z "$home" ] && [ -f "$HOME/.config/hippo/config.toml" ]; then
  home="$(python3 -c "import tomllib; print(tomllib.load(open('$HOME/.config/hippo/config.toml','rb')).get('home',''))" 2>/dev/null || true)"
fi
[ -z "$home" ] || [ ! -d "$home" ] && exit 0

git -C "$home" rev-parse --is-inside-work-tree >/dev/null 2>&1 || exit 0

if ! git -C "$home" add -A -- memory; then
  echo "hippo: git add failed" >&2
  exit 0
fi

if ! git -C "$home" diff --cached --quiet; then
  if ! git -C "$home" commit -m "Record memory updates."; then
    echo "hippo: git commit failed" >&2
    exit 0
  fi
fi

ahead="$(git -C "$home" rev-list --count '@{u}..HEAD' 2>/dev/null || true)"
if [ -n "$ahead" ] && [ "$ahead" != "0" ]; then
  if ! git -C "$home" push; then
    echo "hippo: git push failed" >&2
  fi
fi
exit 0
