---
title: Docs panels
sidebar_position: 4
---

# Docs panels

These panels cover documentation work: getting a docs site ready, keeping translations in sync, and scaffolding new documents from templates.

## Docsite

Docs-site readiness and the build plan. The Docsite panel inspects the repository for an existing platform, a docs directory, community-health files, hook configuration, and edit-notify rules, then reports what is in place and what is missing. The commands it suggests run in your terminal or through `/mari`. It maps to `mari docsite status` and `mari check`.

## Localization

Translation coverage and structural drift. The Localization panel shows which documents have translations, whether each translation shares the source's structure, and, with the deep attention pass, which passages a translation barely covers. Expand a document to explore inline. It maps to `mari i18n conform` and `mari i18n coverage`.

## Templates

Document archetypes. The Templates panel lists the standard document types Mari can scaffold, such as runbook, architecture decision record (ADR), postmortem, RFC, and the community-health files. Pick one to scaffold a new document from a best-practice template. It maps to `mari asset scaffold`.

## See also

- [Team panels](team.md) for sharing and configuration.
- The `mari` command-line tool has equivalents: `mari docsite`, `mari check`, `mari i18n`, and `mari asset`.
