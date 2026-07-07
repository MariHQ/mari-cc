---
description: Sync tracked knowledge sources into the local index (explicitly user-triggered)
argument-hint: "[source]"
allowed-tools: Bash(mari *), Read, Edit
---

Run `mari sync $ARGUMENTS` now — this command is the one explicit user trigger for syncing; never run it unprompted in other contexts. Report the summary line (documents updated/removed, chunks embedded) and surface any per-source notes (unconnected sources print a nudge with the exact `mari auth` command — offer to run it). If sync reports the index was rebuilt or a source errored, explain what to do next. Details: the `mari` skill.
