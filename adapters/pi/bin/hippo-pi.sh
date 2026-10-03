#!/usr/bin/env bash
# Pi harness command. Talks to Hippo only through the CLI (see adapters/README.md).
# A live harness may ignore failures (fail-open). This script exits with
# hippo's status so the contract is testable. Does not source the shell rc.

set -u

usage() {
  echo "usage: hippo-pi.sh [--home PATH] digest [args...]" >&2
  echo "       hippo-pi.sh [--home PATH] recall TEXT" >&2
  echo "       hippo-pi.sh [--home PATH] save    # JSON on stdin" >&2
}

home=""
if [ "${1:-}" = "--home" ]; then
  if [ -z "${2:-}" ]; then
    echo "hippo-pi.sh: --home requires a path" >&2
    usage
    exit 2
  fi
  home="$2"
  shift 2
fi

if [ $# -lt 1 ]; then
  usage
  exit 2
fi
cmd="$1"
shift

hippo_bin="$(command -v hippo 2>/dev/null || true)"
if [ -z "$hippo_bin" ] && [ -x "$HOME/.local/bin/hippo" ]; then
  hippo_bin="$HOME/.local/bin/hippo"
fi
if [ -z "$hippo_bin" ]; then
  echo "hippo: command not found" >&2
  exit 127
fi

# Pass --home on the command line. Do not export HIPPO_HOME.
invoke_hippo() {
  if [ -n "$home" ]; then
    "$hippo_bin" --home "$home" "$@"
  else
    "$hippo_bin" "$@"
  fi
}

workdir=""
cleanup() {
  [ -n "$workdir" ] && rm -rf "$workdir"
}
trap cleanup EXIT

case "$cmd" in
  digest)
    invoke_hippo digest "$@"
    exit $?
    ;;
  recall)
    if [ $# -lt 1 ]; then
      usage
      exit 2
    fi
    invoke_hippo query "$*" --json
    exit $?
    ;;
  save)
    if [ $# -ne 0 ]; then
      echo "hippo-pi.sh: save reads JSON on stdin and takes no arguments" >&2
      exit 2
    fi
    workdir="$(mktemp -d)"
    body_file="$workdir/body"
    meta_file="$workdir/meta"
    python3 -c '
import json, sys

body_path = sys.argv[1]
try:
    data = json.load(sys.stdin)
except json.JSONDecodeError as err:
    sys.stderr.write("invalid JSON: %s\n" % err)
    sys.exit(2)
if not isinstance(data, dict):
    sys.stderr.write("save expects a JSON object\n")
    sys.exit(2)
missing = [key for key in ("title", "keywords", "body") if key not in data]
if missing:
    sys.stderr.write("missing %s\n" % ", ".join(missing))
    sys.exit(2)
if not isinstance(data["keywords"], list):
    sys.stderr.write("keywords must be an array\n")
    sys.exit(2)
if not isinstance(data["body"], str):
    sys.stderr.write("body must be a string\n")
    sys.exit(2)

def reject_nul(value, label):
    if isinstance(value, str) and "\x00" in value:
        sys.stderr.write("%s contains a NUL byte\n" % label)
        sys.exit(2)

reject_nul(data.get("title"), "title")
reject_nul(data.get("body"), "body")
reject_nul(data.get("reason"), "reason")
reject_nul(data.get("confidence"), "confidence")
reject_nul(data.get("category"), "category")
for item in data["keywords"]:
    reject_nul(item, "keywords")

with open(body_path, "w", encoding="utf-8") as handle:
    handle.write(data["body"])

def emit(value):
    if value is None:
        value = ""
    sys.stdout.buffer.write(str(value).encode("utf-8") + b"\0")

emit(data.get("title"))
emit(",".join(str(item) for item in data["keywords"]))
emit(data.get("reason"))
emit(data.get("confidence"))
emit(data.get("category"))
' "$body_file" >"$meta_file"
    py_status=$?
    if [ "$py_status" -ne 0 ]; then
      exit "$py_status"
    fi

    title=""
    keywords=""
    reason=""
    confidence=""
    category=""
    {
      IFS= read -r -d '' title || true
      IFS= read -r -d '' keywords || true
      IFS= read -r -d '' reason || true
      IFS= read -r -d '' confidence || true
      IFS= read -r -d '' category || true
    } <"$meta_file"

    args=(add --title "$title" --keywords "$keywords")
    if [ -n "$reason" ]; then
      args+=(--reason "$reason")
    fi
    if [ -n "$confidence" ]; then
      args+=(--confidence "$confidence")
    fi
    if [ -n "$category" ]; then
      args+=(--category "$category")
    fi

    err_file="$workdir/err"
    invoke_hippo "${args[@]}" <"$body_file" 2>"$err_file"
    status=$?
    # Surface hippo's stderr (exit 2 is the validation/not-found contract).
    cat "$err_file" >&2
    exit "$status"
    ;;
  *)
    echo "hippo-pi.sh: unknown subcommand '$cmd'" >&2
    usage
    exit 2
    ;;
esac
