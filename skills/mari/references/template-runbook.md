<!--
  mari template: runbook — Runbook / operational guide
  Grounding: incident.io / emmer.dev / AWS IDR operational-runbook structure

  Required sections (in order):
    1. Overview       (aliases: purpose, summary, objective, description)
    2. Prerequisites  (aliases: preconditions, requirements, before you begin)
    3. Steps          (aliases: procedure, instructions, resolution, mitigation)
    4. Rollback       (aliases: recovery, remediation, cleanup)
    5. Escalation     (aliases: contacts, contact, on-call, who to contact)
  Recommended sections:
    - Triggers   (aliases: trigger, when to use)
    - Validation (aliases: verification)

  Tone norms:
    - Write steps as numbered, imperative actions ("Restart the service").
    - Give each step an expected outcome to check against.
    - Keep one runbook per procedure — branch into linked runbooks, not nested ifs.

  Detection heuristic (mari asset detect):
    - Directory match (+3, qualifying): /(runbooks?|playbooks?|ops|operations)/
    - Filename match (+3, qualifying): "runbook"/"run-book"/"run_book" or
      "playbook"/"play-book"/"play_book".
    - Heading markers (+2 each, capped at 3 hits): prerequisites, steps, procedure,
      rollback, escalation, trigger, triggers, when to use, validation, verification.
      Strong (content-only qualifying) markers: rollback, escalation.
    - Best-scoring type wins; total score must be >= 4.

  Structure checks (mari asset check):
    - asset-missing-section (warn): each required section absent from headings.
    - asset-missing-section (advisory): each recommended section absent.
-->
# Runbook: <procedure name>

## Overview

What this runbook does and when to reach for it.

## Triggers

The alerts or conditions that mean you should run this.

## Prerequisites

Access, tools, and state required before you start.

## Steps

1. Do the first action. Expected: <what you should see>.
2. Do the next action. Expected: <…>.

## Validation

How to confirm the procedure worked.

## Rollback

How to undo it if something goes wrong.

## Escalation

Who to page and when, if the steps don't resolve it.
