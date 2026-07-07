# 11 — Deferred, Non-Goals & Nice-to-Have

Explicitly-scoped-out items (so they aren't re-discovered as "gaps"), plus
polish and future features worth recording.

---

## 11.1 — SPEC §20 non-goals (stay out of v1)

These are deliberate non-goals from the spec. Listed so nobody files them as
bugs:

- **No SaaS requirement / no server in core.** A hosted sync layer is an
  optional future backend, not a v1 dependency.
- **No translation.** i18n checks structure and coverage only; Mari never
  translates.
- **No source-code linting.** Prose inside code strings is out of scope for
  v1 (deliberately disabled).
- **No autofix by the detector.** Findings are leads; the editorial *verbs*
  (agent-driven) do rewrites, the detector never edits.
- **No PII redaction of indexed content in v1** (credentials protection
  only) — flagged future work in `07-security.md` #6.
- **No automatic sync / daemons / cron in core.** Users wire their own
  cron/CI around `mari sync`. (A resident model *sidecar* for latency is a
  different thing — `04` #6 — and is opt-in.)
- **Legacy binary Office (`.doc`/`.ppt`) unsupported.** Modern OOXML/ODF is
  supported (`office.rs`).

---

## 11.2 — Lineage proposal generation (P3, L)

**Current.** `mari lineage add/list/confirm/reject` curates edges by hand;
the hook fires `⛓` notices on confirmed edges; nudges are the hand-declared
counterpart. What's absent is **machine proposal generation** — SPEC's
`lineage refine` (Tier-2 attention proposing span↔span couplings
automatically).

**Design.** Use the attention engine (already present) to propose lineage
edges: for each doc, find code spans it attends to strongly (focus mode over
the surface), propose those as `proposed` edges with `--by llm` provenance
for human confirm/reject. This is a natural extension now that the attention
engine exists.

**Effort.** L. **Priority** P3 (nudges + hand-curation cover the need for v1).

---

## 11.3 — Editorial verbs as first-class CLI (P3, M)

**Current.** `deslop`/`tighten`/`clarify`/etc. are agent-driven skill flows
(SPEC §17 agent tier); the CLI contributes `detect`/`audit`/`factcheck` and
the reference flows. This is correct per spec (the CLI never calls an LLM).

**Nice-to-have.** If a non-agent, purely-deterministic "apply the obvious
fixes" mode is ever wanted (e.g. apply all map-rule replacements
automatically), it would be a new `mari fix` command — but this contradicts
"no autofix by the detector" (§20) and would need a design decision. Record,
don't build.

**Effort.** M. **Priority** P3.

---

## 11.4 — Docsite generation depth (P2, M)

**Current.** `mari docsite`, `mari platform` (detect + scaffold 8 platforms),
`mari check`/`check --deep`, `mari surface`/`explore` exist. The `docsite`
flow is agent-driven end-to-end.

**Nice-to-have.** Deeper API-doc generation (per-symbol pages grounded in
code via the surface extractor + attention focus), and validation that
generated docs stay in sync (the lineage/nudge machinery already supports
this). Incremental polish, not a gap.

**Effort.** M. **Priority** P2.

---

## 11.5 — Reranking (P2, M)

**Current.** `search.rerank.enabled` config exists (default false) and SPEC
§7.5 step 5 describes an opt-in local cross-encoder rerank over the fused
top-pool. The plumbing point exists; the reranker model is not wired.

**Design.** A cross-encoder rerank (fastembed TextCrossEncoder or an ONNX
cross-encoder, sharing the ML-tier runtime from `03`) over the fused
candidates. Missing model → skipped, never fatal (per spec).

**Acceptance.** `search.rerank.enabled=true` reorders the top-pool via the
cross-encoder; absent model degrades gracefully.

**Effort.** M. **Priority** P2 (embeddings + hybrid fusion already give good
ranking).

---

## 11.6 — `search.large_chunks` coarse vector-only chunks (P3, S)

**Current.** SPEC §7.2 describes optional coarse vector-only "large chunks"
(`chunking.large_chunks`, `large_chunk_ratio`). Verify these are actually
produced and excluded from keyword/neighbor queries as specified (there are
tests referencing large chunks; confirm the end-to-end behavior with real
embeddings once `01` #1 is fixed).

**Effort.** S. **Priority** P3.

---

## 11.7 — Recency decay, tag boosts, section merge under real vectors (P2, S)

**Current.** Post-fusion adjustments (§7.5: filters, tag boosts, recency
decay, section merge, scope union) are implemented and unit-tested, but were
exercised mostly on keyword-only scores because embeddings regressed
(`01` #1). Re-verify the ordering and the `round(1−distance, 3)` cosine score
contract end-to-end once vectors work.

**Effort.** S. **Priority** P2.

---

## 11.8 — Observability / debugging aids (P3, S)

**Nice-to-have.** A `--verbose`/`--debug` flag or `MARI_LOG` env for
structured diagnostics (which layers/heads the attention used, which vector
pool size, why a doc was skipped). Useful for support and for the connector
shakedown (`06`). Today diagnostics are ad-hoc eprintln.

**Effort.** S. **Priority** P3.

---

## 11.9 — Config UX polish (P3, S)

**Nice-to-have.** `mari config` already validates paths and coerces types.
Polish: a `mari config doctor` that flags conflicting settings (e.g.
`search.hybrid=false` with no vectors), a `--explain` that shows where each
effective value came from (the layered resolution), and validation that
`ocr.backend`/`embedding.model`/`attention.model` point at reachable
resources.

**Effort.** S. **Priority** P3.

---

## 11.10 — Performance telemetry for the false-positive budget (P3, S)

**Nice-to-have.** Beyond the CI budget (`09` #2), a `mari detect --stats`
that reports findings-per-family-per-1k-words so a team can tune waivers to
their own corpus. Supports the "calibrate to your repo" workflow.

**Effort.** S. **Priority** P3.

---

## Summary: what's genuinely left vs done

**Done and verified** (this session + prior): the full detector + packs +
grammar, all 13 connectors (code + fixtures), OCR (native + optional model),
Office, embeddings + hybrid fusion (code — currently regressed per `01` #1),
the Tier-2 attention engine and all four deep passes, curation, hooks,
lineage curation, cloud sharing, the plugin packaging, and CI (unpushed).

**Genuinely remaining, in priority order:**
1. Fix the embedding regression + clean the tree (`01`).
2. Release prerequisites: license, community files, binaries, install,
   humanizer URL, download integrity (`02`, `05`, `07`, `10`).
3. Live connector shakedown + fixtures (`06`).
4. Robustness: locking, migrations, cloud-vector replication, sync resume
   (`04`).
5. CI breadth: real-inference, false-positive budget, clippy/deny/matrix
   (`09`).
6. ML tier 1: NLI, machine-likelihood, slop-spans (`03`).
7. Scale: ANN + inverted index past ~100k chunks; resident sidecar (`04`).
8. Windows + CUDA (`08`).
9. Reranking, lineage proposals, and the polish in this doc.
