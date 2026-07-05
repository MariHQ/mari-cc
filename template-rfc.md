<!--
  mari template: rfc — RFC / design doc
  Grounding: Rust RFC 0000-template.md + Oxide RFD + Squarespace RFC

  Required sections (in order):
    1. Summary      (aliases: abstract, overview, tldr)
    2. Motivation   (aliases: goals, problem, problem statement, problem frame,
                     background, requirements, why)
    3. Alternatives (aliases: rationale and alternatives, alternatives considered,
                     other approaches)
    4. Drawbacks    (aliases: risks, tradeoffs, trade-offs, downsides)
  Recommended sections:
    - Non-goals            (aliases: non goals, out of scope, scope boundaries)
    - Unresolved Questions (aliases: open questions, open product decisions)

  Tone norms:
    - State Non-goals explicitly — scope is defined by what you exclude.
    - Show the alternatives you considered and why you rejected them.
    - Name the Drawbacks honestly; a proposal with no downsides is unfinished.

  Detection heuristic (mari asset detect):
    - Directory match (+3, qualifying): /(rfcs?|rfds?|designs?|proposals?|plans?)/
      or docs/(design|rfcs?|proposals?|plans?)/
    - Filename match (+3, qualifying): "rfc-"/"rfd-" prefix, "-design.md",
      "design-" segment, "proposal", or "-plan.md". A bare NNNN-*.md filename
      scores only +1 and is NOT qualifying (ambiguous with ADR).
    - Heading markers (+2 each, capped at 3 hits): motivation, goals, non-goals,
      alternatives, drawbacks, rationale and alternatives, guide-level explanation,
      reference-level explanation, unresolved questions, open questions, prior art,
      summary, problem, problem statement, problem frame, background, requirements,
      out of scope, in scope, proposed, design, verification. Strong (content-only
      qualifying) markers: non-goals, drawbacks, alternatives, rationale and
      alternatives, unresolved questions, out of scope, prior art, guide-level
      explanation, reference-level explanation, problem frame.
    - Best-scoring type wins; total score must be >= 4.

  Structure checks (mari asset check):
    - asset-missing-section (warn): each required section absent from headings.
    - asset-missing-section (advisory): each recommended section absent.
-->
# RFC: <title>

- Status: draft
- Authors: <names>

## Summary

One paragraph explaining the proposal.

## Motivation

Why are we doing this? What problem does it solve? What are the goals?

## Non-goals

What this proposal explicitly does not address.

## Design

The proposal itself, in enough detail to implement and evaluate.

## Alternatives

Other approaches considered, and why they were not chosen.

## Drawbacks

Why might we *not* do this? The honest costs and risks.

## Unresolved Questions

What's still open and needs to be decided.
