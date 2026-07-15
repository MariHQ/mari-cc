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

Claude reads the surface and the relevant source files, so each page traces back to real code rather than guesswork.

## Build the documentation set

Use the focused commands directly as you work through the repository:

1. Survey the codebase.
2. Choose and scaffold a platform.
3. Design the information architecture on the Diátaxis frame (tutorial, how-to, reference, explanation).
4. Write every page, grounded in the code.
5. Add the community-health files. Keep the license verbatim and scaffold
   contributing, code-of-conduct, governance, and security files with
   `mari asset scaffold <type>`.
6. Validate with `mari check --strict`.
7. Keep it alive with the hook, discovered rules, and a continuous-integration gate.

The scaffold templates provide the required structure, but page authoring and
placeholder replacement stay with Claude. Put team-specific versions in
`.mari/templates/<type>.md`; Mari uses them for both scaffolding and checks.

## Validate

`mari check` validates the whole project in one pass: internal links and anchors resolve, the nav agrees with the files on disk, and community-health files exist and are structurally complete:

```sh
mari check --strict
```

Use `mari surface` to compare the finished pages with the repository's public symbols and configuration keys. See [Keep docs and code in sync](keep-docs-fresh.md) for the maintenance loop that keeps the site current.
