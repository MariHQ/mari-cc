# Design decisions and non-goals

Mari makes a few deliberate choices that shape how it behaves, and it draws clear boundaries around what it will not do. Knowing both helps you predict how the tool acts and avoids expecting features that were left out on purpose.

## The detector is deterministic

The detector is a rule engine, not a classifier. It never asserts that a document "is AI-written." It points at spans worth rewriting and always shows why, through an explainable finding and, for the slop score, a full breakdown. This matters for trust: a deterministic rule gives the same result every time and can be argued with, waived, or configured. A model that simply labels text cannot.

Machine-learning tiers layer on top of the rules, never replace them. The base tier runs offline with no dependencies. Small local models add entailment checking and slop-span extraction when provisioned. An attention model powers the opt-in `--deep` passes. Anything that needs generation is handed to Claude in-session.

## Local-first, files over environment

Two invariants are non-negotiable. Everything runs on your machine, with no hard SaaS dependency and no external large language model (LLM) calls from the CLI. And configuration is files, never environment variables, so a repo's setup is versioned, reviewable, and shared through the same pull request as the code. The only permitted environment toggles are capability switches for the optional model tiers. Credentials are the one thing kept out of the repo, stored under your home directory with restrictive permissions.

## Non-goals

Mari deliberately does not do these things:

- No SaaS requirement and no server in the core product. A hosted sync layer may exist later as an optional backend.
- No translation. The i18n commands check structure and coverage only.
- No source-code linting. Prose inside code is out of scope.
- No autofix by the detector, and no editing of external services' content.
- No redaction of personally identifiable information (PII) from indexed content, beyond protecting credentials. This is future work.
- No background daemon and no built-in cron. Wire your own scheduled `mari sync` if you want automation.
- No support for legacy binary Office formats (`.doc`, `.ppt`).

## Known rough edges

The specification is honest about work in progress. The HTTP client can intermittently stall on large GitHub and Slack responses, which is tracked for a fix by swapping the client. Prebuilt binaries and a package channel are still being set up, so for now you build from source. Treat the specification as the current source of truth when a behavior surprises you.
