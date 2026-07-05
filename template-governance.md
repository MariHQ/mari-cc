<!--
  mari template: governance — Governance
  Grounding: GitHub's community-standards checklist

  Required sections (in order):
    1. Roles           (aliases: roles and responsibilities, project roles, responsibilities)
    2. Decision Making (aliases: decision-making, decision-making process, decisions,
                        how decisions are made)
    3. Maintainers     (aliases: becoming a maintainer, becoming a committer, committers)
  Recommended sections:
    - Meetings
    - Code of Conduct
    - Changing This Document (aliases: amendments, changing the governance)

  Tone norms:
    - Define each role by its concrete rights and duties, not just a title.
    - Make the path from contributor to maintainer explicit and measurable.
    - Say how the governance document itself gets amended.

  Detection heuristic (mari asset detect):
    - Canonical filename (+4, qualifying): GOVERNANCE.md (also .mdx/.rst/.txt; optional
      language suffix), case-insensitive, anywhere in the tree.
    - Directory match (+3, qualifying): .github/ or docs/governance/
    - Heading markers (+2 each, capped at 3 hits): roles, roles and responsibilities,
      project roles, responsibilities, decision making, decision-making,
      decision-making process, how decisions are made, maintainers,
      becoming a maintainer, becoming a committer, committers, contributors,
      meetings, voting, code of conduct, amendments, changing this document.
      Strong (content-only qualifying) markers: decision-making process,
      becoming a maintainer, becoming a committer, roles and responsibilities.
    - Best-scoring type wins; total score must be >= 4.

  Structure checks (mari asset check):
    - asset-missing-section (warn): each required section absent from headings.
    - asset-missing-section (advisory): each recommended section absent.

  Community-file role: recommended ("extra") artifact; `mari community` scaffolds it
  at GOVERNANCE.md via this same asset template.
-->
# <project> Governance

## Roles

- **Contributors** — anyone who submits issues or pull requests.
- **Maintainers** — trusted contributors with merge rights and release duties.

Describe the concrete rights and responsibilities of each role.

## Decision Making

How decisions are made — lazy consensus by default; describe when a vote is required and
what threshold carries it.

## Maintainers

How a contributor becomes a maintainer (the measurable bar), and how maintainers step down
or are removed.

## Meetings

When the project meets, where notes live, and how to add an agenda item.

## Changing This Document

How this governance document itself is amended.
