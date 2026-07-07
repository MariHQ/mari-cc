# 05 — Distribution & Packaging

The plugin's skills, commands, and hooks all shell out to a `mari` binary on
`PATH`. Nothing currently installs that binary. This doc is the path from
"builds on my machine" to "a teammate can install and use it."

---

## 5.1 — Prebuilt cross-platform binaries (P0, M)

**Current.** The only way to get `mari` is `cargo build`/`cargo install`,
which requires a Rust toolchain **and cmake** (llama.cpp builds from source)
and a multi-minute compile. That is not an acceptable install for
non-Rust-developer teammates (marketing, support, PM — Mari's stated
audience).

**Design.**
- A GitHub Actions release workflow that builds and uploads binaries for at
  least: macOS arm64, macOS x86_64, Linux x86_64 (glibc), Linux arm64.
  Windows is its own effort (`08-portability.md`).
- Statically link what's feasible; llama.cpp + duckdb bundling means the
  binary is ~48 MB — acceptable.
- Consider CPU-feature baselines (AVX2 vs not) for the llama.cpp path, or
  ship a portable build and let llama.cpp runtime-detect.
- Decide GPU story per platform: Metal is automatic on macOS; Linux CUDA is a
  separate build variant (`08`).

**Acceptance.** `curl`-download or a package-manager install drops a working
`mari` on each target platform with no toolchain.

**Effort.** M (CI matrix + release automation).

---

## 5.2 — Install channel (P0, S–M)

**Current.** None.

**Design (pick per owner decision in `02` #3):**
- **Homebrew tap** (`brew install owner/tap/mari`) — best for macOS/Linux dev
  audiences.
- **Install script** (`curl … | sh`) that fetches the right release binary —
  lowest friction, but a security posture to document.
- **Published crate** (`cargo install mari`) — keep as a fallback for Rust
  users; document the cmake prerequisite.
- The plugin's `init` flow (`skills/mari/references/reference-init.md`) should
  detect a missing `mari` and point at the chosen channel with the exact
  command.

**Acceptance.** A new user runs one documented command and has `mari` on PATH;
the plugin's setup recognizes it.

**Effort.** S–M depending on channel.

---

## 5.3 — Plugin publication (P1, S)

**Current.** The plugin layout exists (`.claude-plugin/plugin.json`,
`skills/`, `commands/`, `hooks/hooks.json`) but is not published to any Claude
Code plugin registry/marketplace, and `plugin.json` has placeholder
`homepage`/`author` fields.

**Design.**
- Fill in real `author`, `homepage`, `repository`, and a stable `version`
  that tracks the binary's.
- Follow the Claude Code plugin publication process (marketplace manifest or
  distribution mechanism).
- Ensure the plugin declares or documents its `mari` binary dependency and
  version compatibility.

**Acceptance.** The plugin installs from the intended channel and its
commands/skills/hooks activate; version compatibility between plugin and
binary is documented.

**Effort.** S.

---

## 5.4 — Model provisioning UX (P0 for correctness, M for polish)

**Current.** First `mari sync` downloads the ~610 MB embedding GGUF inline
(and attention adds ~508 MB on first `--deep`). This is: (a) surprising, (b)
a network dependency on Hugging Face at an unpinned revision (`07` #1), (c)
unusable offline/air-gapped, (d) only resumable at whole-file granularity.

**Design.**
- A dedicated `mari model pull [embedding|attention|all]` command so the
  download is an explicit, resumable, progress-reported step — not a surprise
  inside `sync`.
- `sync`/`--deep` still auto-download when missing (behind `*.auto_download`,
  default true) but print a clear one-time notice.
- Support `*.model` config overrides pointing at a local path (air-gapped
  installs place the GGUF and set the path).
- Resumable download (HTTP range / `.part` + verify) so a dropped 610 MB
  transfer doesn't restart from zero.
- Optionally mirror the GGUFs to owner-controlled storage (`02` #4) for
  reproducibility and to avoid HF rate limits.

**Acceptance.** `mari model pull all` fetches, verifies, and reports both
models; an offline install with pre-placed GGUFs + config paths works with
`auto_download=false`; an interrupted download resumes.

**Effort.** M.

---

## 5.5 — Version, changelog, release process (P1, S)

**Current.** `Cargo.toml` version is `0.1.0`; no `CHANGELOG.md`; no release
tags beyond the ad-hoc "WIP" commits.

**Design.**
- Adopt semver; keep the plugin version and binary version in lockstep (or
  document the compatibility matrix).
- `CHANGELOG.md` (Keep-a-Changelog style); `mari check` recommends it anyway.
- A tagged release process: tag → CI builds binaries → attaches to the
  GitHub Release → updates the install channel.

**Acceptance.** `mari --version` matches the release tag; the changelog and
release notes exist; the process is a documented runbook.

**Effort.** S.

---

## 5.6 — Binary size and cold-build time (P2, S)

**Current.** ~48 MB binary; ~3 min cold build (llama.cpp + duckdb + lance +
datafusion + harper). Acceptable but worth watching as ML tier 1 adds ONNX.

**Design.** Feature-gate the heavy optional runtimes (`--features ml` for
ONNX, keep attention/embedding in the default) so users who only want the
detector aren't forced to compile/download everything. Measure and document
the size/build-time cost of each feature.

**Acceptance.** A `--no-default-features` or detector-only build is
meaningfully smaller and faster; the default build stays under a documented
size ceiling.

**Effort.** S.
