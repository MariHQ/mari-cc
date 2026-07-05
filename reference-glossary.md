# glossary — pull approved terms and phrasings into the style system

Harvest the project's real terminology into STYLE.md's glossary so `terminology-consistency`
can enforce it.

## Flow
1. Scan the target (or the corpus) for recurring product terms, feature names, and key phrases.
2. For each, pick the canonical form and list the variants that should resolve to it
   (e.g. `sign-in` over `signin`/`log in`/`login`).
3. Write them into STYLE.md's terminology glossary as `preferred term → forbidden variants`.
4. Note any genuine ambiguity for the user to decide rather than guessing the canonical term.

## Guardrails
- Record what the project already does well — don't invent terminology it hasn't chosen.
- One canonical term per concept; if two are both defensible, ask.

Leans on: `terminology-consistency` (this command supplies its dictionary).
