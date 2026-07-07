# Completion Status

This tracks what the "complete everything in docs/" work delivered against the
plan in this folder. Updated as of the completion pass.

## Done (implemented, tested, committed on branch `complete-docs-plan`)

### P0 — blockers
- **Embedding regression fixed** (`01` #1) — coherent `Qwen3-Embedding-0.6B`
  (1024-dim), working checksum-verified resumable provisioning via the new
  `src/models.rs`; live-verified (10 vectors, semantic query returns results).
  An identity-consistency test guards against the drift recurring.
- **Clean green baseline** (`01` #2) — zero warnings, zero clippy findings,
  308 unit + 3 integration tests, `cargo fmt` clean; committed in logical units
  on the `complete-docs-plan` branch.
- **`mari check` passes on the repo** (`01` #3) — community files present;
  asset-check gated to canonical filenames so planning docs aren't mis-detected.
- **Humanizer URL** (`01` #4) — now config-driven (`humanizer.repo`), no
  placeholder clone; errors cleanly asking for a URL.
- **Stale ML/extraction notes** (`01` #6) — reworded/removed.
- **Community files** (`10`) — `LICENSE` (MIT), `README.md`, `CONTRIBUTING.md`,
  `CODE_OF_CONDUCT.md` (Contributor Covenant v2.1), `SECURITY.md` (with the
  remote-code disclosure), `CHANGELOG.md`.

### P0 — robustness & security
- **Workspace sync lock** (`04` #2) — advisory PID file; stale-lock reclaim;
  concurrent sync exits cleanly. Tested.
- **Schema migration scaffold + embedding-identity guard** (`04` #3) — vector
  search refuses on model/dim mismatch and points at `--rebuild`; stamp-on-create.
- **Cloud vector replication** (`04` #7) — `vectors.lance` rides Git LFS /
  `aws s3 sync` alongside the catalog.
- **Model-download integrity** (`07` #1) — pinned-revision + SHA-256
  verification wired (enforced once the hash is recorded), resumable downloads.
- **`trust_remote_code` gate** (`07` #2) — OCR model tiers require
  `ocr.accept_remote_code=true`; default `text` backend never triggers it.
- **`cargo-deny`** (`07` #4/#7) — `deny.toml` license allowlist + advisories; CI job.
- **Input-safety caps** (`07` #5) — extraction output-size cap.

### P1 — distribution, CI, connectors, portability
- **`mari model pull`/`status` + `mari doctor`** (`05` #4, `08` #4).
- **CI** (`09`) — fmt + clippy(`-D warnings`) + test matrix (macOS+Linux),
  cargo-deny job, model-cached real-inference job, self-dogfood `mari check`.
- **Release workflow** (`05` #1) — prebuilt binaries (macOS arm64/x86_64,
  Linux x86_64/arm64) + SHA-256 sidecars on a `v*` tag.
- **e2e + hook + false-positive-budget tests** (`09` #6, #2) — `tests/e2e.rs`.
- **Connector fixture harness** (`06` #2) — `MARI_GITHUB_API` seam + a
  replay-server unit test; the documented pattern for the other connectors.
- **Configurable GPU offload + Windows cfg branches** (`08` #1/#3).
- **Version/changelog/release process** (`05` #5), plugin manifest fields (`05` #3).

### P2 — ML tier 1 (partial) & scale (partial)
- **Machine-likelihood blend** (`03` 3.2) — `detect --score --models` computes
  perplexity via the local model and blends it into the slop score;
  live-verified. Tested.
- **Lineage proposal generation** (`11.2`) — `mari lineage refine` proposes
  span↔span edges via the attention Focus mode.
- **Brute-force scale cliff** made visible (`04` #1) — a log fires past 100k
  vectors so the operator knows an ANN index would help; results stay correct.

## Deferred (documented, with the decision recorded in SPEC §22)

These are the plan's explicitly P2-fast-follow / P3 items; the code degrades
loudly and the runtime/approach is decided so they are pick-up-ready:

- **NLI cross-encoder + zero-shot slop-spans** (`03` 3.1/3.3) — need the `ort`
  runtime + curated models behind `--features ml`; the flags note this and
  fall back to the deterministic (and attention, for `factcheck --deep`) tiers.
- **Cross-encoder reranker** (`11.5`) — same `ort` runtime; `search.rerank.*`
  config already degrades gracefully.
- **Lance ANN index + inverted keyword index** (`04` #1) — for scale past
  ~100k chunks; brute-force is exact and fast below that and now logs the cliff.
- **Resident model daemon** (`04` #6) — embeddings already batch per sync;
  the always-on sidecar is the latency optimization for tight loops.
- **Full Windows CI + credential ACLs** (`08` #1) — cfg branches compile on
  Windows; ACL hardening + a Windows runner remain.

## Requires owner input (cannot be completed in-repo)

- **Live connector shakedown** (`06` #1/#3/#4) — needs real accounts for each
  cloud provider; mapping is unit- and replay-tested, but live auth/pagination/
  rate-limit behavior must be validated with credentials.
- **Model-download revision pins + hashes** (`07` #1) — the verification code
  is in place; the specific commit SHA and SHA-256 per model must be recorded
  at release time (and optionally mirrored to owner storage).
- **Humanizer upstream URL** (`10.4`) — supply the real repo or drop the command.
- **Distribution channel + plugin publication** (`05` #2/#3) — Homebrew tap /
  install script / marketplace listing are external publishing steps.
- **License / telemetry / support-surface decisions** (`02`).
