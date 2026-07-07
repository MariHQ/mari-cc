# Asset: RFC / design doc

A proposal that argues for a change and invites review before it's built. Conventions: the
[Rust RFC template](https://github.com/rust-lang/rfcs/blob/master/0000-template.md), Oxide RFD,
and the Squarespace/Google design-doc shape. Structure check: `mari asset check <file>`.
Scaffold: `mari asset scaffold rfc "<title>"`.

## Canonical structure (required)
- **Summary** — the proposal in a paragraph.
- **Motivation** — the problem and the goals (why do this at all?).
- **Alternatives** — other approaches considered and why they lost (or Rationale and alternatives).
- **Drawbacks** — the honest costs and risks of doing this.

Recommended: **Non-goals** (what's explicitly out of scope), **Unresolved Questions**.

## Tone
- Lead with the problem, not the solution. Make the reader feel the motivation first.
- Define scope by exclusion: state Non-goals explicitly.
- Argue fairly. Show the alternatives you rejected and why; a proposal with no drawbacks is
  unfinished, not perfect.

## Review rubric
1. Required sections present; Non-goals and Drawbacks especially — their absence is the common tell.
2. Motivation is concrete (a real problem, ideally with evidence), not "it would be nice".
3. At least one serious alternative is considered and rejected with reasoning.
4. Drawbacks are real tradeoffs, not strawmen.
5. Open questions are surfaced, not hidden.

Leans on: structure check + `marketing-buzzword`, `manufactured-contrast`, `hedge-overuse`.
