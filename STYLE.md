# STYLE
Base style guide: **Microsoft Writing Style Guide**. Register: docs.

## Voice
- Confident and direct. State what Mari does. Skip the hedging.
- Local-first is the through-line. When it's true, say the work stays on the user's machine.
- Technical but plain. Name the mechanism (hybrid search, NLI model, post-edit hook) without jargon for its own sake.
- Evidence-backed. Claims about AI tells or coverage cite the measurement, not vibes.
- Second person ("you", "your team"). Contractions are fine.

## Terminology
<!-- mari-disable-next-line long-sentence: markdown table rows, not sentences -->
| Use | Not |
|-----|-----|
| Mari | mari, MARI |
| plugin (the packaged product) | app, extension, wrapper |
| binary (the `mari` executable) | tool, program |
| detector | linter, scanner |
| finding | issue, error (unless it's the `error` severity) |
| register | mode, genre |
| hook (the post-edit mechanism) | integration, trigger |
| knowledge base | index, corpus (when speaking to users) |
| facts ledger | facts file, FACTS |
| AI tell / slop | "AI-written" (never claim a doc *is* AI-written) |

## Formatting
- Headings in sentence case.
- No em dashes and no semicolons in human-facing copy. Recast into two sentences, a comma, or a colon.
- Code, commands, file paths, and config keys in backticks.
- Expand an acronym on first use (LLM, NLI, CLI, OCR).
- Use a list when items are parallel. Use a sentence when the items are one thought.
- Prefer straight quotes over curly quotes in prose that ships as source.

<!-- mari-disable manufactured-contrast: this section quotes the patterns it forbids -->
<!-- mari-disable vague-attribution: this section quotes the patterns it forbids -->
<!-- mari-disable filler-phrase: this section quotes the patterns it forbids -->
## Forbidden phrasings
- Cliché openers ("In today's fast-paced world…", "In the ever-evolving landscape of…").
- Manufactured contrast ("not just X — it's Y", "not only… but…").
- Vague attribution ("studies show", "experts say") without a citation.
- Reflexive hedging ("it's important to note that", "it could be argued that").
- Marketing filler ("frictionless", "seamless", "leverage", "robust", "unlock").
