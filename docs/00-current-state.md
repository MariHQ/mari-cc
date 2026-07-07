# 00 — Current State Snapshot

A factual picture of what exists in the repository as of this plan, so the
rest of the docs have a shared baseline. Where the working tree and the last
commit disagree, that is called out (and detailed in `01-known-issues.md`).

## What Mari is

A single Rust crate (`mari`) plus a Claude Code plugin wrapper. The crate is
a CLI that implements `SPEC.md`: a local-first knowledge index (hybrid
keyword + vector search over 13 sources), a deterministic ~230-rule prose
detector with an opt-in Harper grammar pass, editorial/factcheck/curation
commands, doc-system tooling, localization checks, a post-edit hook, and a
Tier-2 local attention engine for deep coverage/grounding/focus.

## Metrics

- **Source:** ~59 `.rs` files, ~46k lines including tests.
- **Tests:** ~285 `#[test]` functions; last green run this session reported
  244 passing (the delta is test modules added after the last full run and
  gated `#[cfg(test)]` helpers).
- **Detector rules:** ~230 registered across families A–D and five style packs.
- **Connectors:** 13 (`localfiles`, `git` + commit history, and 11 cloud:
  slack, gdocs, github, confluence, jira, zendesk, salesforce, hubspot,
  microsoft, discord, linear).
- **Models:** two local GGUFs under `~/.mari/models` —
  `Qwen3-Embedding-0.6B-Q8_0.gguf` (~610 MB, embeddings) and
  `Qwen3.5-0.8B-Q4_K_M.gguf` (~508 MB, attention). Both Apache-2.0.
- **Release binary:** ~48 MB, builds clean (llama.cpp via cmake, ~3 min from
  cold). Verified on macOS/arm64 (Metal). Linux/CUDA unverified.

## What works today (verified live this session)

- **Detector** — `mari detect` over the deliberate-slop fixture flags the
  expected rule set; clean prose stays clean; `--score`, `--json`,
  `--summary`, `--style`, per-run pack override all function.
- **Harper grammar** — `--grammar` flags e.g. "for all intensive purposes"
  as `grammar-nonstandard` with suggestions; dropped kinds stay silent.
- **Office extraction** — a synthesized `.docx` synced, embedded, and became
  the top search hit for its content; docx/odt/rtf/pptx/xlsx fixtures pass.
- **Attention (Tier 2)** — `i18n coverage` flagged exactly the section a
  translation dropped; `explore --focus` pinpointed the `$49/seat` lines;
  `factcheck --deep` flagged a fabricated sentence; `check --deep` flagged
  unanchored marketing prose in PRODUCT.md.
- **Factcheck (deterministic)** — number/date-mismatch and contradiction
  detection against FACTS.md / `--source`.
- **Hooks** — post-edit hook reads Claude Code PostToolUse JSON, runs all
  eight §15.1 jobs, always exits 0; post-commit association hook links a
  commit to related knowledge.
- **Cloud sharing** — git and S3 backends; consumer-role sync guard.
- **Lineage** — `mari lineage add/list/confirm/reject`; hook `⛓` notices.
- **Curation** — tags, glossary, facts, extract, `audit kb`.
- **Plugin** — `.claude-plugin/plugin.json`, `skills/` (mari + 11 connect-*),
  12 standalone `commands/`, `hooks/hooks.json`.

## What is stubbed or deferred (by design, documented in SPEC §22)

- **ML tier 1** — machine-likelihood, NLI entailment/contradiction, zero-shot
  slop-span extraction. `--models` / `--slop-spans` print a loud
  "not available in this build" note and degrade to deterministic tier. See
  `03-ml-tier1.md`.
- **Legacy Office** — `.doc` / `.ppt` binary formats unsupported (§20).

## What is currently BROKEN (see `01-known-issues.md`)

- **Embeddings write 0 vectors.** The committed WIP reverted
  `vector.rs::ensure_model()` to an always-error stub and `EMBEDDING_MODEL`
  back to `jina-embeddings-v5-text-nano`, while `embed_texts()` still calls
  `ensure_model()?`. Net effect: `mari sync` prints
  `✗ vector embedding failed … not available in this build` and writes zero
  vectors, so search is keyword-only. This is the #1 fix.
- **Working tree is dirty and inconsistent** with the last commit; the
  session's Qwen swap, warning cleanup, and other edits are partially present.

## Repository layout (orientation)

```
src/
  main.rs                 CLI surface (clap) + dispatch
  config.rs workspace.rs  config resolution, workspace identity, scopes
  detector/               engine (ctx, mask, segment), rules_a..d, packs, grammar, score
  index/                  catalog (duckdb), chunking, hybrid search, sync, vector (lance+llama)
  connectors/             registry, gitlog, cloud/<source>.rs (11)
  attn.rs                 Tier-2 attention engine (llama-cpp-sys FFI)
  ocr.rs office.rs        PDF (pdf-extract default; Unlimited-OCR opt-in), Office (zip+quick-xml)
  factcheck.rs curation.rs lineage.rs hook.rs rulescmd.rs ...
skills/ commands/ hooks/ .claude-plugin/   plugin packaging
.github/workflows/ci.yml  CI (present, not yet pushed)
SPEC.md                   behavioral spec + §22 implementation decisions
fixtures/                 sloppy.md, sample.pdf, ...
docs/                     this plan
```
