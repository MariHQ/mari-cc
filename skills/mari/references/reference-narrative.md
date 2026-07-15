# narrative — the deslop tier for tells that survive editing (`deslop --narrative`)

Surface de-slopping doesn't remove the AI smell. Russell et al. (StoryScope, arXiv:2604.03136)
took AI text, rewrote every surface artifact — clichés, purple prose, redundant exposition — and
a discourse-level classifier still caught it (93.9% macro-F1, down 1.6 points from unedited).
The residue lives in *structural decisions*: what gets explained, how sections are shaped, what
the writing refuses to leave messy. This tier rewrites those decisions. It is whole-document
judgment, so **you run it in-session** — the CLI has no narrative rules and never will.

## Flow

1. **Base deslop first, always.** Run the detector and the standard `deslop` pass; the narrative
   read needs surface-clean text underneath it.
2. **Read the whole piece once, start to end.** Every dimension below is a document-level
   property — none can be judged from a chunk.
3. **One pass per dimension, in order.** Don't batch: applying a rubric this size in a single
   pass drops roughly a quarter of the checks (StoryScope measured 68% coverage single-call vs
   95% aspect-by-aspect).
4. **Re-read for meaning drift**, re-run the detector, and report what changed per dimension so
   the user can veto any move.

## The seven dimensions

For each: the AI-elevated tell, the human counterpart, the rewrite move. Prevalence figures are
human vs AI from the paper's 61,608-story corpus; the direction transfers to nonfiction even
where the magnitude was measured on fiction.

**1. Stated morals.** AI explains its own point: the narrator states the lesson (77% of AI
texts vs 52% human), sections close by restating their significance, the copy announces what
the reader should conclude. The lexical version: one thematic keyword restated in every section
(AI thematic-unity gap: 4.74 vs 4.41). *Move:* delete the takeaway sentence and end on the
concrete fact.
If the section is right, the reader draws the conclusion; if the conclusion must be stated,
the section failed.

**2. Tidy structure.** One unbroken causal chain, every element serving the central theme
(no-subplots: 79% AI vs 57% human), every section the same shape — problem, mechanism, payoff,
repeat. *Move:* vary section shapes; let one section be a bare fact or an aside; earn one
digression; delete a transition instead of writing one.

**3. Machine parallelism.** Triads, balanced clause pairs, one syntactic template recycled
across headings ("X. It's Y." — count them). Repetition of *form* is the tell even when no word
repeats. Two forms hide well: **closure cadence** — every paragraph landing on an aphoristic
mic-drop is a template even when each aphorism is fresh (let some paragraphs end on plain
information) — and **uniform sentence length**, all beats 8–18 words; humans ramble once.
*Move:* break one leg of the triad; make lists asymmetric; when two headings (or two adjacent
paragraphs' closers) share a shape, rewrite one of them.

**4. Performed embodiment.** Emotion rendered as bodily metaphor — readers "smell" it, failures
"sting," judgments are "quiet" (81% AI vs 38% human; humans mostly just name the thing: explicit
labels 29% vs 8%). *Move:* say it plainly, or swap the metaphor for one specific instance.
"Readers notice" beats "readers have learned to smell it."

**5. Vague allusion.** AI gestures at the world ("studies show," "industry leaders"); humans
name names at nearly double the rate (47% vs 24%) and mix explicit references with implicit
ones. *Move:* name the paper, the person, the year, the number — or cut the allusion. A date is
a human tell; "recently" is not.

**6. No concession, no reader.** AI protagonists are morally tidy (human ambivalence 59% vs
38%) and AI writes as if no one is watching (human direct address 28% vs 7%). In copy: a page
with no honest cost, no tradeoff, no aside. *Move:* concede one true, non-fatal cost; break
frame once. Both must be **true** — an invented flaw is worse slop.

**7. Flat time.** AI writes in an eternal present; humans jump — an anecdote, a date, a
flashback, a flash-forward (every temporal-complexity feature in the paper is human-elevated).
*Move:* anchor one section in a real moment; open one section mid-story instead of with a
thesis.

## The score — a number to optimize against

The CLI turns this rubric into a quantifiable narrative-slop score (0–100, lower = more human),
scored deterministically against the paper's published Table 15 human/AI means. YOU annotate;
the CLI does arithmetic. The loop:

1. `mari narrative questions --json` (default register `prose`; `--register
   fiction` for stories — 33 items instead of 15).
2. Answer every item **from the whole document** — honestly, before editing. Write
   `{"<id>": <value>}` to a scratchpad JSON (`"na"` to skip an item that truly doesn't apply).
3. `mari narrative score --answers <file> [--json]` → the score, the
   human/AI baselines for the answered subset, and the items with the largest pull toward AI,
   each mapped to its rewrite dimension above.
4. Rewrite the top-pull items' dimensions, then re-answer **only the items you tried to move**
   and re-score. Converge when the score sits nearer the human baseline than the AI one
   (`position < 0.5`; `< 0.35` reads human).

**Do not chase zero.** The baselines exist because real human writing scores ~30–35, not 0 —
a document at 5 is as suspicious as a document at 70. And the score is a proxy: it only stays
meaningful if the answers stay honest. Re-answering optimistically without rewriting, or
fabricating mess to flip an item, is Goodharting — the guardrails below outrank the number.

## Guardrails

- **Never fabricate the mess.** No invented anecdotes, flaws, or specificity. The truth
  constraint beats every dimension; if a pass added names or numbers, verify them against the source material.
- **One instance per human move.** A concession in every paragraph is just a new template.
- **Whole-document judgments only.** One triad is style; five triads are a voice. Never flag a
  single sentence.
- **Register-gate it.** Full strength on marketing, editorial, and long-form. For docs and
  microcopy apply only 1, 3, and 5 — reference docs *should* be tidy and linear.
- Leans on: nothing in the detector. This tier exists because the detector's 171 rules are
  surface rules, and the paper shows surface editing leaves the structural signal standing.
