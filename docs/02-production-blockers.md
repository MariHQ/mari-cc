# 02 — Production Blockers

Items that must be cleared before Mari is handed to anyone outside the
authors. Each is a gate, not a nice-to-have. Cross-references point at the
area docs where the item is detailed.

## The go/no-go checklist

- [ ] **Embeddings actually run** — `01-known-issues.md` #1. Without this the
  product's headline feature (semantic search) is inert.
- [ ] **Clean, green `main`** — `01-known-issues.md` #2. Build + test + clippy
  green, zero warnings, legible history, tagged baseline.
- [ ] **LICENSE chosen and present** — the crate declares `license = "MIT"`
  in `Cargo.toml` but there is no `LICENSE` file. Decide and add it
  (`10-community-and-user-docs.md`). Verify every bundled model/dependency
  license is compatible with that choice (`07-security.md` #4).
- [ ] **README + CONTRIBUTING + SECURITY** — `10`. Mari fails its own
  `mari check` today.
- [ ] **Binary distribution** — prebuilt per-platform binaries and an install
  path the plugin's setup can point at; `cargo install` alone requires a Rust
  toolchain **and cmake** and a multi-minute build (`05-distribution.md`).
- [ ] **Real humanizer URL** — `01-known-issues.md` #4 / `10`.
- [ ] **Model-download integrity** — pinned HF revisions + SHA-256
  verification for both GGUFs; today they pull unpinned `main` with no
  checksum (`07-security.md` #1).
- [ ] **`trust_remote_code` disclosure** — the optional Unlimited-OCR runner
  loads a HF model with `trust_remote_code=True` (arbitrary code execution by
  that model's design). It is opt-in, but the risk must be documented
  prominently and gated behind an explicit acknowledgement
  (`07-security.md` #2).
- [ ] **Credential handling audit** — confirm 0600/0700 perms hold on all
  write paths, tokens never land in committed config or logs, and the
  `.mari/config.json` team-shared file never carries secrets
  (`07-security.md` #3).
- [ ] **Connector live shakedown** — at least one supervised real-account sync
  per cloud connector, captured into replayable fixtures (`06-connectors.md`).
- [ ] **Concurrency safety** — a workspace lock so two `mari sync` runs can't
  corrupt the catalog; graceful behavior on a locked DB (`04-scale-and-robustness.md` #2).
- [ ] **Schema migration path** — a version-gated migration so an older
  catalog upgrades instead of erroring (`04-scale-and-robustness.md` #3).
- [ ] **CI green on the target platforms** — at minimum a Linux job (build +
  test + clippy + slop self-test) and, ideally, a macOS job and a
  real-inference job (`09-testing-ci.md`).
- [ ] **First-run UX** — the first `mari sync` triggers a ~610 MB model
  download (and, if attention is used, another ~508 MB). This must be
  surfaced clearly, be resumable, and be skippable/redirectable for offline
  or air-gapped installs (`05-distribution.md` #4, `07-security.md` #1).

## Decisions the owner must make (not code)

These block planning of the items above and only the project owner can
answer them:

1. **License.** MIT (as declared) or something else? Affects the `LICENSE`
   file and the compatibility audit.
2. **Humanizer upstream.** What is the real repo, or should `humanize` ship
   at all?
3. **Distribution channel.** GitHub Releases + Homebrew tap? A curl-install
   script? A published crate? This shapes `05`.
4. **Model hosting.** Rely on Hugging Face directly (network dependency,
   rate limits, revision drift) or mirror the GGUFs to owner-controlled
   storage (S3/CDN) for reproducibility and offline installs? Affects
   `05` and `07`.
5. **Telemetry/opt-in.** Any usage reporting, or strictly local-only
   (consistent with the "local-first, no external LLM calls" invariant)?
6. **Support surface.** Which connectors are "supported" in v1 vs
   "experimental"? This scopes `06`'s live-testing effort.
