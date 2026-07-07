---
description: Check a file's claims against FACTS.md, a source-of-truth file, or the knowledge base
argument-hint: "<file> [--source FILE]"
allowed-tools: Bash(mari *), Read, Edit
---

Run `mari factcheck $ARGUMENTS` (add `--source <file>` to ground against a specific file, `--kb` for canonical-tagged knowledge). Report contradictions (errors) first with both values quoted, then unsupported claims. When the user asks for depth, run `mari factcheck <file> --emit-claim-targets`, decompose the candidate sentences into atomic claims yourself in-session, write them to a JSON file, and re-run with `--claims <file>` — the CLI never calls an LLM. Full flow: `references/reference-factcheck.md` in the mari skill.
