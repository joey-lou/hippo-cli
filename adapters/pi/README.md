# Pi adapter

Pi loads this adapter as a skill and a session extension. Both call the `hippo`
command. JSON shapes and exit codes are in [`ADAPTER.md`](../../ADAPTER.md).

```bash
adapters/pi/install.sh
```

That links:

- `~/.pi/agent/skills/use-memory` — when to recall and save
- `~/.pi/agent/extensions/hippo.ts` — session start runs `hippo reindex --changed` and injects `hippo digest`

`bin/hippo-pi.sh` is the same contract as a shell command, for a harness that
cannot load the extension:

```bash
hippo-pi.sh [--home PATH] digest
hippo-pi.sh [--home PATH] recall TEXT
hippo-pi.sh [--home PATH] save     # JSON on stdin
```
