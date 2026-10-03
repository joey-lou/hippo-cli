---
name: use-memory
description: >-
  Recall and save the user's personal memories via the `hippo` CLI. Use when the
  user references past context, preferences, or decisions, starts a task that
  overlaps known topics, or when a durable lesson, preference, or correction
  emerges.
---
# Use Memory (Hippo)

The user has a durable memory store. Memories are markdown files. `hippo` is the
only way to read or write them. A session starts with a short digest (count and
top topics), not the catalog and not the bodies.

## Recall

When the user refers to something saved, or the task overlaps the digest topics,
run `hippo query "<text>" --json`, then read only the top hit files at `path`.
A lower score is a better match. If nothing matches, it is not stored. Do not
search chat transcripts for a past decision.

## Save

Save a fact when it is durable, general, and specific. Skip debug steps, one-off
commands, and restatements of something already stored.

1. `hippo query "<topic>" --json`.
2. Same subject: `hippo update <id>` and fold the new fact into that body.
3. No hit: create it. Body on stdin. Pass `--source pi`, `--reason`, and
   `--confidence high`, `medium`, or `low`.

```bash
echo "<markdown body>" | hippo add \
  --title "<concise title>" \
  --keywords "kw1,kw2,kw3" \
  --category "<folder>" \
  --source pi \
  --reason "<one short sentence>" \
  --confidence high
```

Low confidence or secret-like text is rejected unless `--force`. `--force` is
only for an explicit "remember this". Do not use it to store a rejected secret
unless the user insisted on that exact text.

A duplicate notice means fold the fact into the other id. A contradiction notice
means the two memories disagree; resolve one of them. `hippo consolidate --apply`
merges duplicates only.

When the user says "remember this" or "note that", save it. Dedupe first, then
pass `--force`.

Do not commit or push the memory repo yourself.
