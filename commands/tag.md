---
description: Tag knowledge as canonical, stale, deprecated, draft, internal, customer-facing, or needs-review
argument-hint: "<path-or-ref> <status> | analyze | list | remove <ref>"
allowed-tools: Bash(mari *), Read, Edit
---

Curation tags are a team decision — confirm intent if ambiguous, then run `mari tag $ARGUMENTS`. Statuses: canonical, stale, deprecated, draft, internal, customer-facing, needs-review. Tags live in the catalog `tags` table and ride the shared warehouse (run `mari sync` first if there's no catalog yet). When tagging `deprecated`, add `--superseded-by <ref>` to record the replacement — it powers the pointer shown on `deprecated` search hits. `mari tag list [--status S]` and `mari tag remove <ref>` also work.

`/tag analyze [path…]` runs the bulk auto-tagging flow: `mari tag analyze` extracts deterministic context cards for the docs in scope, you judge from them, ask the user grouped questions about what you can't infer, and apply the agreed tags. Default posture is *untagged = current* — hunt only for docs that need special treatment. Full flows: `references/reference-tag.md` and `references/reference-tag-analyze.md` in the mari skill.
