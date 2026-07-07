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
   - tables: run `mari detect --strings`-style normalization by hand or pass `mari detect`
     over the file first; to normalize every markdown table to one canonical GFM form
     (aligned pipes, one header rule), use `mari format`'s `--tables` pass
3. Don't touch the wording — this pass is about the container, not the content.

## Labels and microcopy

`format` is the routing owner for nav-label and microcopy consistency — parallelism, casing, and
terminology across a set of labels. Two deterministic entry points feed it:

- `mari detect --labels` treats each input line as its own unit, so a list of nav titles or menu
  labels over stdin doesn't trip whole-document rules like `long-sentence`.
- `mari detect --strings <dir>` extracts user-facing copy from code (JSX/TSX text and string
  literals, `className`/import/attribute noise excluded) and lints it, so labels living in `.tsx`
  are checked like markdown. Findings point back to the real source line.

Leans on: `sentence-case-heading`, `skipped-heading`, `excessive-bold`, `bold-lead-in-list`,
`listicle-reflex`, `vague-link-text`, `terminology-consistency`.
