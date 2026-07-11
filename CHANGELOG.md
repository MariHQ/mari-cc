# Changelog

All notable changes to Mari are recorded here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Local hybrid search (keyword + `Qwen3-Embedding-0.6B` vectors) over 13
  sources; Lance vector storage with DuckDB Arrow-bridge ranking and weighted
  RRF fusion.
- Deterministic ~230-rule prose detector across families A–D and five style
  packs; opt-in Harper grammar pass; slop score.
- Editorial verbs, factcheck (deterministic + attention grounding), curation
  (tags, glossary, facts, extract, audit kb), lineage curation.
- Tier-2 local attention engine (`Qwen3.5-0.8B`) powering `i18n coverage`,
  `i18n conform --deep`, `factcheck --deep`, `check --deep`, and
  `explore --focus`.
- Office extraction (docx/odt/rtf/pptx/xlsx) and PDF extraction (pure-Rust
  default; optional `baidu/Unlimited-OCR` model tiers, gated behind an explicit
  remote-code acknowledgement).
- Connectors: localfiles, git (+ commit history), Slack, Google Drive, GitHub,
  Confluence, Jira, Zendesk, Salesforce, HubSpot, Microsoft 365, Discord,
  Linear, Granola (local meeting-notes cache); shared §6.0 HTTP contract.
- Post-edit hook (8 jobs) and post-commit association hook; edit-notify rules
  and nudges; cloud sharing (git + S3) with vector replication.
- `mari model pull` / `mari model status`, `mari doctor`, checksum-verified
  resumable model downloads, workspace sync lock, schema-migration scaffold,
  and an embedding-identity guard on vector search.
- Claude Code plugin packaging: `.claude-plugin/plugin.json`, skills,
  standalone commands, and the post-edit hook.
- `mari console`: a local, single-user web console served from the binary
  (embedded Vite/React bundle, no Node at runtime) over a synchronous
  `tiny_http` server. Read/write JSON API over the same catalog and config the
  CLI uses — observe and curate documents, connectors, tags, lineage (a
  React Flow graph), glossary, facts, config, cloud (S3/git sharing), and
  status; edit connector tracked-refs, apply/remove tags, confirm/reject
  lineage, edit config, manage cloud role and push/pull, and trigger syncs
  from the browser. Switch between any workspace already indexed on the machine
  (a `~/.mari/projects.json` registry maps them to paths); manage nudges and
  edit-notify rules; browse and govern the full detector rule catalog
  (zero-tolerance / ignore); scaffold document templates; generate a STYLE.md;
  and manage the tag status vocabulary. The lineage view is a dagre-laid-out,
  searchable/filterable graph; the overview has recharts summaries.

### Security
- OCR model tiers require `ocr.accept_remote_code=true` (they run
  `trust_remote_code=True`); the default PDF path is pure Rust.
- Model downloads are checksum-ready (SHA-256 verification once revisions are
  pinned) and resumable; `deny.toml` enforces the license/advisory policy.

### Notes
- Deferred to a later release (SPEC §22): ML tier 1 (NLI, machine-likelihood,
  slop-spans) and the cross-encoder reranker; Windows support; ANN indexing at
  very large scale. See `docs/` for the plan.
