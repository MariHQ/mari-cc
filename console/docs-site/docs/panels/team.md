---
title: Team panels
sidebar_position: 5
---

# Team panels

These panels cover sharing the knowledge base across a team and adjusting how Mari behaves.

## Cloud

Team sharing. The Cloud panel manages pushing and pulling the knowledge base to a shared warehouse, so teammates read one index instead of each building their own. It shows whether this workspace is a producer or a consumer of the shared replica. It maps to `mari cloud`.

## Config

Effective configuration, clustered by area. The Config panel shows the whole resolved configuration, grouped by area, and lets you edit repo or global values. Because config resolves in layers, the panel marks where each value comes from. Changes that affect indexing prompt a rebuild reminder, just as they do on the command line. It maps to `mari config`.

## See also

- [Knowledge panels](knowledge.md) to see what a shared index contains.
- The `mari` command-line tool has equivalents: `mari cloud` and `mari config`.
