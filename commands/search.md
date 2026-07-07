---
description: Search the team knowledge base (natural language or keywords)
argument-hint: "<question>"
allowed-tools: Bash(mari *), Read, Edit
---

Answer the user's question from the Mari knowledge index. $ARGUMENTS is the question — natural language is expected, not just keywords.

Compose a toolbox, not one search: run `mari search "$ARGUMENTS" --variant "<agent-written rephrasing>" --variant "<another>"` (you are the query-expansion step), then follow up with `mari doc`, `mari thread`, `mari related`, `mari recent`, `mari neighbors`, or `mari sql` as leads emerge. Extract identifiers from early hits and feed them back as variants. Never conclude from a truncated preview — use `--full` before quoting a source. Answer from the current index even when stale; suggest `/sync` but never run it unprompted. Cite each claim with its source ref from the hits. Full flow: the `mari` skill, knowledge section.
