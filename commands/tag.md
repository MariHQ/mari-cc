---
description: Tag knowledge as canonical, stale, deprecated, draft, internal, customer-facing, or needs-review
argument-hint: "<path-or-ref> <status> | list | remove <ref>"
allowed-tools: Bash(mari *), Read, Edit
---

Curation tags are a team decision — confirm intent if ambiguous, then run `mari tag $ARGUMENTS`. Statuses: canonical, stale, deprecated, draft, internal, customer-facing, needs-review. Tags are stored in the committed `.mari/config.json`, so remind the user to commit. When tagging `deprecated`, suggest a lineage edge to the replacement (`mari lineage add`). `mari tag list [--status S]` and `mari tag remove <ref>` also work. Full flow: `references/reference-tag.md` in the mari skill.
