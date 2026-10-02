---
name: use-memory
description: >-
  Recall and save the user's personal memories via the `hippo` CLI. Use when the
  user references past context/preferences/decisions, starts a task overlapping
  known topics, or when a durable lesson, preference, or correction emerges.
---
# Use Memory (Hippo)

The user has a durable personal memory store ("second brain"). Memories are
markdown files; the `hippo` CLI provides fast recall and capture. At session start
only a compact **digest** (memory count + top topics) is injected — never the full
catalog and never bodies. `hippo query` is the only way to see memory contents.

## Recall (read)

Trigger when the user references prior context ("we discussed…", "like last
time", "my usual…") or starts a task overlapping the session-start topics. Do not
assume a memory's contents from the digest — always `query` to retrieve it.

1. `hippo query "<natural language>" --json` — returns ranked hits with
   `id, path, title, score, snippet` (lower score = better).
2. Read only the top hit files (their `path`) before answering. Don't dump the
   whole store; recall is cheap-first.
3. Hippo is the memory bank. Do not search `~/.cursor/projects` or other chat
   transcripts for a past decision. If the query has no hit, it is not stored.

## Save (write)

Capture is model-judged: decide yourself, save quietly, rarely interrupt. Save a
fact only if it is **durable** (still true next session) AND **general** (matters
beyond this one chat) AND **specific** (actionable, not vague).

**SAVE** — preferences, decisions, processes, conventions, stable project facts,
and corrections:
- "I use `uv`, not pip." / "Always run tests before pushing."
- "We decided the data repo is private; tooling is public-able."
- "My Robinhood rule: confirm account before any trade."
- A correction to something previously stored or assumed.

**SKIP** — anything ephemeral or already-known:
- Debug steps, one-off commands, status checks, transient errors.
- Restating something already stored with no new information.
- Speculation or facts the user hasn't confirmed.

When unsure, prefer NOT saving over adding noise — the store's value is its
signal-to-noise ratio.

### Dedupe before every write

1. `hippo query "<topic>" --json`.
2. **Top hit is the same subject:** `hippo update <id>` and fold the new fact into
   that body. Never create a second memory for the same decision — supersede it.
   Pass `--reason` and `--confidence` the same way as `add`.
3. **No hit on that subject:** create it. Pass `--source cursor` for provenance,
   `--reason` (one short sentence), and `--confidence high|medium|low`:
   ```bash
   echo "<markdown body>" | hippo add \
     --title "<concise title>" \
     --keywords "kw1,kw2,kw3" \
     --tags "cat1" \
     --category "<folder>" \
     --source cursor \
     --reason "<one short sentence>" \
     --confidence high
   ```

   A low-confidence save is rejected unless `--force`. `--force` is only for an
   explicit user "remember this". If Hippo rejects a secret, do not retry with
   `--force` unless the user insisted on saving that exact text.

`hippo add` and `hippo update` print a notice when the write is a near-duplicate
or contradicts another memory. A duplicate notice means fold the new fact into
the other id (or run `hippo consolidate` to review). A contradiction notice
means the two memories disagree — resolve it with `hippo update` on one of them.
`hippo consolidate --apply` merges near-duplicates into the most-recalled memory
and deletes the extras. It does not merge contradictions.

### Manual override (guaranteed path)

When the user explicitly says "remember this" / "note that" / "save this",
**always** save — no judgment needed. Still dedupe first (update vs add), then
persist with `--force`. This overrides the SKIP criteria above and the
low-confidence gate. `--force` is only for that explicit request. If Hippo
rejects a secret, do not retry with `--force` unless the user insisted on
saving that exact text.

## Writing good memories

- Title and keywords carry ranking signal; make them specific and searchable.
- Body holds the full detail in plain markdown.
- Choose a sensible `--category` (e.g. `coding`, `preferences`, `projects`).

## Git

No hook creates a memory. `hippo add` and `hippo update` write the file. The
`afterShellExecution` hook commits and pushes the data repo when one of those
commands finishes. A Cursor edit of a memory markdown file is reindexed, then
committed and pushed, by `afterFileEdit`.

Do not commit or push the memory repo yourself.

The `stop` hook only checks status. It speaks up only when drift, uncommitted
files, or unpushed commits are still left.

## Commands

| Command | Purpose |
|---|---|
| `hippo query "<text>" [--k N] [--json]` | Recall |
| `hippo add [--title --keywords --tags --category --source --reason --confidence --force]` (body on stdin) | Create. Low confidence or secret-like text is rejected unless `--force` |
| `hippo update <id> [--reason --confidence --force ...]` | Edit. Same capture gate as add |
| `hippo consolidate [--apply] [--json]` | Near-duplicates and contradictions. `--apply` merges duplicates only |
| `hippo digest [--format md\|json]` | Session primer: count + top topics |
| `hippo manifest [--format md\|json]` | Full catalog (tooling/debug) |
| `hippo status [--json]` | Drift + git state |
| `hippo show/path <id>` | Inspect |
