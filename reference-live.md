# live — in-place iteration: generate alternatives for a selected span

Prose analog of a live variant mode. Pick a sentence or paragraph, get N rewrites at different
intensities, keep the one that fits.

## Flow
1. Take the selection from the editor, or read it from stdin (`mari live` reads
   stdin and prints numbered alternatives; the skill path uses the editor selection).
2. Generate a small set of alternatives at distinct intensities — for example **tighter**
   (cut words), **bolder** (more direct, more voice), **quieter** (calmer, less hype) — each a
   real rewrite, not a synonym swap.
3. Present them numbered with a one-word label. Let the user pick; apply the chosen one in
   place. Offer another round on the result if asked.

## Guardrails
- Every alternative must preserve the span's meaning and the project's voice — vary the dial,
  not the facts.
- Keep the set small (three is usually right). A wall of variants is noise.

Leans on: `deslop` + `tighten` instincts per intensity; run the detector on the chosen result.
