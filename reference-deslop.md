# deslop — strip AI tells, rewrite in human voice (signature)

The reason the product exists. Rewrite — don't just delete — the statistically-measurable tells
of machine prose, preserving meaning and the project's voice.

## Flow
1. Run the detector first (`mari detect <target>`) and read the Family A findings.
2. Rewrite, targeting in priority order:
   - overused vocabulary (delve, tapestry, underscore, meticulous, …) → plain words
   - marketing buzzwords, cliché openers, manufactured contrast ("not just X — it's Y")
   - conclusion-restate, vague attribution, significance/legacy boilerplate
   - em-dash overuse, emoji decoration, bold-lead-in lists
   - assistant meta-phrases, sycophancy, transition/conversational scaffolding, listicle reflex
3. Keep the author's meaning and register. Replace, don't gut — a deslopped sentence should say
   the same thing in a human voice.
4. Re-run the detector; confirm Family A is quiet and nothing else regressed.

## `--narrative` tier

Surface tells are the cheap half. When the user asks for `deslop --narrative` — or a text passes
the detector clean yet still reads machine-made — load `reference-narrative.md` and run its
seven whole-document dimension passes (stated morals, tidy structure, machine parallelism,
performed embodiment, vague allusion, no concession, flat time) on top of this flow. Base deslop
first, always: the narrative read needs surface-clean text. This tier is in-session judgment;
the CLI has no narrative rules — but it does score them: `mari narrative questions` /
`narrative score --answers <json>` quantifies the tier (0–100 vs published human/AI baselines)
so the rewrite loop has a number to converge on. The reference has the loop.

## Keep what humans do

De-slopping removes the machine signal; don't sand off the human one with it:
- Sentence-initial "And", "But", and "So" are fine. So are fragments, when they land.
- Back a superlative with a measurable or cut it ("loads in 200ms", not "blazing fast") —
  `soften` has the full flow.
- Final gut check, after the detector is quiet: read the result aloud (mentally). A sentence
  you wouldn't say to a colleague isn't done.

## Guardrails
- Don't flatten a real voice into generic plainness. Read a representative file first.
- A single "AI word" is noise; act on density and co-occurrence, which the detector already
  gates. If the project bans a tell outright (em-dashes, semicolons), set that per rule with
  `mari zero add <rule-id>` instead of pretending the density gate said so.
- Leans on: Family A (all) + `wordy-phrase`, `complex-word`.
