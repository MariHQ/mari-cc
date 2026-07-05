# factcheck — check a document's claims against known facts

Ground each factual claim in a doc against `FACTS.md` (or an explicit `--source <file>`). A
number/date mismatch or a semantic contradiction is an **error**; a claim the facts neither
support nor contradict is **advisory** (absence isn't disproof). The check is deterministic +
a small local NLI model — no network for the grounding itself.

## Two depths

**Whole-sentence (default).** Fast, fully local, no Claude needed:
```
mari factcheck <file> [--source <src>] [--models]
```
`--models` adds the local NLI entailment pass. Run this for a quick check, or when no session
is around.

**Atomic-claim (`--decompose`) — YOU are the decomposer.** Splitting each sentence into
self-contained claims catches one bad clause buried in an otherwise-true sentence. Decomposition
is a generative task, so **you do it in-session — the CLI never calls a model or `claude` for
it.** Three steps:

1. **Emit the sentences to decompose:**
   ```
   mari factcheck <file> [--source <src>] --emit-claim-targets --json
   ```
   → `{"targets": ["<sentence 0>", "<sentence 1>", …]}` (document order).

2. **Split each target into atomic claims yourself.** An atomic claim states exactly one fact, is
   self-contained, and resolves every pronoun/reference to an explicit name from the sentence.
   Copy numbers, dates, names, and quantities **verbatim** — never invent or alter them. Ignore
   opinions, questions, instructions, and hedges; a sentence with no checkable fact gets `[]`.
   Write a JSON file aligned to the targets (same order, one entry per target):
   ```json
   [["Mari was built in 2026.", "Mari ships 90 rules."], [], ["Mari runs on the CPU."]]
   ```
   (The labeled form `[{"i": 0, "claims": [...]}, …]` is also accepted.)

3. **Ground the claims you wrote:**
   ```
   mari factcheck <file> [--source <src>] --claims <your-claims.json>
   ```
   mari does the deterministic + NLI grounding on your claims and reports per-claim verdicts.

Standalone (a human runs `--decompose` with no session and no `--claims`), there's no decomposer,
so mari just falls back to whole-sentence NLI grounding and prints a note. The atomic tier only
runs through this skill (or any `--claims` file). The CLI never invokes Claude on its own.

## Report
- Lead with `error`s (contradictions, number/date mismatches) — quote the claim and the fact.
- Then `advisory` unsupported claims — "verify or cite."
- Each finding anchors to its parent sentence (atomic claims have no source offset).
- Offer to fix the contradicted claims or add the missing facts to `FACTS.md`.

## Notes
- Needs the ML sidecar for the NLI step (`.venv` with `ml/requirements.txt`). Without it, only the
  pure-deterministic number/date/entity checks run.
- `--deep` / `--ground=attention` adds the on-device Lookback-Lens attention pass (needs `--source`).
- Keep the `--claims` file in the scratchpad, not the repo.
