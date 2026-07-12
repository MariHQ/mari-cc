# Architecture

Mari is local-first. Indexing, embedding, search, and the prose detector all run on your machine, and the CLI makes no external large language model (LLM) calls. This page explains how the pieces fit together and why the design holds to that principle.

## The pipeline

Every source flows through the same three stages:

1. **Ingest.** A connector fetches documents from a source (a Slack thread, a GitHub issue, a local file) and normalizes them to text. Change detection uses a per-document revision signal to decide what to re-fetch, and a content hash as the final authority on what to re-embed. A revision bump with identical text updates metadata only.
2. **Index.** Each document is split into fixed line-window chunks with a small overlap. Chunks are embedded with the local model and stored alongside a keyword-searchable copy. Stable chunk ids mean an unchanged document re-embeds nothing.
3. **Retrieve.** A query runs against both the vector store and the keyword scorer, and the two ranked lists are fused with weighted reciprocal-rank fusion. Filters, tag boosts, recency decay, and an optional rerank shape the final order.

## Hybrid retrieval

Neither vectors nor keywords win on their own, so Mari runs both. Vector similarity captures meaning, and keyword scoring captures exact identifiers and quoted phrases. Auto-weighting routes each query: identifier-like or quoted queries lean toward keywords, and natural-language questions lean toward vectors. Curation tags then adjust the ranking, so canonical documents surface first and stale ones sink.

## Storage

State lives in a per-workspace catalog, a DuckDB database that holds documents, chunks, embeddings, tags, facts, and a graph of edges between documents. Because the catalog is a single queryable store, `mari sql` can read it directly, and the graph primitives (`related`, `neighbors`) are ordinary queries rather than a separate service.

## Workspaces and scopes

A workspace is the index for one context. Every connector is scoped `global` or `local`:

- **Local** sources index into a per-repo workspace, identified by the repo path.
- **Global** sources index into a single shared `_global` workspace, so knowledge like a Slack workspace or a Drive is available from every repo.

When any connector is global, searches automatically union the repo workspace with `_global` and dedupe the results. Credentials live under your home directory with restrictive permissions, never in the repo.

## Why local-first

Two invariants drive the design. Configuration is files, not environment variables, so a repo's setup is versioned and reviewable. And the CLI never calls an external model, so your knowledge and credentials stay on your machine. The only work that needs a large model, such as rewriting or claim decomposition, is handed to Claude in-session rather than to a service the CLI calls. Team sharing, when you want it, goes through infrastructure you already control: Git Large File Storage (LFS), S3, or a hosted sync layer.
