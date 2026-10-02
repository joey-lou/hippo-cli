# hippo-cli

Rust port of [Hippo](https://github.com/joey-lou/hippo), a personal memory system.
Memories stay markdown in a private data repo. This crate builds the `hippo`
command: SQLite FTS5, a cached hashing embedding, and the same CLI contract as
the Python tooling.

The crate is named `hippo-cli` because `hippo` is already taken on
[crates.io](https://crates.io/crates/hippo) and on PyPI. The binary name is
still `hippo`.

## Visibility and a later crates.io release

GitHub stays private until that changes on purpose. `Cargo.toml` sets
`publish = false`, so `cargo publish` refuses to run.

The later release is the crate `hippo-cli` (the name returned 404 on crates.io
on 2026-10-01). A crates.io upload makes the packaged source public, so
revisit GitHub visibility in the same step. Personal memories stay in the
private data repo and are outside this package.

When it is time to publish:

1. Confirm `hippo-cli` is still free: `https://crates.io/api/v1/crates/hippo-cli`.
2. Run `cargo package --list` and read the file list. Expect code, sample
   fixtures, adapters, the license, and this readme.
3. Remove `publish = false` and set the version you want to ship.
4. Run `cargo publish`.

After that, install with `cargo install hippo-cli`. That command installs the
`hippo` binary.

The Python repo remains the previous implementation. Cursor hooks call whatever
`hippo` is on `PATH`. Installing this binary over that command is a separate
switch.

## Build

```bash
cargo test
cargo build --release
```

The release binary is `target/release/hippo`.

Install beside the existing command, without replacing it:

```bash
cargo install --path . --root "$HOME/.local/hippo-cli"
"$HOME/.local/hippo-cli/bin/hippo" --help
```

`cargo install --path .` puts `hippo` in `~/.cargo/bin`, which can come first on
`PATH`. Use the `--root` form until you want hooks to call this binary.

## Data home

Resolution order: `--home` → `HIPPO_HOME` → `home` in
`~/.config/hippo/config.toml`.

```bash
export HIPPO_HOME=/path/to/hippo-campus
hippo reindex --all
```

The index is `<home>/.index/memory.db` and is gitignored in the data repo.

## CLI

| Command | Purpose |
|---|---|
| `hippo query "<text>" [--k N] [--json]` | Ranked recall. Lower score is better. |
| `hippo add [--title --keywords --tags --category --source --reason --confidence --force --from-json] [--json]` | Create a memory. Body comes from stdin, or the whole record from `--from-json`. |
| `hippo update <id> [--reason --confidence --force ...] [--json]` | Edit a memory. Same capture gate as `add`. |
| `hippo consolidate [--apply] [--json]` | Near-duplicates and contradictions. `--apply` merges duplicates only. |
| `hippo reindex [--all\|--changed]` | Rebuild the index. Default is a full rebuild. |
| `hippo manifest [--format md\|json]` | Full catalog. |
| `hippo digest [--format md\|json]` | Session primer: count and top topics. |
| `hippo status [--json]` | Index drift, git state, suggested actions. |
| `hippo show <id>` / `hippo path <id>` | Raw file or path. |

Exit codes: `0` ok, `2` validation or not-found, `1` unexpected.

Global flags `--home` and `--scope` go before the subcommand.

## Tests

```bash
cargo test
```

`tests/parity.rs` covers fixture recall, ranking, the capture gate, hygiene,
CLI JSON, and the pi adapter.

## Adapters

`adapters/cursor` and `adapters/pi` are the same shell adapters as the Python
repo. They call `hippo` on `PATH`. See [ADAPTER.md](ADAPTER.md).
