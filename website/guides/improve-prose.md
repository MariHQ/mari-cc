# Improve AI-authored prose

Mari gives your team a shared editorial vocabulary for working with Claude. The deterministic detector finds problems, and the editorial verbs fix them. Every verb re-runs the detector afterward, so you can see that nothing regressed.

## Detect and audit

`mari detect` is the source of ground truth. It reads Markdown and reports findings without editing:

```sh
mari detect README.md
mari detect .              # walk the whole tree
```

Findings carry a rule id, a family (AI slop, clarity, style, inclusive), and a severity (error, warn, advisory). Useful flags:

- `--score` prints a 0 to 100 slop score with an explainable breakdown. See [Detector rules](../reference/detector-rules.md).
- `--strict` fails on warnings too, which makes it a continuous-integration gate.
- `--summary` prints the worst files and a rule histogram for large trees.
- `--style=<guide>` overrides the base style guide for one run.

For a human-facing report that pairs each finding with a suggested fix, run `mari audit` instead.

## The editorial verbs

The verbs run through the Claude Code skill, with `mari detect` backing each one before and after. Each preserves your meaning and voice. The rule is rewrite, not delete:

| Verb | What it does |
|------|--------------|
| `deslop` | Strips AI tells, clichés, and generic phrasing |
| `understate` | Cuts over-explanation and restated takeaways |
| `tighten` | Cuts wordiness and filler |
| `clarify` | Fixes jargon, acronyms, passive voice, and error-message copy |
| `sharpen` | Cuts hedges and commits to claims without inflating them |
| `soften` | Turns superlatives into checkable facts |
| `critique` | Scores argument, clarity, voice, and reader experience without rewriting |
| `polish` | Final pass: resolves findings, aligns to `STYLE.md`, reads aloud |
| `draft` | Outlines then writes a new piece end to end |

Run them from Claude Code as slash commands (`/deslop README.md`) or through `/mari`.

## Deeper passes

- **Narrative tier.** `deslop --narrative` adds a whole-document pass over structural tells that survive surface editing, such as stated morals and flat time. `mari narrative score <file>` prints the review metric behind it.
- **Grammar.** The grammar pass is opt-in. Add `--grammar` to a detect run or enable `detector.grammar` in config.

## What the detector will not do

The detector never claims a document "is AI-written." It points at spans worth rewriting and always shows why. It also never autofixes. The editing is yours, or Claude's through the verbs.
