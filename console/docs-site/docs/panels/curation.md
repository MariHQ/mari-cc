---
title: Curation panels
sidebar_position: 2
---

# Curation panels

Curation is how you tell Claude what to trust. These panels manage the tags, glossary, and facts that turn a search index into a maintained memory layer.

## Tags

Curation tags applied to documents, and the status vocabulary. The Tags panel lists every tagged document and lets you apply, change, or remove a tag from the seven-status vocabulary: canonical, draft, stale, deprecated, internal, customer-facing, and needs-review. You can also edit the status vocabulary itself. Tags change how documents rank in search and whether they can support a factcheck. It maps to `mari tag`.

## Glossary

Approved terms and their forbidden variants. The Glossary panel shows the terminology table from `STYLE.md`, with the preferred term in one column and the variants to avoid in the other. Approved rows feed the `terminology-consistency` detector rule, so your house terms are enforced. It maps to `mari glossary`.

## Facts

The facts ledger. The Facts panel lists the entries in `FACTS.md`, one atomic fact per row with its source attribution. This ledger is the deterministic ground truth that factchecking grades claims against. It maps to `mari facts`.

## See also

- [Maintenance panels](maintenance.md) for the detector that enforces glossary terms.
- The `mari` command-line tool has equivalents: `mari tag`, `mari glossary`, and `mari facts`.
