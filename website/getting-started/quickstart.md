# Quickstart

This walkthrough takes about ten minutes. You index a local folder, search it, and run the prose detector. It assumes Mari is [installed](install.md) and on your `PATH`.

## 1. Set up the workspace

Run the guided setup from the root of your project:

```sh
mari init
```

`mari init` walks through two things: which knowledge sources to connect, and your editorial style (register and base style guide). Re-run it any time.

## 2. Track a source

Point Mari at a folder of local files. Markdown, text, HTML, Office documents, and PDFs are all supported. Mari deliberately skips source code and logs.

```sh
mari track localfiles add ./docs
```

## 3. Sync the index

Build the index for everything you tracked. The first sync downloads the embedding model (about 640 MB), which is a one-time cost.

```sh
mari sync
```

Mari prints per-document progress, then a summary line like `✓ 12 document(s) updated, 0 removed, 48 chunk(s) embedded.`

## 4. Search

Ask a question in natural language:

```sh
mari search "why did we change pricing tiers"
```

Each hit shows the source, the document, and a short preview. Add `--full` to print the full bodies, or `--source localfiles` to restrict the search to one source.

## 5. Detect prose problems

Run the deterministic detector on any Markdown file:

```sh
mari detect README.md
```

Mari groups findings into families (AI slop, clarity, style, inclusive) with a severity of error, warn, or advisory. It edits nothing. To get a human-facing report with a suggested fix for each finding, run `mari audit README.md` instead.

## 6. Factcheck a claim

Check a document's claims against a source of truth:

```sh
mari factcheck pricing.md --source PRODUCT.md
```

Mari extracts typed spans (numbers, dates, money, percentages) and flags any that conflict with the source.

## Next steps

- Connect a real source like Slack or GitHub: [Connect your sources](../guides/connect-sources.md).
- Clean up a draft: [Improve prose](../guides/improve-prose.md).
- Browse every command: [CLI reference](../reference/cli.md).
