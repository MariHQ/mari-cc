# Register: microcopy (UI strings, errors, empty states, buttons)

Every word is load-bearing. Optimize for a stressed reader in a small space.

## Bar
- **Sentence ceiling:** ~12 words. Often a fragment is right ("No results", "Saved").
- **Errors:** state what happened, why, and the next action — in that order, plainly. No blame,
  no "Oops!", no stack-trace jargon.
- **Buttons/labels:** verb-first, specific ("Delete file", not "OK"). Consistent terminology with
  the rest of the product.
- **i18n:** avoid idioms and puns; keep variables out of sentence grammar; leave room for ~30%
  expansion (German) — don't pack the string to the edge.

## Detector emphasis
Strict on: `long-sentence` (tight ceiling), `terminology-consistency`, `vague-link-text`,
`complex-word`, jargon. Relaxed on: nothing — microcopy is where discipline matters most.
