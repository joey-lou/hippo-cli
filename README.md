# Hippo

[![Crates.io](https://img.shields.io/crates/v/hippo-cli?logo=rust)](https://crates.io/crates/hippo-cli)
[![License: MIT](https://img.shields.io/crates/l/hippo-cli)](LICENSE)

Memory for agents. Each memory is a markdown file. An agent does not open the store itself. An adapter calls the `hippo` command: a short digest at session start, then `hippo query` when something looks relevant, and `hippo add` or `hippo update` when a fact is worth keeping.

## Install

```bash
cargo install hippo-cli
```

From a checkout: `cargo install --path .`

## Quick start

Point Hippo at a folder, save a memory, then search it.

```bash
mkdir -p ~/memories/memory ~/.config/hippo
printf 'home = "%s/memories"\n' "$HOME" > ~/.config/hippo/config.toml

echo "Break large functions into small, well-named units." | hippo add \
  --title "Prefer small functions" \
  --keywords "coding,style"

hippo query "small functions"
```

Each memory is `<home>/memory/<category>/<id>.md`. The search index is `<home>/.index/memory.db`. Rebuild it with `hippo reindex --all` after editing files by hand.

The data folder is also resolved from `--home`, then `HIPPO_HOME`, then `home` in `~/.config/hippo/config.toml`. `--home` and `--scope` go before the subcommand.

## Commands

| Command | Purpose |
|---|---|
| `hippo query "<text>" [--k N] [--json]` | Ranked matches. A lower score is a better match. |
| `hippo add [--title --keywords --tags --category --reason --confidence] [--json]` | Create a memory. The body is read from stdin. |
| `hippo update <id> [...] [--json]` | Edit a memory. |
| `hippo show <id>` / `hippo path <id>` | Print the file or its path. |
| `hippo reindex [--all\|--changed]` | Rebuild the index from markdown. |
| `hippo status [--json]` | Index drift, git state, and what to do next. |
| `hippo digest [--format md\|json]` | Short summary: how many memories, and the top topics. |
| `hippo consolidate [--apply] [--json]` | Near-duplicates and contradictions. `--apply` merges duplicates only. |

Exit `0` on success. Exit `2` when input is invalid or the memory does not exist.

## Adapters

An agent needs an adapter. This repo includes two.

**Cursor.** Hooks inject the digest, recall and save go through `hippo`, and memory edits are committed in the data repo.

```bash
adapters/cursor/install.sh
```

Restart Cursor after that.

**Pi.** A skill plus a session extension. The extension injects the digest. Recall and save still go through `hippo`.

```bash
adapters/pi/install.sh
```

The command list and exit codes are in [ADAPTER.md](ADAPTER.md).

## Releasing

`Cargo.toml` on `main` stays at `0.0.0-dev`. Push a tag on the latest `main` commit and [the release workflow](.github/workflows/release.yml) publishes that version and attaches binaries plus `hippo-adapters.tar.gz`.

```bash
git tag v0.0.2 && git push origin v0.0.2
```
