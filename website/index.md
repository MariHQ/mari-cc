# Mari Docs

Mari is a local-first Claude Code plugin for curating, searching, and sharing your team's product knowledge, and for keeping everything Claude writes clean. It answers one question: what should our AI know, trust, and reuse?

Everything runs on your machine. Indexing, embeddings, search, and the prose detector are all local. The CLI makes no external large language model (LLM) calls, and your credentials never enter the repo.

## What Mari does

Mari has five pillars:

- **Ingest and search.** Local hybrid search over the knowledge your team already uses. Sources include Slack, GitHub, Google Drive, Jira, Confluence, Zendesk, Salesforce, HubSpot, Microsoft 365, Discord, Linear, Granola, mailing-list archives, git history, and local files.
- **Curate.** Tag knowledge as canonical, stale, deprecated, draft, internal, customer-facing, or needs-review. Keep a glossary and a facts ledger. Audit the knowledge base.
- **Improve AI-authored prose.** A deterministic detector for AI slop, clarity, house style, and inclusive language, plus editorial verbs like `deslop`, `tighten`, `clarify`, and `polish`.
- **Ground claims.** Factcheck content against a facts ledger, source-of-truth files, or the knowledge base, catching contradictions and unsupported claims before they publish.
- **Keep it alive.** A post-edit hook, doc-to-code lineage, edit-notify rules, localization staleness checks, and docsite generation.

## Where to start

- New here? [Install Mari](getting-started/install.md), then run the [Quickstart](getting-started/quickstart.md).
- Solving a task? The [How-to guides](guides/connect-sources.md) cover connecting sources, searching, curation, prose, factchecking, and docs.
- Looking something up? The [CLI reference](reference/cli.md) lists every command and flag.
- Want the why? [Architecture](explanation/architecture.md) explains how the local-first pipeline fits together.
