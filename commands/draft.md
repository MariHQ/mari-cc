---
description: Outline and write a new document in the team's voice
argument-hint: "<brief>"
allowed-tools: Bash(mari *), Read, Edit
---

Run the **draft** verb from the mari skill: $ARGUMENTS is the brief. Flow (`references/reference-draft.md`): load editorial context (PRODUCT.md, STYLE.md, FACTS.md), search the knowledge base for prior art (`mari search`), outline, write in the team's register and voice, self-deslop, then verify with `mari detect` and `mari factcheck` before presenting. If the document matches a known archetype (RFC, ADR, runbook, postmortem…), scaffold with `mari asset scaffold <type>` and honor its required sections.
