# 04 — Scale & Robustness

Mari works today at the scale the authors have tested (a handful of files, a
few thousand chunks). These are the items that keep it correct and fast as
real teams point it at large repos, big Slack workspaces, and shared indexes.

---

## 4.1 — Search scaling past ~100k chunks (P1→P2, L)

**Current.** The keyword side does a full `chunks`-table scan per query and
scores in Rust; the vector side brute-forces cosine over every Lance row in
one DuckDB pass. Fine to roughly 50–100k chunks; beyond that both are linear
and latency grows.

**Where.** `src/index/search.rs` (`search_hits`, keyword scoring),
`src/index/vector.rs` (`duckdb_cosine_topk` brute force, `read_dataset`).

**Design.**
- **Keyword:** move to DuckDB FTS extension or a `tantivy` inverted index so
  candidate retrieval is sublinear. SPEC §7.7 lists `tantivy` as the sanctioned
  alternative to the count-based scorer.
- **Vector:** build a Lance ANN index (IVF-PQ) once the row count crosses the
  floor SPEC §7.3 anticipates (~4096; partitions ≈ √rows, capped 1024).
  `read_dataset` currently reads *all* rows into memory — replace with a
  Lance ANN `nearest` query so only the pool is materialized.
- Keep the current brute-force path for small indexes (it's exact and simpler);
  switch on a row-count threshold, logged.

**Acceptance.** A synthetic 250k-chunk index answers a query in well under a
second and does not read the whole vector set into RAM; results match the
brute-force baseline on the top-k for a small index.

**Effort.** L. Split into two independent PRs (keyword, vector).

---

## 4.2 — Workspace locking / concurrent-sync safety (P1, M)

**Current.** Two concurrent `mari sync` runs collide on the DuckDB catalog
with an ungraceful error; the Lance dataset is rewritten *whole* on any
deletion (`vector.rs::write_dataset` uses `WriteMode::Overwrite`), so a
concurrent reader can see a half-written dataset.

**Where.** `src/index/sync.rs` (top of `run`), `src/index/vector.rs`
(`write_dataset`), `src/index/mod.rs` (`open_catalog`).

**Design.**
- A per-workspace advisory lockfile (`<workspace>/sync.lock`, PID + timestamp)
  acquired at the top of `sync`; a second sync exits cleanly with "another
  sync is in progress (pid N)" rather than corrupting state.
- Lance: prefer delete-predicates / append over full overwrite so readers
  never observe a truncated dataset; if staying with overwrite, write to a
  temp dataset dir and atomically swap.
- Read commands already tolerate a stale replica; ensure they never block on
  the lock.

**Acceptance.** Two `mari sync` invocations in parallel: one completes, the
other exits 1 with a clear message; the catalog is never corrupted; a
concurrent `mari search` always sees a consistent (possibly stale) dataset.

**Effort.** M.

---

## 4.3 — Schema migrations (P1, M)

**Current.** `schema_meta` records versions (`embedding.model`,
`extractor.version`, etc.) but nothing *upgrades* an existing catalog. A
schema change means a manual rebuild. Worse: the embedding-dim change
(768→1024) means any catalog built before the Qwen swap has incompatible
vectors, and there's no guard.

**Where.** `src/index/mod.rs` (`ensure_schema`, `set_meta`).

**Design.**
- A `schema_version` key + an ordered list of migration steps run at
  `open_catalog` time inside a state flag (SPEC §8.6 calls for "legacy-format
  catalogs migrate idempotently behind a state flag").
- An **embedding-identity guard**: if `embedding.model` or `embedding.dims`
  in the catalog differs from the build's, refuse vector search with a clear
  "run `mari sync --rebuild`" message instead of silently comparing
  incompatible vectors (SPEC §7.1 requires this; `mari status` already warns,
  but search should hard-guard). This also catches the 768↔1024 hazard from
  the jina→Qwen swap.

**Acceptance.** Opening a catalog written by an older schema upgrades it
without data loss; opening one with a mismatched embedding identity refuses
vector search loudly and points at `--rebuild`.

**Effort.** M.

---

## 4.4 — Sync resilience and partial-failure UX (P2, M)

**Current.** `sync` tolerates one bad doc and one bad source (§6.0), but:
- A crash mid-sync can leave the `embedded`/vector state partially written
  with no resume marker beyond per-doc content hashes.
- Rate-limit backoff is per-request; there's no global budget or a "this
  source is rate-limited, resume later" path.
- Progress is line-per-doc to stderr with no summary of what was skipped.

**Where.** `src/index/sync.rs`, `src/connectors/cloud/*.rs`, the shared HTTP
retry in `src/connectors/cloud.rs`.

**Design.**
- Checkpoint cursors *after* each successful doc (mostly done); verify a
  killed sync resumes cleanly on the next run for every connector.
- Aggregate a per-source summary (fetched / updated / skipped / errored) at
  the end, not just per-doc lines.
- Consider a `--continue-on-rate-limit` that records the cursor and exits 0
  with a "resume with `mari sync <source>`" note rather than blocking.

**Acceptance.** Killing a sync at an arbitrary point and re-running produces a
complete, correct index with no duplicate or lost docs; the summary names
what was skipped and why.

**Effort.** M.

---

## 4.5 — Large-repo detector performance (P2, S–M)

**Current.** The detector walks with `rayon` (good) but recompiles some
per-rule regexes lazily via `OnceLock` (good) — confirm no rule rebuilds a
regex per file. The false-positive *budget* against big trees (§19) is
unmeasured (`09-testing-ci.md`).

**Where.** `src/detector/runner.rs`, individual rules.

**Design.** Profile `mari detect` over a large real docs tree (hundreds of
files); confirm linear scaling and that the tree-walk skip lists prune
vendored/generated content effectively. Fix any per-file allocation hot spots.

**Acceptance.** `detect` over a 500-file tree completes in a few seconds; a
documented findings-per-file budget holds (see `09`).

**Effort.** S–M.

---

## 4.6 — Resident model sidecar (latency) (P2, M)

**Current.** Every `mari search` loads the embedding GGUF (~1s cold in debug)
to embed the query; every `--deep`/attention call loads the attention GGUF.
Fine for one-shot CLI use; painful in tight loops (the hook, `audit kb`,
`i18n conform` over many files).

**Where.** `src/index/vector.rs::embed_texts` (loads per call),
`src/attn.rs::analyze` (loads per call).

**Design.** SPEC §17 describes a "resident sidecar" — a long-lived local
process holding the loaded models, spoken to over a socket, with only
structured output crossing the boundary. Options:
- A `mari serve` daemon (opt-in) that the CLI connects to when present, falls
  back to in-process load when absent.
- Or, cheaper: batch all query embeddings / all attention docs in a single
  model load per command (already done for sync embeddings and multi-doc
  i18n; extend to the hook and `audit kb`).

**Acceptance.** A loop of 50 `explore --focus` calls (or the hook firing
repeatedly) does not pay 50 model loads; latency is dominated by inference,
not load.

**Effort.** M (batching) / L (daemon).

---

## 4.7 — Cloud replica consistency and one-writer edges (P2, S)

**Current.** One-writer rule is enforced on `sync` (consumer role refuses);
`--rebuild` is blocked on cloud indexes. Remaining sharp edges:
- The git backend copies `catalog.db` into `.mari/catalog` but the Lance
  `vectors.lance` dataset is **not** part of the cloud replica flow — a
  consumer pulling the git catalog gets documents/chunks but no vectors, so
  their search silently degrades to keyword-only.
- S3 backend pushes/pulls only `catalog.db` similarly.

**Where.** `src/cloud.rs` (`init`, `pull`, `push_s3`), `src/index/vector.rs`
(dataset path).

**Design.** Include `vectors.lance` in the cloud artifact set (it's a
directory; tar it for S3, add it under Git LFS for the git backend), or have
consumers re-embed locally on first pull (embeddings are per-machine per
SPEC §8.2's `embedded` table rationale — decide which). Document the choice.

**Acceptance.** A consumer that pulls a shared catalog gets working vector
search (either the shared vectors or a local re-embed), not silent
keyword-only degradation.

**Effort.** S–M.
