# Architecture

Mari has three small layers:

1. The Rust CLI reads repository files and applies deterministic detector rules, word lists, glossary terms, and structural documentation checks.
2. Repository configuration is resolved from `.mari/config.json` and `.mari/config.local.json`.
3. The bundled web console calls the same Rust functions through a localhost-only HTTP interface.

The Claude Code hook passes edited file paths to `mari hook run`. Mari reports concrete findings and edit obligations; Claude keeps the surrounding writing task in context and performs any requested revision.

The console bundle is built from `console/` and embedded in the binary with `include_dir`. It exposes detector rules, 49 word lists, glossary entries, templates, localization tools, nudges, and configuration.
