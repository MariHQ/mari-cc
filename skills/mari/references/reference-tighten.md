# tighten — cut wordiness, redundancy, filler

Make the prose shorter without losing meaning.

## Flow
1. Run the detector; read the clarity findings.
2. Apply, in order:
   - concision swaps (`wordy-phrase`: "in order to" → "to"; `complex-word`: "utilize" → "use")
   - drop redundant pairs (`redundant-pair`: "each and every" → "every")
   - kill expletive constructions (`there-is-expletive`: "There are X that…" → "X…")
   - cut filler (`filler-phrase`) and reflexive hedges/weasel words
   - split or merge over-long sentences (`long-sentence`)
3. Preserve the author's voice and every load-bearing claim. Tightening removes words, not meaning.
4. Re-run the detector to confirm the count dropped.

Leans on: `wordy-phrase`, `complex-word`, `redundant-pair`, `filler-phrase`, `long-sentence`,
`adverb-overuse`, `there-is-expletive`.
