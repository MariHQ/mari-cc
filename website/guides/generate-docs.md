# Generate a documentation site

Mari can take a repository from no docs to a full documentation site. It scaffolds a platform, derives an information architecture from the code, writes and grounds every page, adds the community-health files, and validates the result. The deterministic parts live in the CLI. Claude does the survey, architecture, and writing in-session.

## Set up a platform

First check whether the repo already has a docs generator:

```sh
mari platform detect
```

If none exists, compare the options and scaffold one. Mari scaffolds MkDocs, Docusaurus, Sphinx, Hugo, Jekyll, mdBook, Antora, and Docsify:

```sh
mari platform list
mari platform scaffold mkdocs --name "My Docs"
```

Scaffolding writes a minimal, valid site and refuses to overwrite an existing setup unless you pass `--force`.

## Ground pages in the code

`mari surface` extracts the public API surface: exported symbols, headings, config keys, and command-like spans, each with a file and line. This is the inventory every page documents against:

```sh
mari surface
```

Claude reads the surface, plus `mari explore` and the catalog, so each page traces back to real code rather than to guesswork.

## Run the docsite flow

`mari docsite plan` prints the seven phases, and `mari docsite status` inspects what the repo already has:

1. Survey the codebase.
2. Choose and scaffold a platform.
3. Design the information architecture on the Diátaxis frame (tutorial, how-to, reference, explanation).
4. Write every page, grounded in the code.
5. Add the community-health files. The license is copied verbatim, everything else is a template.
6. Validate with `mari check --strict`.
7. Keep it alive with the hook, discovered rules, and a continuous-integration gate.

The CLI never writes prose. Page authoring stays with Claude.

## Validate

`mari check` validates the whole project in one pass: internal links and anchors resolve, the nav agrees with the files on disk, and community-health files exist and are structurally complete:

```sh
mari check --strict
```

Add `--deep` for the opt-in attention passes: public symbols the docs never mention, and doc sentences anchored to no code. Cap the cost with `--limit N`. See [Keep docs and code in sync](keep-docs-fresh.md) for the maintenance loop that keeps the site current.
