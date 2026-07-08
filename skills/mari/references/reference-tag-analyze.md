# tag analyze — bulk-curate the knowledge base in one session

`tag analyze` finds the docs that need **special treatment** (stale, deprecated, draft,
internal-only, contradicted, …) and tags them. Everything else stays untagged — an untagged doc
is assumed current, full trust, normal ranking. The flow is not trying to tag everything.

The CLI extracts deterministic context and never calls an LLM; the agent judges from that context,
asks the user about what it cannot infer, and writes tags via `mari tag`. Teams usually run it
once at setup and afterwards only tag docs that become a problem in search.

**Trigger:** `/mari tag analyze`, `/tag analyze`, or intent phrasing — "tag the knowledge base",
"which docs are stale/out of date?", "what should Claude trust?", "mark the deprecated stuff".
Never runs from a hook, sync step, or background job — same trust posture as `/sync`.

## Steps

1. **Extract.** Run `mari tag analyze` (scope it to any path/source the user named):
   ```
   mari tag analyze [path…] [--status <S>] [--source <key>] [--json]
   ```
   - `path…` — restrict to repo paths/globs; default = whole knowledge base (repo + `_global`).
   - `--status <S>` — restrict to docs already tagged `S` (re-reviewing existing tags); default =
     untagged docs.
   - `--source <key>` — restrict to one connector source.
   - `--json` — machine-shaped repo-context block + cards.

   Output is a **repo-context block** (current project version from manifest + latest semver git
   tag, shown together when they disagree; version-marker conventions observed across doc
   paths/titles/front matter) followed by one bounded **context card** per doc: identity + current
   tag, outline, lede, version markers, versioned siblings, near-dup pointer, inbound-link count,
   draft markers, modified time. Cards are capped (~1 KB each) so a whole knowledge base fits one
   session. Requires an index for doc-ref cards (repo-path cards work without one).

2. **Judge.** Decide from the cards; read a doc (`mari doc`, `mari related`) only when a card
   isn't enough. Common sense, not rules — weigh each starting point against everything else:
   - A version marker older than the current project version usually means `deprecated` (its
     search semantics — down-ranked but searchable, replacement pointer shown — are exactly right
     for a superseded doc). Not `stale`: a frozen version doc is finished, not out of date. Ask
     the user whether the old version is still an *operative supported surface* (an LTS most users
     run) — if so it's current product and stays untagged. Versions are never tags; the marker
     only informs the category.
   - Supersession language ("superseded by", "moved to", "no longer maintained"), a newer
     near-duplicate, or a newer same-stem sibling → `deprecated`.
   - A doc whose lineage counterpart changed after its last edit, or whose dated claims have
     passed → leans `stale`.
   - Draft/WIP markers, heavy TODO density, `drafts/` paths → `draft`.
   - Help-center sources (Zendesk/Salesforce/HubSpot KB) and docsite pages → `customer-facing`;
     internal channels/spaces → `internal`. You can't know a team's boundary — ask once and
     generalize.
   - `audit kb` contradiction candidates and claims unsupported by FACTS.md → `needs-review`.
   - Heavily-linked hub docs, FACTS.md sources, root spec-position files (README, SPEC,
     PRODUCT.md) → `canonical`.

3. **Plan & ask.** Present one plan of **all** proposed tags (ref, status, one-line rationale,
   grouped by status; `deprecated` entries show their proposed successor). Ask **grouped**
   questions — one per pattern, not per file ("these 12 docs match `flink-doc-1.15.*` and the
   project is on 1.18 — tag them `deprecated`, or is 1.15 still a supported release your users run
   (leave untagged)?"). The user edits the plan: change statuses, drop entries, add docs you
   missed.

4. **Apply.** Write each agreed tag:
   ```
   mari tag <ref> <status> --note "analyze: <rationale>" [--superseded-by <ref>]
   ```
   Add `--superseded-by` whenever the successor is known (a newer sibling or near-duplicate found
   during judging) so the deprecation carries its replacement pointer. Print a summary of tags per
   status and what was deliberately left untagged.

## Guardrails
- Proposals only until the user confirms — tags are a team-visible edit to the shared catalog.
- Default posture is *untagged = current*: when unsure whether a doc needs special treatment,
  leave it untagged and say so.
- Don't tag versions as versions; a version marker informs a category (usually `deprecated`), it
  is not itself a status.

Leans on: `tag` (writes), `lineage` (the `replaces` edge), `search`, `audit kb`, `doc`/`related`.
