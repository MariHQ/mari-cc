# polish — final pre-publish pass

Resolve everything outstanding and align to the house style. Run this last.

## Flow
1. Load the latest `critique` snapshot (if one exists) — that's the backlog.
2. Run the detector and collect all findings.
3. Resolve, in order: `error` → `warn` → the critique's top issues → `advisory` worth fixing.
4. Align to `STYLE.md`: terminology glossary, heading case, emphasis discipline, number style.
5. Verify nothing regressed — re-run the detector; the count should only go down.
6. Read the piece aloud (mentally) one last time — the detector can't hear a sentence nobody
   would say. Rewrite anything that fails the ear.
7. Report what changed and what you deliberately left (with reasons).

Polish is the convergence step: it should leave the piece publishable. Don't introduce new
slop while fixing old; re-read for voice after the mechanical pass.

Leans on: all rules + the critique snapshot.
