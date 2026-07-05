# Asset: Postmortem / incident retrospective

A blameless written record of an incident: impact, root cause, and what we'll change.
Conventions: [Google SRE](https://sre.google/sre-book/example-postmortem/), PagerDuty,
and the [Atlassian incident handbook](https://www.atlassian.com/incident-management/handbook/postmortems).
Structure check: `mari asset check <file>`. Scaffold: `mari asset scaffold postmortem "<incident>"`.

## Canonical structure (required)
- **Summary** — what happened, impact, resolution, in a paragraph.
- **Impact** — who/what was affected, how long, how badly.
- **Timeline** — timestamped events in one consistent timezone.
- **Root Cause** — the systemic cause (or Contributing Factors).
- **Action Items** — preventive work, each with an owner (or Corrective Actions).
- **Lessons Learned** — what went well / what went wrong / where we got lucky.

Recommended: **Detection**, **Resolution**.

## Tone — blameless (non-negotiable)
- Attribute to systems and process, **never** to individuals. Assume everyone acted in good
  faith on the information they had.
- Describe *how* a mistake was possible, not *who* made it. Human error is a symptom of a
  systemic gap. (The structure check flags blameful phrasing as `postmortem-blame`.)
- Distinguish the proximate trigger from the root cause; show the causal chain (five whys).

## Review rubric
1. All required sections present.
2. No blame or individual fault language; passive "mistakes were made" is fine, accusation is not.
3. Root cause is systemic and reached via a causal chain, not a single surface symptom.
4. Every action item has an owner (and ideally a due date and a tracking link).
5. Timeline is concrete and timezoned.

Leans on: structure check + `postmortem-blame` + `passive-voice` (acceptable here), `vague-attribution`.
