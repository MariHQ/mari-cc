# understate — cut the over-explaining; trust the reader

The most durable AI tell isn't a word, it's a habit: the model explains its own point. It
states the moral, restates the intro as a conclusion, glosses what the reader already sees, and
appends the takeaway to a section that already landed it. Russell et al. (arXiv:2604.03136) call
this *over-determination* — the #1 feature separating AI from human writing, and the one that
survives every surface edit. `understate` removes it. Where `tighten` cuts wordy phrasing at the
sentence level, `understate` cuts whole explanatory *moves* the reader never needed.

## Flow
1. Run the detector; read `conclusion-restate`, `significance-boilerplate`, and any
   `transition-scaffolding` / `interrogative-answer` findings.
2. Rewrite, in order:
   - **Delete the restated takeaway.** A section that ends by announcing what it just showed
     ("What this means is…", "In short…", "The key point is…") should end on the last concrete
     fact instead. If the point needs restating, the section failed — fix the section, don't
     add the gloss.
   - **Cut the explained obvious.** "X does Y, which means Z" where Z follows plainly from Y →
     stop at Y. Drop "in other words" and the paraphrase that follows it; keep the better half.
   - **Kill the stated moral.** Remove the sentence that tells the reader how to feel or what to
     conclude (narratorial commentary). Let the evidence carry it.
   - **Say it once.** When a claim appears in the intro, the body, and the summary, keep the
     strongest instance and delete the echoes.
   - **Trust the transition.** Delete a scaffolding connective ("Moreover", "It's important to
     note that") rather than writing one; often the next sentence stands alone.
3. Preserve the information. Understating removes *redundant explanation*, never a fact, a
   caveat, or a step. If cutting a sentence loses content the reader needs, it wasn't
   over-explanation — keep it.

## Guardrails
- Don't strand the reader. Genuinely hard ideas need their explanation; the target is the
  explanation of the *easy* ones. Calibrate to audience — an expert reference tolerates far
  less hand-holding than a beginner tutorial.
- Register-gate it. Full strength on marketing, blog, and editorial prose. For reference docs
  and tutorials, cut only true restatement (the summary that duplicates the body), not the
  first, load-bearing explanation.
- This is the sentence-and-paragraph front end of `deslop --narrative` dimension 1 (stated
  morals). For a whole-document pass and a score to converge against, run that tier and
  `mari narrative score`.

Leans on: `conclusion-restate`, `significance-boilerplate`, `transition-scaffolding`,
`interrogative-answer`.
