# Mari

Mari is a prose-quality plugin for Claude Code. It gives teams a shared editorial system that catches weak writing during an edit and helps Claude revise with purpose.

## Product promise

Mari turns house style into actionable feedback. Its detector identifies the passage, the rule, and the reason; Claude handles the rewrite with the surrounding project context.

## Core workflows

- Detect canned phrasing, ambiguity, excess length, grammar problems, inclusive-language issues, and house-style violations.
- Run focused editorial passes such as `deslop`, `tighten`, `clarify`, `sharpen`, `understate`, `critique`, and `polish`.
- Maintain preferred terminology in `STYLE.md` and configure detector waivers or zero-tolerance rules.
- Check localization structure and keep documentation obligations visible through edit-notify rules and nudges.
- Inspect public code and documentation surfaces, validate links and navigation, and scaffold common document types.
- Review detector rules, 49 configurable word lists, and repository settings in the local console.

## Configuration

Projects commit `.mari/config.json`. A developer may use `.mari/config.local.json` for repository-specific personal overrides.

## Principles

- Findings must point to concrete text and a named rule.
- Editorial commands preserve facts and constraints by default.
- Repository conventions take precedence over generic style advice.
- Configuration stays reviewable alongside the writing it governs.
