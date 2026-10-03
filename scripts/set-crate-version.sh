#!/usr/bin/env bash
# Set the hippo-cli crate version in Cargo.toml and Cargo.lock.
# Release CI only. main stays at 0.0.0-dev; the git tag is the version.
#
#   ./scripts/set-crate-version.sh 0.0.2
set -euo pipefail

VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
  echo "usage: $0 VERSION" >&2
  exit 1
fi

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.-]+)?(\+[a-zA-Z0-9.-]+)?$ ]]; then
  echo "invalid semver: $VERSION" >&2
  exit 1
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ "$(uname)" == Darwin ]]; then
  sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
  sed -i '' "/^name = \"hippo-cli\"$/,/^version = / s/^version = \".*\"/version = \"$VERSION\"/" Cargo.lock
else
  sed -i "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
  sed -i "/^name = \"hippo-cli\"$/,/^version = / s/^version = \".*\"/version = \"$VERSION\"/" Cargo.lock
fi

echo "Set crate version to $VERSION"
