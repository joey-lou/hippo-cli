# Pi adapter (stub)

Pi has no in-repo hooks. This is a stub so a second harness can use Hippo only
through the CLI contract. The Cursor adapter stays the real integration.

A harness that can run a shell command makes three calls:

1. Session start: `hippo reindex --changed` then `hippo digest`
2. Recall: `hippo query "<text>" --json`
3. Save: `hippo add` / `hippo update` (body on stdin), including `--reason`,
   `--confidence`, and `--force` only when the user explicitly asked to
   remember something

JSON shapes and exit codes (`0` ok, `2` validation or not-found) are in
[`ADAPTER.md`](../../ADAPTER.md).

`bin/hippo-pi.sh` wraps those calls and exits with hippo's status:

```bash
hippo-pi.sh [--home PATH] digest
hippo-pi.sh [--home PATH] recall TEXT
hippo-pi.sh [--home PATH] save     # JSON on stdin
```

`save` reads a JSON object with `title`, `keywords` (array), `body`, and
optional `reason`, `confidence`, and `category`, then runs `hippo add` with
the body on stdin.
