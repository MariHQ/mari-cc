# Factcheck claims

`mari factcheck` checks a document's claims against ground truth and flags contradictions, mismatched numbers, and unsupported statements before they publish. It works in layers, from an instant deterministic pass to on-device attention grounding.

## Ground against a facts ledger

By default, factcheck extracts typed spans (numbers, money, percentages, years, dates, entities) and matches them against your [facts ledger](curate-knowledge.md):

```sh
mari factcheck launch-post.md
```

Findings come in four kinds: `number-date-mismatch` and `contradicts-fact` are errors, `unsupported-claim` is a warning, and `ungrounded-span` is an advisory. A source tagged `stale` or `deprecated` cannot support a claim.

## Ground against a specific source

Point factcheck at any file as the source of truth:

```sh
mari factcheck pricing-page.md --source PRODUCT.md
```

Or check against the canonical-tagged documents in your knowledge base:

```sh
mari factcheck pricing-page.md --kb
```

## Deeper grounding

The default pass is deterministic and instant. Two opt-in layers go further:

- **Local NLI.** Add `--models` to bring in local entailment and contradiction checking, which catches claims that conflict in meaning rather than in a literal span.
- **Attention grounding.** Add `--deep` (with `--source`) to check each sentence against the source on-device using the attention model. Set the sensitivity with `--threshold`.

## Atomic-claim decomposition

For the most thorough pass, factcheck can grade one atomic claim at a time. Because the CLI never calls an LLM, Claude does the decomposition in-session:

1. Run `mari factcheck <file> --emit-claim-targets` to print candidate sentences as JSON.
2. Claude splits each sentence into atomic claims and writes them to a file.
3. Re-run with `--claims <file>` to grade each claim against the source.

The [factcheck reference flow](../reference/cli.md) has the full sequence.
