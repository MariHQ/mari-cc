# tag — curate the knowledge base with trust statuses

Tag a repo file or an indexed doc ref with one curation status so search, factcheck, and hooks
know what to trust. Statuses: `canonical` · `stale` · `deprecated` · `draft` · `internal` ·
`customer-facing` · `needs-review`. Tags live in the committed `.mari/config.json`
(`tags.entries`) — team-shared and versioned — and are mirrored into the catalog at
sync/search time.

## Flow
1. Confirm the status meaning with the user before writing — tagging is a team decision, not a
   personal note. `canonical` = source of truth; `stale` = known out of date; `deprecated` =
   superseded; `draft` = not yet trusted; `internal` = not customer-facing; `customer-facing` =
   published surface; `needs-review` = flagged for a human.
2. Apply it:
   ```
   mari tag <path-or-ref> <status> [--note "…"]
   ```
   `<path-or-ref>` is a repo path or an indexed doc ref (`source:doc_id`).
3. When tagging `deprecated`, suggest a lineage edge to the replacement — `deprecated` search
   hits print their replacement pointer only if that edge exists.
4. Review or undo:
   ```
   mari tag list [--status <S>] [--json]
   mari tag remove <path-or-ref>
   ```

## Effects
- **Search ranking:** fused scores multiply by `search.tag_boosts` — `canonical` up-ranked
  (×1.15), `stale` (×0.7) and `deprecated` (×0.5) down-ranked, `draft` ×0.9. `--tag`/`--no-tag`
  filter `search`/`recent`.
- **Display:** every hit shows its tag badge; `deprecated` hits show the replacement via lineage.
- **Factcheck trust:** sources tagged `stale`/`deprecated` cannot *support* a claim — such claims
  report as `unsupported-claim` with a "source is stale" note. `canonical` sources are preferred
  evidence; `deprecated` content is a contradiction candidate.
- **Hooks:** editing a file tagged `stale` or `deprecated` produces an advisory notice;
  `needs-review` files are surfaced by `mari audit kb`; `internal` warns if referenced from
  customer-facing docs.

## Guardrails
- Confirm the status meaning with the user when asked to tag something — don't guess intent.
- One status per path; changing it is a team-visible edit to `.mari/config.json`, so say what
  changed.
- When tagging `deprecated`, always offer the lineage link to the successor.

Leans on: `search` tag boosts, `factcheck` trust rules, edit-hook advisories, `audit kb`.
