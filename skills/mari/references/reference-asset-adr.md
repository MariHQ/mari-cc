# Asset: ADR (Architecture Decision Record)

A record of one architectural decision and its rationale. Conventions: Michael Nygard's
template and [MADR](https://adr.github.io/madr/). Detection + structure check:
`mari asset check <file>`. Scaffold: `mari asset scaffold adr "<title>"`.

## Canonical structure (required)
- **Status** — proposed | accepted | rejected | deprecated | superseded (a metadata line or heading).
- **Context** — the forces at play (technical, political, project-local), value-neutral and factual.
- **Decision** — the change, in active voice: "We will …".
- **Consequences** — what becomes easier and harder; positive *and* negative.

Recommended: **Options Considered** (the alternatives and their tradeoffs).

## Tone
- Decision is decisive and active ("We will adopt X"), not hedged.
- Context states facts, not the verdict — no arguing for the decision here.
- Consequences are honest: name the costs, not just the wins.

## Review rubric
1. All four required sections present (run the structure check).
2. Status is a valid enum value; if superseded, links the superseding ADR.
3. Decision is one clear choice in active voice.
4. At least one real alternative was considered and rejected with a reason.
5. One decision per record — split compound decisions.

Leans on: structure check (`asset-missing-section`) + `hedge-overuse`, `passive-voice` in Decision.
