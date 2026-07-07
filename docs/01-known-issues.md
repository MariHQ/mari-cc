# 01 — Known Issues (fix these first)

Concrete defects in the current working tree, ordered by severity. These are
not "future features" — they are things that are wrong right now.

---

## #1 — Embeddings write zero vectors (P0, S–M)

**Symptom.** `mari sync` prints
`✗ vector embedding failed: embedding runtime for jina-embeddings-v5-text-nano
is not available in this build; keyword-only search continues without writing
embeddings` and the summary reads `… (0 vector(s))`. Vector search silently
degrades to keyword-only for every query.

**Root cause.** An intervening WIP commit reverted two pieces of this
session's embedding work while leaving the rest intact, producing an
internally inconsistent `src/index/vector.rs`:

- `ensure_model()` is now an always-`Err` stub — it neither checks for the
  on-disk GGUF nor downloads it, and returns
  `"…not available in this build…"` unconditionally.
- `EMBEDDING_MODEL` (`src/index/mod.rs`), `DIMS`, and `MODEL_FILE`
  (`src/index/vector.rs`) reverted to the jina values (`768`,
  `jina-embeddings-v5-text-nano`).
- BUT `embed_texts()` still contains the full working llama.cpp path and
  calls `ensure_model()?`, so it can never run.

**Why it matters.** This is the core value proposition (semantic search).
The Qwen3-Embedding-0.6B model (610 MB) is already downloaded to
`~/.mari/models/`; only the resolver is broken.

**Fix.** Restore the coherent Qwen3-Embedding-0.6B state that was verified
live earlier this session:

- `ensure_model()`: check `model_path()` on disk → return it; else, if
  `embedding.auto_download` (default true) → download the Qwen GGUF with a
  `.part` temp + rename; else loud error. (This exact function existed and
  worked; it was the download-on-first-use path.)
- `EMBEDDING_MODEL = "qwen3-embedding-0.6b"`, `DIMS = 1024`,
  `MODEL_FILE = "Qwen3-Embedding-0.6B-Q8_0.gguf"`,
  `MODEL_URL = "https://huggingface.co/Qwen/Qwen3-Embedding-0.6B-GGUF/resolve/main/Qwen3-Embedding-0.6B-Q8_0.gguf"`.
- Query prefix = Qwen retrieval instruct; documents embed raw.
- Add a checksum/pin per `07-security.md` #1 while you're in here.

**Acceptance.** `mari sync --rebuild` on a temp repo reports
`… (N vector(s))` with N = chunk count; a no-keyword-overlap query
(e.g. "how expensive is the premium tier") ranks the pricing docs. Add a
regression test that asserts `EMBEDDING_MODEL`, `DIMS`, `MODEL_FILE`, and the
`MODEL_URL` host all agree (a cheap guard against exactly this drift).

**Note.** A prior system reminder marked the `vector.rs` change "intentional."
Confirm with the committer whether the stub was a deliberate direction (e.g.
"don't auto-download in this build") before re-applying. If auto-download is
genuinely unwanted, the fix is instead: make `ensure_model()` check the disk
path and return a clear "run `mari model pull`" error when absent — but do NOT
leave `embed_texts` calling an always-`Err` resolver.

---

## #2 — Uncommitted, inconsistent working tree (P0, S)

**Symptom.** `git status` shows 7 modified files
(`curation.rs`, `factcheck.rs`, `index/mod.rs`, `index/search.rs`, `main.rs`,
`platform.rs`, `rulescmd.rs`) on top of two "WIP" commits that are not this
session's work. The session's large body of changes (connectors, OCR, Office,
attention, Harper, deep passes, Qwen swap, warning cleanup) is only partly
reflected in commits.

**Why it matters.** There is no clean, buildable, test-green `main` to branch
from. Any new work risks compounding the drift.

**Fix.**
1. Reconcile #1 so the tree builds and embeds.
2. `cargo build && cargo test && cargo clippy` all green, zero warnings.
3. Commit in logical units (suggested: `embeddings+vector`, `attention+deep`,
   `office+ocr`, `harper-grammar`, `connectors`, `plugin+docs`) with real
   messages, not "WIP".
4. Tag a baseline (`v0.1.0-dev` or similar).

**Acceptance.** `git status` clean; `main` builds, tests, and lints green;
history is legible.

---

## #3 — `mari check` fails on Mari's own repo (P0, S)

**Symptom.** `mari check` reports `community-missing-file` errors for
`README.md`, `LICENSE`, `CONTRIBUTING.md`. Mari does not pass its own gate.

**Fix.** See `10-community-and-user-docs.md`. This is both a dogfooding fix
and a release prerequisite.

**Acceptance.** `mari check` on the repo root returns `check: ok` (or only
advisories for the recommended-but-optional files).

---

## #4 — Humanizer clone URL is a placeholder guess (P1, S)

**Symptom.** `src/curation.rs` `HUMANIZER_REPO =
"https://github.com/blader/humanizer"` — invented during implementation.
`mari humanize ensure` will clone the wrong (or a non-existent) repo.

**Fix.** Replace with the real vendored-humanizer upstream (owner must
supply), or remove the `humanize` command from the shipped surface if there
is no upstream. Add a smoke test that the URL resolves (HTTP 200 / valid git
remote) gated behind a network feature so CI doesn't flake.

**Acceptance.** `mari humanize ensure` clones the intended skill; `status`
and `update` operate on it.

---

## #5 — 16 compiler warnings were fixed but not committed (P1, S)

**Symptom.** The warning cleanup (dead-code annotations on spec-contract
fields, deletion of superseded `vector::rank`/`rank_inner`, `#[cfg(test)]` on
`test_settings`) is in the working tree, not committed, and may collide with
#1's reconciliation of `vector.rs`.

**Fix.** Fold into #2's clean commits; re-verify `cargo build` emits zero
warnings and add `-D warnings` to CI (`09-testing-ci.md`).

**Acceptance.** `cargo build 2>&1 | grep -c warning` → 0; CI enforces it.

---

## #6 — Stale "not available in this build" notes after Office wiring (P2, S)

**Symptom.** `gdocs.rs` and `microsoft.rs` still contain
`"… extraction not available in this build"` branches for the non-PDF /
non-Office fallthrough. These are now only reached for genuinely unsupported
types, but the message is misleading (Office *is* supported).

**Fix.** Reword to name the actually-skipped type
(e.g. "unsupported binary format `<ext>` — skipping"), so logs don't imply a
capability gap that no longer exists.

**Acceptance.** Grep for "not available in this build" returns only the ML
tier-1 sites (`factcheck.rs`, `detector/runner.rs`) and the deliberate
`vector.rs` fallback message (which #1 removes).

---

## #7 — `factcheck --deep` threshold semantics need a fixture (P2, S)

**Symptom.** The attention grounding threshold was recalibrated this session
(spec's 0.10 → config `attention.threshold` default 0.3) because this port
preserves absolute attention mass rather than the prototype's row-normalized
scores. That reasoning is documented in SPEC §22 but has no regression
fixture pinning the calibration.

**Fix.** Add a small fixture pair (a grounded source + a claims file with one
fabricated sentence) and assert the fabricated sentence flags while the
grounded one does not, at the default threshold. This is a real-inference
test — gate it behind the model-present CI job (`09-testing-ci.md`).

**Acceptance.** A `#[test]` (or CI script) that runs the pair and checks the
`ungrounded-span` finding set.
