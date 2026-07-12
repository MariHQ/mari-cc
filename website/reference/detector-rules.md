# Detector rules

The detector is deterministic. It reads Markdown, applies a large rule registry, and reports spans worth rewriting. It never claims a document "is AI-written." Every finding shows a rule id, a family, a severity, and the span it points at.

## Families

Findings are grouped into four families:

| Family | What it catches |
|--------|-----------------|
| AI slop | Overused vocabulary, marketing buzzwords, cliché openers, manufactured contrast, restated conclusions, vague attribution, em-dash overuse, and tricolon reflexes |
| Clarity | Long sentences, passive voice, nominalizations, expletive constructions, undefined acronyms, and repeated words |
| Style-guide conformance | The rules from your base guide (Microsoft, Google, AP, Chicago, or plain): sentence-case headings, contractions, serial commas, and word-list preferences |
| Inclusive and accessible | Non-inclusive terms, violent metaphors, and vague link text |

## Severities

Each finding carries one of three severities:

| Severity | Meaning |
|----------|---------|
| `error` | A defect. Detector-family commands exit non-zero when any error exists. |
| `warn` | Worth fixing. `--strict` treats warnings as failures too. |
| `advisory` | A judgment call. Resolve it or consciously leave it. |

## The slop score

`mari detect --score` computes a 0 to 100 score, where higher reads sloppier. The score is always explainable, so the number never stands in for a verdict. In short:

1. Each finding contributes weighted mass by severity and family, normalized per 1000 words.
2. A saturating curve maps that density to a base score that approaches but never reaches 100.
3. A human-signal discount subtracts for contractions and first-person voice.
4. With `--models`, a small machine-likelihood term blends in at 20 percent, so the model never dominates.

Bands: clean below 12, light 12 to 29, moderate 30 to 59, heavy 60 and up. The reported breakdown lists word count, findings by family, the per-1000 density, and the human signals.

## Waiving findings

Waivers live only in config JSON. There are no inline in-file disable comments.

```sh
mari ignores add-rule long-sentence          # silence a rule project-wide
mari ignores add-file "CHANGELOG.md"          # skip a file or glob
mari ignores add-value overused-word robust   # allow one term for one rule
```

For a rule your house style forbids outright, add it to the zero-tolerance list so it fires on the first occurrence rather than waiting for a density threshold:

```sh
mari zero add em-dash-overuse
```

Zero tolerance is a no-op for whole-document aggregate rules such as `uniform-cadence` and `reading-grade`.
