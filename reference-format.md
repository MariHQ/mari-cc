# format — fix headings, lists, emphasis, and markdown structure

Structure-level cleanup, not sentence-level. Make the document scannable and the markup honest.

## Flow
1. Run the detector; read the structural findings.
2. Fix, in order:
   - heading case and hierarchy (`sentence-case-heading`, `skipped-heading` — no jumping h2→h4)
   - list vs prose: a list of full sentences is usually prose (`listicle-reflex`); strip
     `bold-lead-in-list` where it's decoration
   - emphasis discipline (`excessive-bold`) — if everything's bold, nothing is
   - link text says where it goes (`vague-link-text`: not "click here")
   - code, commands, and paths in backticks
3. Don't touch the wording — this pass is about the container, not the content.

Leans on: `sentence-case-heading`, `skipped-heading`, `excessive-bold`, `bold-lead-in-list`,
`listicle-reflex`, `vague-link-text`.
