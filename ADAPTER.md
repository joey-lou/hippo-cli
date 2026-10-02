# Adapter / Portability Contract

Harnesses integrate with Hippo **only** through the `hippo` CLI (or by importing
`MemoryStore`). Core, schema, store, and data repo never change per harness.
Adding a harness = a new `adapters/<name>/` mapping its events to these commands.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `2` | Validation error or memory not found |
| other | Unexpected error |

## Commands & JSON shapes

All JSON is emitted on stdout with `--json` (or `--format json`).

### `hippo query "<text>" [--k N] [--json]`
```json
[
  {"id": "...", "path": "...", "title": "...", "score": -1.23,
   "snippet": "...", "scope": "global"}
]
```
Lower `score` = better. It is BM25 minus boosts for the active scope, update
recency, and recall frequency, minus `1.0 * max(cosine, 0)` from a cached hashing
embedding (word tokens and character trigrams — not a neural model). That second
signal lifts memories that share subword shape when the FTS tokenizer misses
(`fulltext` vs `full-text`). It does not understand synonyms. A neighbor with no
BM25 hit starts from a miss penalty of 8.0 before those boosts. Treat `score` as
an opaque ordering signal, not a raw BM25 value. Recall unions `global` + active scope.

### `hippo add [--title --keywords --tags --category --scope --source --body --reason --confidence --force] [--json]`
Body is read from stdin when `--body` is omitted. Or pass `--from-json` to read
a full JSON record from stdin. Emits `{"id": "...", "path": "..."}`. When the new memory
overlaps another, `notices` is added (omitted when empty). When lexical capture
checks fire, `warnings` is added (omitted when empty):

```json
{"id": "...", "path": "...",
 "warnings": ["body is shorter than 40 characters"],
 "notices": [
  {"kind": "duplicate", "other_id": "...", "detail": "near-duplicate (score 0.72)"}
]}
```

`kind` is `duplicate` or `contradiction`. Text mode prints notices and
`warning: ...` lines on stderr.

`--reason` is an optional string stored in frontmatter (why the memory was saved).
`--confidence`, when present, must be `high`, `medium`, or `low`. Omitted
confidence is allowed. The write is rejected (exit 2, no file) when confidence
is `low`, or when the lexical assessment is `low` (secret-like text), unless
`--force` is set. `--force` is the manual override. Invalid confidence is also
exit 2 and writes nothing.

### `hippo update <id> [--title --keywords --tags --body --scope --reason --confidence --force] [--json]`
Emits the same object as `add`, including `notices` and `warnings` when present.
The capture gate runs on the memory after the patch is applied and before the
file is written. Omitted body, title, and keywords are read from the current
file. A low `--confidence` or a low assessment is rejected unless `--force`.
Updating other fields does not require `--force`.

### `hippo consolidate [--apply] [--json]`
Reports near-duplicate clusters and contradictions. Does not merge unless
`--apply` is set, and `--apply` merges duplicates only — contradictions stay for
a person to resolve. Reindex never merges, because session start runs it.

```json
{"duplicates": [{"keeper": "...", "members": ["...", "..."], "score": 0.72}],
 "contradictions": [{"a": "...", "b": "...", "tokens": ["pip", "uv"]}],
 "applied": false, "removed": []}
```

The keeper is the most-recalled member, else the most recently updated, else the
lowest id. Absorbed files are deleted and their body is appended to the keeper.

### `hippo reindex [--all|--changed]`
Emits `indexed=N added=N updated=N deleted=N`.

### `hippo manifest [--format md|json]`
Full catalog (every memory's metadata). Not injected at session start; use for
tooling/debugging.
```json
[{"id": "...", "title": "...", "keywords": ["..."], "path": "..."}]
```

### `hippo digest [--format md|json]`
Constant-size session primer: memory count + top topic tags, no bodies and no
per-memory rows. Injected at session start so cost stays flat as the store grows;
the agent recalls on demand via `hippo query`.
```json
{"total": 6, "topics": [{"tag": "hippo", "count": 2}]}
```

### `hippo status [--json]`
```json
{"indexed": 4, "new": [], "changed": [], "removed": [],
 "git": {"is_repo": true, "dirty": false, "ahead": 0},
 "actions": []}
```

## Scope resolution

Active scope: `--scope` → `HIPPO_SCOPE` → `.hippo/scope` marker → `global`.

## Data home resolution

`--home` → `HIPPO_HOME` → `~/.config/hippo/config.toml` (`home = "..."`).
