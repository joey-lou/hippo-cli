# Hippo

Personal memory. Each memory is a markdown file. `hippo` searches and edits them.

## Install

```bash
cargo install hippo-cli
```

From a checkout:

```bash
cargo install --path .
```

Either command installs `hippo`.

## Data home

Resolution order: `--home`, then `HIPPO_HOME`, then `home` in `~/.config/hippo/config.toml`.

```toml
home = "/path/to/your-memory-repo"
```

```bash
hippo reindex --all
```

The search index is `<home>/.index/memory.db`.

## Commands

| Command | Purpose |
|---|---|
| `hippo query "<text>" [--k N] [--json]` | Ranked recall. Lower score is better. |
| `hippo add [--title --keywords --tags --category --source --reason --confidence --force --from-json] [--json]` | Create a memory. Body on stdin, or a full record via `--from-json`. |
| `hippo update <id> [...] [--json]` | Edit a memory. |
| `hippo consolidate [--apply] [--json]` | Near-duplicates and contradictions. `--apply` merges duplicates only. |
| `hippo reindex [--all\|--changed]` | Rebuild the index. |
| `hippo manifest [--format md\|json]` | Full catalog. |
| `hippo digest [--format md\|json]` | Short primer: count and top topics. |
| `hippo status [--json]` | Index drift, git state, suggested actions. |
| `hippo show <id>` / `hippo path <id>` | Raw file or its path. |

`--home` and `--scope` go before the subcommand. Exit `0` on success, `2` for a validation error or a missing memory.

## Cursor

```bash
adapters/cursor/install.sh
```

The hooks call `hippo` on `PATH`. Restart Cursor after installing.
