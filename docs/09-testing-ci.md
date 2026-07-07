# 09 — Testing & CI

The detector and mapping logic are well-covered by unit tests (~285 `#[test]`
functions). The gaps are the model-inference paths, the large-repo
false-positive budget, and CI breadth.

---

## 9.1 — The §19 quality bars, status by bar (P1)

SPEC §19 lists five behavioral quality bars. Current status:

| §19 bar | Status | Gap |
|---|---|---|
| Per-rule bad→good fixture pairs (~180 assertions) | **Mostly met** — each rule module ships bad/good tests | Audit that *every* registered rule has both; add a meta-test that enumerates the registry and asserts coverage |
| Integration/regression suite (~35 checks: masking, skip-lists, localized files, table rules) | **Partially met** | Confirm masking (front matter, comments, shortcodes), CJK/generated/vendored skipping, and table-aware number rules each have explicit tests |
| Model tests run real local inference (no stubs) | **Not met in CI** — verified manually this session | Needs a model-cached CI job (9.3) |
| Large-repo false-positive budget vs big real trees | **Not met** — CI does an advisory sweep with no ceiling | Needs a real corpus + a hard budget (9.2) |
| Deliberate-slop self-test (`fixtures/sloppy.md`) | **Met** — CI runs it | Keep |

**Effort.** M to close the audit + missing integration tests.

---

## 9.2 — Large-repo false-positive budget (P1, M)

**Current.** `.github/workflows/ci.yml` runs `mari detect skills/mari/references
--summary || true` — advisory only, no ceiling, small tree.

**Design.**
- Check in (or fetch in CI) a representative large real documentation tree
  (hundreds of files — e.g. a snapshot of a well-known OSS docs site that is
  redistributable).
- Establish a findings-per-1k-words budget per family and fail CI if the
  detector exceeds it (catches a rule that starts over-firing after a change).
- This is the §19 "false-positive budget validated against big real
  documentation trees" bar.

**Acceptance.** CI fails if the detector's false-positive rate on the corpus
regresses past the budget.

**Effort.** M (corpus selection + budget calibration).

---

## 9.3 — Real-inference CI job (P1, M)

**Current.** CI never runs the embedding/attention/grammar-inference paths;
those were verified only by hand this session.

**Design.**
- A CI job that caches the GGUFs (keyed on the pinned revision from `07` #1)
  and runs: an embedding sync + semantic-search assertion; an attention
  coverage/grounding assertion (`01-known-issues.md` #7's fixture); the
  Harper grammar tests (these already run in `cargo test` and are cheap).
- Gate behind cache availability so PRs from forks (no cache) still get the
  deterministic suite.

**Acceptance.** Every push runs the deterministic suite; pushes with the model
cache also run the inference assertions; a regression in embedding/attention
output fails CI.

**Effort.** M.

---

## 9.4 — CI hardening: clippy, fmt, deny, audit (P1, S)

**Current.** CI runs build + test + the slop self-test. Missing:
- `cargo clippy -- -D warnings` (the warning cleanup this session must not
  regress — `01-known-issues.md` #5).
- `cargo fmt --check`.
- `cargo deny check` (licenses + advisories + bans — `07` #4, #7).
- `cargo audit`.

**Design.** Add these as CI steps; fix any clippy findings the current code
has (the build is warning-clean but clippy is stricter).

**Acceptance.** CI enforces zero warnings, formatting, license policy, and
advisory-free dependencies.

**Effort.** S (plus whatever clippy surfaces).

---

## 9.5 — CI platform matrix (P1, M)

**Current.** CI is Ubuntu-only (`.github/workflows/ci.yml`), single job.

**Design.** Matrix over macOS (arm64 + x86_64) and Linux (x86_64), with the
release build (`05` #1) sharing the matrix. Windows once `08` #1 lands. Cache
the cargo registry + target dir (already via `Swatinem/rust-cache`) and the
cmake build of llama.cpp (slowest step).

**Acceptance.** Every push builds and tests on all supported platforms.

**Effort.** M.

---

## 9.6 — End-to-end / integration test coverage (P2, M)

**Current.** Unit tests dominate; there are integration-style tests
(temp-repo sync/search round-trips) but not a full plugin-level e2e.

**Design.**
- A temp-repo harness that exercises: init → track → sync (localfiles+git) →
  embed → search → detect → factcheck → tag → hook-fires → check — as one
  scripted flow, asserting outputs at each step.
- A hook e2e: feed a PostToolUse JSON, assert the eight §15.1 jobs' outputs.
- The `mari sql` read-only guard, `mari config` coercion, scope resolution,
  and cloud init/pull round-trips as integration tests.

**Acceptance.** A single `cargo test --test e2e` (or a CI script) walks the
primary user journey and asserts each stage.

**Effort.** M.

---

## 9.7 — Fuzz / robustness tests (P2, M)

**Current.** None. The extractors and parsers handle untrusted input.

**Design.** `cargo fuzz` (or property tests) for: the detector masking/
segmentation (malformed markdown), Office extraction (malformed zip/xml),
PDF extraction (malformed PDF), RTF parser, and the factcheck typed-span
extractors. See `07-security.md` #5.

**Acceptance.** Fuzz targets run in CI (time-boxed) without crashes/hangs.

**Effort.** M.
