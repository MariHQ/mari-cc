# extract — pull candidate facts from the knowledge base into FACTS.md

Mine recent knowledge-base content for checkable factual statements — numbers, dates, pricing,
limits, launch claims — and, with the user's sign-off, record them in `FACTS.md` so
`mari factcheck` can ground documents against them. YOU review; the CLI only proposes.

## Flow
1. Pull the candidates:
   ```
   mari extract facts [--source <key>] [--doc <substr>] [--since D] [--json]
   ```
   `--source` narrows to one connected source, `--doc` to docs matching a substring, `--since`
   to the last D days. Example from PRODUCT.md: `/mari extract facts from recent slack messages
   in #product` → `mari extract facts --source slack --doc "#product" --since 7`.
2. **Review each candidate with the user — never bulk-write.** For each: quote the statement,
   name where it came from, and ask accept / edit / skip. Drop opinions, hedges, and anything
   already in `FACTS.md` (or flag it if the new value contradicts an existing fact).
3. Write each accepted fact:
   ```
   mari facts add "<fact>" --source "<ref>"
   ```
   One fact per line in `FACTS.md`: `- fact  (source)`. Copy numbers, dates, and names
   verbatim from the source — never round, infer, or merge.
4. Close by noting how many facts were added and that `mari factcheck` now treats them as
   ground truth.

## Guardrails
- Never write a fact the user hasn't accepted; extraction proposes, the user disposes.
- A candidate that contradicts an existing fact is a decision for the user, not a silent
  overwrite — surface both versions.
- Prefer candidates from `canonical`-tagged sources; flag any that come from `stale` or
  `deprecated` content.

Leans on: `mari facts add` (the write path), `factcheck` (the consumer), source tags (§ trust).
