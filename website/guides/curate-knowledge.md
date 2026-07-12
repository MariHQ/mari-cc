# Curate what Claude should trust

Search answers "where is the information?" Curation answers "what should Claude trust?" Mari turns passive search results into a managed memory layer through tags, a glossary, and a facts ledger. For the reasoning behind the model, see [The curation model](../explanation/curation-model.md).

## Tag knowledge

Tag any repo file or indexed document with one status:

```sh
mari tag docs/pricing.md canonical
mari tag old-onboarding.md stale
mari tag list
```

The seven statuses each change how Mari treats the document:

| Status | Effect |
|--------|--------|
| `canonical` | Ranks higher in search, preferred as factcheck evidence |
| `draft` | Ranks lower, cannot support a claim |
| `stale` | Ranks lower, cannot support claims, advisory when you edit it |
| `deprecated` | Ranks lowest, shows its replacement, treated as a contradiction candidate |
| `internal` | Badge only, warns when referenced from customer-facing docs |
| `customer-facing` | Badge only, held to stricter linting |
| `needs-review` | Badge only, surfaced by `mari audit kb` |

Tags are stored in committed `.mari/config.json`, so they are versioned and shared with your team.

## Keep a glossary

Approved terms live in the Terminology table of `STYLE.md`. Mari can mine candidates from your repo and knowledge base:

```sh
mari glossary harvest      # propose Use/Not rows for you to accept
mari glossary list
mari glossary add tell --use "AI tell" --not "AI-ism,robotic phrasing"
```

Accepted rows feed the `terminology-consistency` detector rule, so your house terms are enforced automatically.

## Maintain a facts ledger

`FACTS.md` holds one fact per line, with optional source attribution. It is the deterministic ground truth for [factchecking](factcheck.md):

```sh
mari facts add "Free tier allows 3 projects" --source PRODUCT.md
mari facts list
```

For bulk work, `mari extract facts` pulls candidate facts (numbers, dates, pricing, limits) from recent knowledge, which you review before they are written:

```sh
mari extract facts --source slack --since 30
```

## Audit the knowledge base

`mari audit kb` scans the whole index for problems: stale pages, contradiction candidates, duplicated content, unsupported claims, inconsistent terminology, the `needs-review` backlog, and content that drifts from `PRODUCT.md`. It produces a prioritized report and edits nothing.

```sh
mari audit kb
```
