<!--
  mari template: adr — ADR (Architecture Decision Record)
  Grounding: Michael Nygard's template + MADR (https://adr.github.io/madr/)

  Required sections (in order):
    1. Status        (metadata field — a "Status:" line or front-matter key counts; aliases: status)
    2. Context       (aliases: context and problem statement)
    3. Decision      (aliases: decision outcome)
    4. Consequences
  Recommended sections:
    - Considered Options (aliases: options, alternatives)

  Tone norms:
    - Write the Decision in active voice ("We will…").
    - Keep Context value-neutral — describe the forces, not the verdict.
    - List Consequences both positive and negative.

  Detection heuristic (mari asset detect):
    - Directory match (+3, qualifying): /(adr|adrs|decisions)/ or docs/(adr|decisions)/
    - Filename match (+3, qualifying): "adr-"/"adr_" prefix or word "adr"; a bare NNNN-*.md or
      YYYYMMDD-*.md filename scores only +1 and is NOT qualifying (ambiguous with RFC).
    - Front-matter status in {proposed, accepted, rejected, deprecated, superseded} (+3, qualifying).
    - Heading markers (+2 each, capped at 3 hits): decision, consequences, context,
      considered options, decision outcome, status. Strong (content-only qualifying) markers:
      consequences, considered options, decision outcome.
    - Best-scoring type wins; total score must be >= 4.

  Structure checks (mari asset check):
    - asset-missing-section (warn): each required section absent from headings
      (Status also accepted as a "Status:" metadata field or front-matter key).
    - asset-missing-section (advisory): each recommended section absent.
-->
# NNNN. Short decision title

- Status: proposed
- Date: YYYY-MM-DD
- Deciders: <names>

## Context

What is the issue we're facing? Describe the forces at play — technical, political,
project-local — in value-neutral, factual language.

## Options Considered

- Option A — tradeoffs.
- Option B — tradeoffs.

## Decision

What is the change we're making? State it in active voice: "We will …".

## Consequences

What becomes easier or harder as a result? List the outcomes, positive and negative.
