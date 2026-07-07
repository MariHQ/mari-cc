# Asset: Security policy

How to report a vulnerability, and what's supported. Convention: GitHub's SECURITY.md (surfaced
in the repo's Security tab) — supported versions plus a private reporting path. Structure check:
`mari asset check <file>`. Scaffold: `mari asset scaffold security "<project>"`.

## Canonical structure (required)
- **Supported Versions** — which versions receive security fixes (a table is conventional).
- **Reporting a Vulnerability** — the private channel, what to include, and what to expect.

Recommended: **Disclosure Policy** (coordinated disclosure + credit), **Security Update Policy**.

## Tone
- Give a **private** channel — GitHub private advisory or a security email — never "open an issue".
- State a response-time expectation you can actually meet, not an aspirational one.
- Be exact about which versions are covered; vagueness here erodes trust.

## Review rubric
1. Both required sections present.
2. Reporting path is private and real (no public-issue instruction, no placeholder contact).
3. Supported-versions list is concrete and current.
4. A response-time commitment is stated.
5. Disclosure/credit expectations are set.

Leans on: structure check + `assistant-meta` (placeholder detection), `vague-link-text`, `passive-voice`.
