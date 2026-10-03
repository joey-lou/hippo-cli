# Hippo

[![Crates.io](https://img.shields.io/crates/v/hippo-cli?logo=rust)](https://crates.io/crates/hippo-cli)
[![License: MIT](https://img.shields.io/crates/l/hippo-cli)](LICENSE)

There are many agent memory setups, but this one is yours to keep and see, in plain markdowns!

`hippo` saves, searches, and updates that memory. Install an adapter and the agent does this during the session. You can still run the commands yourself when you want to look something up or fix a file. Or just edit the markdowns yourself.

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

| Command                                                                           | Purpose                                                               |
| --------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| `hippo query "<text>" [--k N] [--json]`                                           | Ranked matches. A lower score is a better match.                      |
| `hippo add [--title --keywords --tags --category --reason --confidence] [--json]` | Create a memory. The body is read from stdin.                         |
| `hippo update <id> [...] [--json]`                                                | Edit a memory.                                                        |
| `hippo show <id>` / `hippo path <id>`                                             | Print the file or its path.                                           |
| `hippo reindex [--all\|--changed]`                                                | Rebuild the index from markdown.                                      |
| `hippo status [--json]`                                                           | Index drift, git state, and what to do next.                          |
| `hippo digest [--format md\|json]`                                                | Short summary: how many memories, and the top topics.                 |
| `hippo consolidate [--apply] [--json]`                                            | Near-duplicates and contradictions. `--apply` merges duplicates only. |

Exit `0` on success. Exit `2` when input is invalid or the memory does not exist.

## Adapters

Cursor and Pi are included. Each one calls `hippo` and leaves the markdown files alone. The command list, JSON shapes, and exit codes are in [adapters/README.md](adapters/README.md).

They ship inside the `hippo` binary:

```bash
hippo adapter install cursor
hippo adapter install pi
```

**Cursor.** Hooks inject the digest, recall and save go through `hippo`, and memory edits are committed in the data repo. Restart Cursor after install.

**Pi.** A skill plus a session extension. The extension injects the digest. Recall and save still go through `hippo`. Start a new Pi session after install.

From a checkout, `adapters/cursor/install.sh` and `adapters/pi/install.sh` link that working tree instead.

## Releasing

`Cargo.toml` on `main` stays at `0.0.0-dev`. Push a tag on the latest `main` commit and [the release workflow](.github/workflows/release.yml) publishes that version and attaches binaries plus `hippo-adapters.tar.gz`.

```bash
git tag v0.0.3 && git push origin v0.0.3
```
