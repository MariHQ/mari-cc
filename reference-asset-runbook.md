# Asset: Runbook / operational guide

A procedure an on-call engineer follows under pressure. Conventions: incident.io's "5 A's"
(Actionable, Accessible, Accurate, Authoritative, Adaptable), AWS IDR, and common SRE practice.
Structure check: `mari asset check <file>`. Scaffold: `mari asset scaffold runbook "<procedure>"`.

## Canonical structure (required)
- **Overview** — what the procedure does and when to reach for it.
- **Prerequisites** — access, tools, and state needed before starting.
- **Steps** — numbered, imperative actions, each with an expected outcome.
- **Rollback** — how to undo it if it goes wrong (or Recovery / Remediation).
- **Escalation** — who to page, and when, if the steps don't resolve it.

Recommended: **Triggers** (the alerts/conditions that invoke it), **Validation**.

## Tone
- Steps are imperative and testable: "Restart the service" → "Expected: health check returns 200".
- One runbook per procedure. If diagnosis branches, link out to a separate runbook — don't nest
  conditionals.
- Write for 3 a.m.: no prose padding, no assumed context, exact commands.

## Review rubric
1. All required sections present.
2. Steps are numbered and imperative, not narrative paragraphs.
3. Each step states how to confirm it worked.
4. Rollback exists and is specific.
5. Escalation names a concrete owner/rotation, not "the team".

Leans on: structure check + `long-sentence`, `passive-voice`, `minimizing-words` ("simply", "just").
