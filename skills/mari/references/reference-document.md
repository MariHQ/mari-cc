# document — infer STYLE.md from existing writing

Reverse of `init`: read the project's *good* existing writing and generate a `STYLE.md` from the
observed patterns. Use when a project has strong writing but no documented style.

## Flow
1. Pick a corpus of the project's best prose (3–6 representative files).
2. Observe and record: heading case, list vs prose habits, emphasis discipline, oxford-comma
   stance, number style, contraction use, sentence-length rhythm, recurring approved phrasings.
3. Extract a **terminology glossary** — preferred term plus the forbidden variants you saw
   (sign in / log in, email / e-mail). This feeds `terminology-consistency`.
4. Write `STYLE.md` describing the house style as rules, with one real before/after example each.
5. Note any inconsistencies you found so the team can resolve them.

Describe what the writing *actually does*, not a generic ideal. If two files disagree, surface
the conflict rather than silently picking one.
