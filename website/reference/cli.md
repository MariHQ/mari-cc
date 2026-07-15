# CLI command reference

## Setup and status

| Command | Purpose |
|---|---|
| `mari init [style\|all]` | Set up editorial project files and hooks. |
| `mari status` | Show the repository, config, detector counts, style, and hook state. |
| `mari config [get\|set\|list]` | Inspect or change repository configuration. |
| `mari features [--json]` | Print the capability catalog. |
| `mari doctor` | Check the compiled detector and grammar tools. |
| `mari console [--port N] [--open]` | Open the local web console. |

## Prose quality

| Command | Purpose |
|---|---|
| `mari detect <path...>` | Report deterministic prose findings. Supports `--json`, `--summary`, `--score`, `--strict`, `--quiet`, `--style`, `--grammar`, `--stdin`, `--strings`, and `--labels`. |
| `mari audit <path...>` | Print a human-facing detector report. |
| `mari narrative <questions\|score>` | Review whole-document narrative shape. |
| `mari glossary <harvest\|list\|add>` | Maintain the terminology table in `STYLE.md`. |
| `mari ignores ...` | Manage detector waivers. |
| `mari zero ...` | Manage zero-tolerance rules. |

## Documentation maintenance

| Command | Purpose |
|---|---|
| `mari rules ...` | Manage edit-notify rules. |
| `mari nudge ...` | Manage directed edit obligations. |
| `mari hook run` | Run the post-edit checks. |
| `mari i18n ...` / `mari localize ...` | Find translations and compare structure. |
| `mari surface [dir]` | Extract public symbols, headings, and config keys. |
| `mari asset ...` | Detect, check, or scaffold document archetypes. |
| `mari platform ...` | Detect or scaffold a documentation platform. |
| `mari docsite <plan\|status>` | Plan or inspect a repository documentation project. |
| `mari check [--strict] [--anchors]` | Validate links, navigation, community files, and assets. |

Use `mari <command> --help` for the complete flags and argument shapes.
