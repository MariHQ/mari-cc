# CLI command reference

Every `mari` command, grouped by purpose. Conventions that apply throughout:

- **Exit codes.** `0` success, `1` runtime error or no results, `2` usage error or unknown argument. Detector-family commands exit non-zero when any `error` finding exists.
- Mutating commands print a `✓` or `✗` result line. Read commands print results or a "no matches, have you run mari sync?" nudge.
- Read commands warn to stderr when the index age passes `sync.stale_days`, and auto-pull the shared replica first when cloud is enabled.

## Setup and lifecycle

| Command | What it does |
|---------|--------------|
| `mari init [search\|style\|all]` | Guided setup. `search` prints connection status per source, `style` runs editorial setup. Default `all`. |
| `mari status` | Workspace, cloud role, embedding identity, last-sync age, per-source state, detector settings, tag counts. |
| `mari auth <provider> [flags]` | Save a credential for a source. Interactive for `google` and `microsoft`, token-based otherwise. |
| `mari scope [source] [global\|local]` | List or change a source's scope. |
| `mari config [get\|set\|list] [--json]` | Read or write config. `set` writes global config and warns when a change needs `--rebuild`. |
| `mari features [--json]` | The capability catalog, grouped by intent. |
| `mari doctor` | Report which optional tools and models are available. |
| `mari hooks <status\|on\|off\|reset\|ignore-...>` | Manage the post-edit hook and hook-level waivers. |
| `mari ignores <list\|add-rule\|add-file\|add-value>` | Detector waivers, written to committed config. |
| `mari zero <list\|add\|remove> <rule-id>` | Zero-tolerance rules that fire on first occurrence. |
| `mari rules <list\|discover\|add\|remove>` | Edit-notify rules. `discover` proposes code-to-docs couplings. |
| `mari nudge <list\|add\|remove\|check>` | Directed edit obligations scoped by glob or symbol. |

## Knowledge: sync and retrieval

| Command | What it does |
|---------|--------------|
| `mari track <source> <add\|remove\|list> [ref]` | Manage tracked refs. `--list-key` selects a specific list for multi-list sources. |
| `mari sync [source] [--rebuild] [--since N]` | Index tracked sources. `--rebuild` re-fetches and re-embeds everything. |
| `mari search "question" [flags]` | Hybrid search. Flags: `--full`, `--k`, `--source`, `--doc`, `--author`, `--since`, `--before`, `--tag`, `--no-tag`, `--variant`, `--json`. |
| `mari explore "<question-or-file>" [--k\|--deep\|--focus\|--json]` | Skill-facing explorer over a question or a file. |
| `mari surface [dir] [--json]` | Print the extracted public API and documentation surface. |
| `mari recent [filters] [--limit N]` | Most recently changed documents. |
| `mari doc <ref> [--source\|--full]` | Full body of the best-matching documents. |
| `mari thread <ref>` | A whole conversation as one block. |
| `mari neighbors <chunk-id> [--radius N]` | Chunks surrounding a chunk id, in order. |
| `mari related <ref> [--limit N]` | Documents one hop away in the graph, each with a reason. |
| `mari sql "SELECT ..." [--global]` | Read-only SQL over the catalog. |
| `mari cloud <init\|connect\|role>` | Manage the shared team replica. |

## Curation

| Command | What it does |
|---------|--------------|
| `mari tag <ref> <status>` / `tag list` / `tag remove <ref>` | Tag a file or document with one of the seven statuses. |
| `mari glossary <harvest\|list\|add>` | Manage the Terminology table in `STYLE.md`. |
| `mari facts <list\|add>` | Manage `FACTS.md`, one fact per line. |
| `mari extract facts [filters]` | Pull candidate facts from recent knowledge for review. |
| `mari audit kb [path...] [--strict]` | Audit the knowledge base for staleness, contradictions, and drift. |

## Editorial

| Command | What it does |
|---------|--------------|
| `mari detect <path> [flags]` | The deterministic detector. Flags: `--json`, `--summary`, `--score`, `--strict`, `--quiet`, `--style`, `--models`, `--slop-spans`, `--grammar`, `--stdin`, `--no-config`. |
| `mari audit [path]` | Human-facing detector report with a fix per finding. |
| `mari narrative <questions\|score <file>>` | The whole-document narrative metric. |
| `mari humanize <ensure\|update\|status>` | Manage the vendored humanizer skill. |

The editorial verbs (`deslop`, `tighten`, `clarify`, `sharpen`, `understate`, `soften`, `critique`, `polish`, `voice`, `cadence`, `format`, `delight`, `harden`, `adapt`, `localize`, `draft`, `outline`, `document`) run through the Claude Code skill, each backed by `mari detect` before and after. See [Improve prose](../guides/improve-prose.md).

## Grounding

| Command | What it does |
|---------|--------------|
| `mari factcheck <file> [flags]` | Check claims against `FACTS.md`, `--source <file>`, or `--kb`. Deeper passes: `--models`, `--decompose`, `--claims`, `--deep`. |

## Documentation systems

| Command | What it does |
|---------|--------------|
| `mari asset <detect\|check\|scaffold> <type>` | Document archetypes: runbook, adr, postmortem, rfc, contributing, code-of-conduct, governance, security. |
| `mari platform <detect\|list\|scaffold <id>>` | Detect or scaffold a docs platform. |
| `mari check [--strict] [--deep]` | Whole-project docs validation: links, nav, community files. |
| `mari docsite <plan\|status>` | Print the docsite phases or inspect what the repo has. |

## Localization

| Command | What it does |
|---------|--------------|
| `mari i18n <file>` | List a file's translations across supported layouts. |
| `mari i18n conform <file\|dir> [--deep]` | Check translations share the source's structure. |
| `mari i18n coverage <source> [translation]` | Attention pass for passages a translation barely covers. |

## Web console

| Command | What it does |
|---------|--------------|
| `mari console [--port <PORT>] [--open]` | Launch the local web dashboard over your knowledge base. Defaults to `http://127.0.0.1:4319/console`. `--open` opens your browser. |

The console is a browser interface to the same knowledge base the CLI reads. It serves entirely from the local binary, with no external service. See [Search and explore](../guides/search-knowledge.md#browse-in-the-console) for what it covers.
