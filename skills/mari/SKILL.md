---
name: mari
description: Improve prose and documentation with deterministic rules, project terminology, editorial workflows, localization checks, and documentation maintenance. Use when the user asks to write, rewrite, edit, critique, audit, polish, understate, tighten, clarify, sharpen, de-slop, localize, or maintain prose, or invokes /mari and its editorial commands.
version: 0.4.0
user-invocable: true
argument-hint: "[command] [target]"
allowed-tools: Bash(mari *), Read, Edit
---

# Mari

Mari is a prose and documentation system. Its detector supplies concrete findings; you make the editorial judgment and edits.

## Before editing

1. Read the target and at least one representative file from the same project.
2. Read `PRODUCT.md` and `STYLE.md` when present.
3. Choose the closest register reference: docs, marketing, editorial, or microcopy.
4. Run `mari detect <target>` and use the highest-value findings to guide the pass.
5. For a recognized document type, run `mari asset detect <target>`, load its reference, and run `mari asset check <target>`.

Project settings are read from `.mari/config.json`, with personal repository overrides in `.mari/config.local.json`.

## Routing

- `deslop`, `tighten`, `clarify`, `sharpen`, `understate`, `critique`, `polish`, and `draft`: load the matching reference and follow it.
- `voice`, `cadence`, `format`, `delight`, `harden`, `adapt`, `localize`, and `narrative`: load the matching reference.
- `detect`, `audit`, `asset`, `platform`, `surface`, `check`, `i18n`, `rules`, `nudge`, `glossary`, `config`, `status`, and `features`: run the CLI command directly. Use `--strict` when the user asks for a release or CI gate.
- A general request such as “make this better”: inspect the text first, then choose the smallest useful pass. Use `deslop` for canned phrasing, `tighten` for excess length, `clarify` for ambiguity, and `polish` for a final multi-category pass.

## Editing standard

- Preserve facts, names, numbers, links, examples, and constraints unless the user asks to change them.
- Match the project's existing register instead of imposing a generic voice.
- Prefer direct sentences, specific verbs, varied cadence, and concrete claims.
- Remove throat-clearing, repeated conclusions, canned transitions, inflated language, fake quotations, and unnecessary headings.
- Do not optimize for a detector score at the expense of meaning.
- After editing, run `mari detect <target>` again and review the diff.
- Explain material changes briefly; do not narrate every mechanical edit.

## Documentation workflows

- `mari surface [dir]` extracts public symbols, headings, configuration keys, and command-like code spans.
- `mari check --strict` validates documentation structure, links, navigation, and community files.
- `mari i18n conform <file-or-dir> --strict` compares localization structure.
- `mari rules` and `mari nudge` maintain repository edit obligations.

## Glossary

Use `mari glossary list` to inspect the terminology table in `STYLE.md`. Add a preferred term with `mari glossary add <term> --not <discouraged>` only after the user approves the terminology choice.
