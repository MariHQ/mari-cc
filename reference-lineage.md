# lineage — curate the semantic-lineage graph

Semantic lineage is a knowledge graph of span↔span links across the repo — a code symbol and
the doc paragraph that documents it, two doc passages that state the same fact, a config block
and the guide that explains it. Machines *propose* the links (embeddings, attention, symbol
mentions); a human or you *curates* them; the result lives in an embedded DuckDB at
`.mari/lineage.duckdb`. A confirmed edge is a promise: **when one side changes, the other side
gets reviewed.** The post-edit hook enforces the promise by injecting the impact into the
session the moment a curated span's content changes.

All commands: `mari lineage …`

## 1. Propose

```
mari lineage propose
```

Candidates come from two sources, both inserted as `proposed` (already-curated pairs never
resurface, including rejected ones):

- **Symbol mentions** — exported functions/classes named in docs (high precision, free).
- **Assoc index** — embedding/attention associations from `mari assoc build [--attn]`. If no
  index exists, offer to build one first (it embeds the whole repo — minutes on a big repo;
  `--attn` is slower and higher quality). Symbol candidates work without it.

## 1.5 Refine spans (attention — recommended before reviewing embedding edges)

```
mari lineage refine --limit 20
```

Embedding candidates inherit the assoc index's fixed ~40-line RAG chunks — retrieval units,
not promise units. `refine` runs attention focus on each side of a proposed edge (context =
the span, query = the counterpart) and shrinks the span to the lines where the counterpart's
attention mass actually lands. Symbol candidates are already definition-/paragraph-bounded and
rarely need it. ~6s per edge; needs the native attn binary + a GGUF model — skip silently if
unavailable. Tighter spans mean cleaner review decisions AND more precise impact later (an
edit inside the old 40-line window but outside the real promise no longer fires).

## 2. Curate — this is your job

```
mari lineage review --json --limit 20
```

Each proposed edge carries both span texts. For **every** edge, read both spans (open the
files if the stored text is truncated or you need surrounding context) and decide:

- **The doc genuinely describes/depends on that code, or the two passages state the same
  thing** → `lineage confirm <id> --rel <relation> --by llm`, with the relation that fits:
  `documents` (doc explains code), `implements` (code realizes a spec/doc), `describes`
  (looser prose↔artifact), `duplicates` (two spans state the same fact), `derives-from`
  (generated/adapted content), `related` (real coupling, none of the above).
- **Vocabulary coincidence, boilerplate overlap, or a mention too incidental to promise
  sync** → `lineage reject <id> --by llm --note "why"`.
- **Genuinely unsure whether the link should be a maintenance promise** → ask the user;
  batch these into one question at the end, don't interrupt per edge.

Confirming is consequential — every confirmed edge will interrupt future editing sessions when
it drifts. Reject freely; a lean graph of real promises beats a complete graph of maybes.
Repeat propose→review until `lineage review` comes back empty. `--note` is worth filling on
confirms too: it's shown in the impact prompt later ("keep the flag list in sync").

Users can assert links you can't infer: `lineage link src/a.mjs:10-40 docs/a.md:5-12 --rel
documents --by human`. Offer this when they point out a dependency mid-conversation.

## 3. Impact — when things change

The installed post-edit hook (`mari install`) checks every edited file against confirmed
edges. It compares *content hashes* of the curated spans, so whitespace churn and line
movement don't fire — only real drift does. When it fires you'll see:

> ⛓ Mari semantic lineage — `src/parser.mjs` has curated links whose content just changed…

Treat it as part of the current task, immediately:

1. Open each counterpart span and decide: update it, or confirm it's still accurate.
2. When done: `mari lineage stamp <edited-file>` — records the new content as
   the curated baseline (also re-anchors spans that merely moved).
3. If the link itself was wrong, `lineage reject <id> --by llm` instead of stamping.

Never stamp *before* reviewing the counterpart — stamping is the statement that both sides
agree again.

Outside the hook (e.g. reviewing a branch): `lineage impact [file…]` checks explicit files, or
all git-dirty files with no argument. `lineage stats` / `lineage list --status confirmed`
summarize the graph.
