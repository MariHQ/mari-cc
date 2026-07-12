# The curation model

Enterprise search tools help people find company knowledge. They are not built for teams to curate the context an AI agent uses. Search answers "where is the information?" Mari answers a different question: "what should our AI know, trust, and reuse?" Curation is what turns a pile of search results into a managed memory layer.

## Search is not enough

A search index returns whatever matches, ranked by relevance. It has no opinion about whether a document is current, trusted, or superseded. That is fine for a person who can judge a result at a glance. It is a problem for an agent that will treat any retrieved passage as fact. Without curation, the freshest pricing doc and a two-year-old draft compete on equal footing.

Curation adds that missing judgment. Instead of asking what the search engine found, a team can define what Claude should trust.

## The tag lifecycle

A tag is a status you attach to a document. Each status changes how Mari ranks, grounds, and lints that document:

| Status | Search | Factcheck | Hook |
|--------|--------|-----------|------|
| `canonical` | Ranks higher | Preferred evidence | Normal |
| `draft` | Ranks lower | Cannot support a claim | Normal |
| `stale` | Ranks lower | Cannot support, flagged | Advisory on edit |
| `deprecated` | Ranks lowest, shows replacement | Contradiction candidate | Advisory on edit |
| `internal` | Badge only | Neutral | Warns if referenced from customer-facing docs |
| `customer-facing` | Badge only | Held to stricter checks | Stricter linting |
| `needs-review` | Badge only | Neutral | Surfaced by `audit kb` |

The effects compound. Tagging the current pricing page `canonical` lifts it in search and makes it the preferred source when factchecking a launch post. Tagging the old one `deprecated` sinks it and, if a lineage edge points to the replacement, shows readers where to go instead.

Tags live in committed config, so the whole team shares one view of what to trust, and the history of that judgment is versioned with the code.

## Glossary and facts

Two more structures round out the managed memory. The glossary records approved terms and their forbidden variants, and it feeds the `terminology-consistency` detector rule, so house terms are enforced rather than merely suggested. The facts ledger records atomic, attributable facts, and it is the deterministic ground truth that [factchecking](../guides/factcheck.md) grades claims against. Together with tags, they move product knowledge from passive search results into context a team actively maintains.
