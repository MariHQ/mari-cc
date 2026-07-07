# Mari

Mari is a local-first Claude Code plugin for curating, searching, and sharing
your team's product knowledge — and for keeping everything Claude writes clean.
It answers "what should our AI know, trust, and reuse?"

Everything runs on your machine. Indexing, embeddings, search, and the prose
detector are all local; there are no external LLM calls from the CLI and your
credentials never enter the repo.

## What it does

- **Ingest and search.** Local hybrid (semantic + keyword) search over the
  knowledge your team already uses: Slack, GitHub, Google Drive, Jira,
  Confluence, Zendesk, Salesforce, HubSpot, Microsoft 365, Discord, Linear,
  git history, and local files (Markdown, HTML, PDF, and Office documents).
- **Curate.** Tag knowledge as canonical, stale, deprecated, draft, internal,
  customer-facing, or needs-review; keep a glossary and a facts ledger; audit
  the knowledge base.
- **Improve AI-authored prose.** A deterministic ~230-rule detector for AI
  slop, clarity, house style (Microsoft/Google/AP/Chicago/plain), and
  inclusive language, plus editorial verbs (`deslop`, `tighten`, `clarify`,
  `sharpen`, `understate`, `critique`, `polish`, …) and an opt-in grammar pass.
- **Ground claims.** Factcheck content against a facts ledger, source-of-truth
  files, or the knowledge base — catching contradictions and unsupported
  claims before publish, with an optional local attention model for deep
  grounding.
- **Keep it alive.** A post-edit hook, doc↔code lineage, edit-notify rules and
  nudges, localization staleness checks, and docsite generation/validation.

## Install

Mari is a Rust binary. Prebuilt binaries and an install channel are being set
up (see `docs/05-distribution.md`); until then, build from source:

```sh
# Requires a Rust toolchain and cmake (llama.cpp builds from source).
cargo install --path .
# or
cargo build --release   # binary at target/release/mari
```

As a Claude Code plugin, the `skills/`, `commands/`, and `hooks/` directories
plus `.claude-plugin/plugin.json` wrap the `mari` binary; ensure `mari` is on
your `PATH`.

## Quickstart

```sh
mari init                 # assistant-guided setup (sources + editorial style)
mari track localfiles add ./docs
mari sync                 # index tracked sources (first run downloads the
                          #   embedding model, ~640 MB, one-time)
mari search "why did we change pricing tiers"
mari detect README.md     # prose-quality findings
mari factcheck pricing.md --source PRODUCT.md
```

From Claude Code, the standalone commands work directly:
`/search`, `/sync`, `/tag`, `/factcheck`, `/audit`, `/deslop`, `/tighten`,
`/clarify`, `/sharpen`, `/understate`, `/critique`, `/polish`, `/draft`.

## Models

Mari uses two small local models, downloaded on first use into `~/.mari/models`
(verify status with `mari model status`, provision explicitly with
`mari model pull all`):

- **Embeddings** — `Qwen3-Embedding-0.6B` (Apache-2.0), ~640 MB.
- **Attention** (deep grounding/coverage/focus, opt-in via `--deep`) —
  `Qwen3.5-0.8B` (Apache-2.0), ~520 MB.

An optional OCR tier for scanned PDFs uses `baidu/Unlimited-OCR`; it is off by
default (the default PDF path is pure-Rust text extraction) and requires an
explicit opt-in because it executes code from the model repo — see
`SECURITY.md`.

## Documentation

- `SPEC.md` — the complete behavioral specification (every command, rule, and
  config key) and §22 implementation decisions.
- `docs/` — the roadmap and remaining-work plan.
- `skills/mari/references/` — the editorial and workflow reference flows.

Run `mari doctor` to see which optional tools and models are available, and
`mari features` for the full capability catalog.

## License

MIT — see `LICENSE`. Bundled models carry their own permissive licenses
(Qwen: Apache-2.0; Unlimited-OCR: MIT).
