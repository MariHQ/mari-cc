<!--
  mari template: postmortem — Postmortem / incident retrospective
  Grounding: Google SRE example postmortem + PagerDuty + Atlassian incident handbook

  Required sections (in order):
    1. Summary          (aliases: overview, what happened)
    2. Impact
    3. Timeline
    4. Root Cause       (aliases: root causes, contributing factors, fault)
    5. Action Items     (aliases: corrective actions, follow-up, follow-ups)
    6. Lessons Learned  (aliases: what went well, how'd we do)
  Recommended sections:
    - Detection
    - Resolution (aliases: recovery)

  Tone norms:
    - Stay blameless — attribute to systems and process, not individuals.
    - Distinguish the proximate cause from the root cause (five whys).
    - Give every action item an owner.

  Detection heuristic (mari asset detect):
    - Directory match (+3, qualifying): /(postmortems?|post-mortems?|incidents?|retros?)/
    - Filename match (+3, qualifying): "postmortem"/"post-mortem"/"post_mortem",
      word "retro"/"retrospective", or an "incident-"/"incident_" segment.
    - Heading markers (+2 each, capped at 3 hits): timeline, root cause, root causes,
      contributing factors, impact, action items, corrective actions, lessons learned,
      detection, resolution. Strong (content-only qualifying) markers: root cause,
      root causes, contributing factors, action items, corrective actions, lessons learned.
    - Best-scoring type wins; total score must be >= 4.

  Structure checks (mari asset check):
    - asset-missing-section (warn): each required section absent from headings.
    - asset-missing-section (advisory): each recommended section absent.
    - postmortem-blame (advisory): blameful phrasing anywhere in the text —
      "blame(d/s)", "at fault", "his/her/their fault|mistake|error", "should have known",
      "incompeten…", "careless", "negligen…". Message: postmortems stay blameless;
      attribute to systems and process, not people.
-->
# Postmortem: <incident name>

- Date: YYYY-MM-DD
- Authors: <names>
- Status: draft

## Summary

One paragraph: what happened, impact, and resolution. Blameless throughout.

## Impact

Who/what was affected, for how long, and how severely.

## Detection

How the incident was detected, and how long that took.

## Resolution

What was done to mitigate and recover.

## Timeline

- HH:MM — event (use a consistent timezone)

## Root Cause

The systemic cause. Distinguish the proximate trigger from the root cause (five whys).

## Action Items

| Action | Owner | Due |
|--------|-------|-----|
|        |       |     |

## Lessons Learned

- What went well
- What went wrong
- Where we got lucky
