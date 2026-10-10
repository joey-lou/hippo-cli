# Pi adapter

Pi loads this adapter as a skill and a session extension. Both call the `hippo`
command. JSON shapes and exit codes are in [`adapters/README.md`](../README.md).

```bash
hippo adapter install pi
```

From a checkout, `install.sh` links this working tree instead. Either way you get:

- `~/.pi/agent/skills/use-memory`: when to recall and save. It is the same file as the Cursor adapter's skill.
- `~/.pi/agent/extensions/hippo.ts`:
  - At session start it runs `hippo reindex --changed` and injects `hippo digest`.
  - After Pi's `edit` or `write` tool changes a file, it runs `hippo sync --file <path>`.
  - Before Pi settles, it runs `hippo status --json` and, once per prompt, asks the model to report unsynced work.

`hippo add` and `hippo update` commit and push on their own, so a Pi `bash` call needs no hook.

On `cursor/*` models the Cursor SDK runs the Cursor adapter's hooks for file
edits and the end-of-turn check, so the extension skips those turns. The Cursor
session-start hook skips its digest when `PI_CODING_AGENT` is set, which leaves
the Pi digest as the only one.

`bin/hippo-pi.sh` is the same contract as a shell command, for a harness that
cannot load the extension:

```bash
hippo-pi.sh [--home PATH] digest
hippo-pi.sh [--home PATH] recall TEXT
hippo-pi.sh [--home PATH] save     # JSON on stdin
```
