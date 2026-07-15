# Quickstart

Run setup from the repository root:

```sh
mari init style
```

Check a Markdown file:

```sh
mari detect README.md
```

For a grouped report with suggested corrections:

```sh
mari audit README.md
```

Open the local console to inspect the rule catalog, 49 word lists, glossary, nudges, and repository configuration:

```sh
mari console --open
```

In Claude Code, use `/deslop README.md`, `/tighten README.md`, or another focused editorial command to revise the file from the detector findings.
