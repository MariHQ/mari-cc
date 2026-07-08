# tag — curate the knowledge base with trust statuses

Tag a repo file or an indexed doc ref with one curation status so search, factcheck, and hooks
know what to trust. Statuses: `canonical` · `stale` · `deprecated` · `draft` · `internal` ·
`customer-facing` · `needs-review`. Tags live in the catalog `tags` table, keyed
`(target_type, target_id)` — team sharing rides the shared warehouse like every other catalog
table. A ref that resolves to an indexed doc is stored as a `doc` tag; an uncataloged repo path
is stored as a `ref` tag and promoted to a `doc` tag once it gets indexed. Tagging needs a built
catalog — run `mari sync` first.

## Flow
1. Confirm the status meaning with the user before writing — tagging is a team decision, not a
   personal note. `canonical` = source of truth; `stale` = known out of date; `deprecated` =
   superseded; `draft` = not yet trusted; `internal` = not customer-facing; `customer-facing` =
   published surface; `needs-review` = flagged for a human.
2. Apply it:
   ```
   mari tag <path-or-ref> <status> [--note "…"] [--superseded-by <ref>]
   ```
   `<path-or-ref>` is a repo path or an indexed doc ref (`source:doc_id`).
3. When tagging `deprecated`, pass `--superseded-by <successor-ref>` so the deprecation carries
   its replacement pointer — it records a confirmed `replaces` lineage edge from the successor to
   the deprecated doc, and `deprecated` search hits print that successor. (Errors if the successor
   ref does not resolve.)
4. Review or undo:
   ```
   mari tag list [--status <S>] [--json]
   mari tag remove <path-or-ref>
   ```
   `remove` clears the tag but leaves any lineage edge in place — lineage is history, not tag
   metadata.

## Bulk: `tag analyze` (§10.4)

Tagging by hand does not scale past a few dozen docs. `tag analyze` does the whole knowledge base
in one session: the CLI extracts bounded deterministic context, the agent judges from it, asks
about what it can't infer, and writes the agreed tags via `mari tag`. See
`reference-tag-analyze.md`. Trigger phrases: "tag the knowledge base", "which docs are stale?",
"what should Claude trust?", "mark the deprecated stuff". Never runs unprompted.

## Effects
- **Search ranking:** fused scores multiply by `search.tag_boosts` — `canonical` up-ranked
  (×1.15), `stale` (×0.7) and `deprecated` (×0.5) down-ranked, `draft` ×0.9. `--tag`/`--no-tag`
  filter `search`/`recent`.
- **Display:** every hit shows its tag badge; `deprecated` hits show the replacement via the
  `replaces` lineage edge (created with `--superseded-by`).
- **Factcheck trust:** sources tagged `stale`/`deprecated` cannot *support* a claim — such claims
  report as `unsupported-claim` with a "source is stale" note. `canonical` sources are preferred
  evidence; `deprecated` content is a contradiction candidate.
- **Hooks:** editing a file tagged `stale` or `deprecated` produces an advisory notice;
  `needs-review` files are surfaced by `mari audit kb`; `internal` warns if referenced from
  customer-facing docs.

## Guardrails
- Confirm the status meaning with the user when asked to tag something — don't guess intent.
- One status per target; changing it is a team-visible edit to the shared catalog, so say what
  changed.
- When tagging `deprecated`, always offer `--superseded-by` to link the successor.

Leans on: `search` tag boosts, `factcheck` trust rules, edit-hook advisories, `audit kb`,
`lineage` (the `replaces` edge).
