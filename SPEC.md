# Mari — Product Specification (the "What")

This is the master behavioral specification for Mari, a local-first Claude Code plugin that lets teams curate, search, and share their product knowledge layer, and that enforces prose quality on everything Claude writes. It defines every command, subcommand, switch, configuration key, rule, and behavior — independent of implementation language, library, or cloud choices. A companion document (the "how") will map this spec onto concrete technology.

Everything here is harvested and unified from two prototypes: **bean** (knowledge connectors, hybrid search, team sync) and **mari-cli** (deterministic prose detector, editorial skill, hooks, grounding). Where the prototypes disagreed or left gaps, this document resolves them; §22 lists those resolutions explicitly.

---

## 1. Product overview

Mari answers "What should our AI know, trust, and reuse?" It has five pillars:

1. **Ingest & search** — connect the knowledge teams already use (Slack, GitHub, Google Drive, Jira, Confluence, Zendesk, Salesforce, HubSpot, Microsoft 365, Discord, git history, local files) and make it retrievable by Claude with local hybrid search.
2. **Curate** — tag knowledge as canonical, stale, deprecated, draft, internal, customer-facing, or needs-review; maintain a glossary and a facts ledger; audit the knowledge base.
3. **Improve AI-authored content** — an editorial vocabulary (`deslop`, `tighten`, `understate`, `clarify`, `critique`, `polish`, …) plus a deterministic ~170-rule detector for AI slop, clarity, house style, and inclusive language.
4. **Ground claims** — factcheck content against FACTS.md, source-of-truth files, and the knowledge base; catch contradictions and unsupported claims before publish.
5. **Keep it alive** — deterministic post-edit hooks, edit-notify rules, doc↔code lineage, localization staleness checks, and docsite generation/validation.

### 1.1 Design invariants

These are non-negotiable behaviors, carried over from the prototypes:

- **Local-first.** All indexing, embedding, and search run on the user's machine. No SaaS dependency, no external LLM calls from the CLI. Team sharing goes through infrastructure the team already controls (Git, Git LFS, S3).
- **Configuration is files, never environment variables.** No config env vars are read. (A small set of *capability toggles* for optional ML features are permitted; see §17.4.)
- **Credentials never enter the repo.** They live under the user's home Mari directory with restrictive permissions (dir `0700`, files `0600`).
- **Mari never auto-syncs.** Sync hits live services; it runs only when the user asks. Read commands warn when the index is stale (see `sync.stale_days`) so the assistant can nudge.
- **The detector reports; it never autofixes.** Rewriting is the agent's job, guided by findings. Findings are leads, not verdicts — Mari never claims text "was AI-written."
- **Deterministic before generative.** Every quality gate has a deterministic core that runs instantly and offline. Local ML models are additive; generative passes are opt-in.
- **Reversible-safe.** Scaffolds never overwrite without `--force`; licenses are copied verbatim, never generated; Mari never edits files outside the repo.
- **Hooks never break the turn.** A hook always exits 0 and emits nothing on internal failure.
- **The assistant drives the CLI; users never have to run anything.** Setup is assistant-guided with a privacy path (the user can run credential commands themselves so tokens never reach the assistant).

### 1.2 Positioning

- Glean is company-wide search; **Mari is project-level curated memory.**
- Vale/Grammarly lint text; **Mari is a design system for text** wired into the agent loop: detect → rewrite → verify, plus retrieval and grounding.
- Not an LLM wrapper: no additional AI spend, no duplicate model hosting.

---

## 2. Delivery surfaces

Mari ships as a **Claude Code plugin** containing:

1. **A CLI** (`mari`) — every capability is a CLI command; the plugin's skills drive it via `${CLAUDE_PLUGIN_ROOT}`. The CLI is also installable standalone.
2. **Skills** — one router skill (`/mari`) plus connector-setup skills (`/mari:connect-slack`, etc.). Skills are user-invocable and also trigger on natural language ("what did we decide about pricing?", "tighten this README").
3. **Pinned commands** — users can pin frequent verbs as standalone slash commands (`/search`, `/deslop`, `/factcheck`, `/tag`, `/sync`, …) via `mari pin`.
4. **Hooks** — a post-edit hook (`PostToolUse` on `Edit|Write|MultiEdit`) that lints edited prose, fires edit-notify rules, and raises lineage/localization impact notices. Provider adapters exist for Claude Code, Cursor, Codex, and Copilot (§15).

Skill/command routing rules are in §16.

---

## 3. Workspaces, files, and scopes

### 3.1 Directory layout

| Location | Purpose |
|---|---|
| `~/.mari/config.json` | Global (per-user) config. `mari config set` writes here. |
| `~/.mari/credentials/<provider>.json` | Credentials for globally-scoped connectors. Mode `0600`. |
| `~/.mari/scopes.json` | Per-source scope map `{source_key: "global"\|"local"}`. Default `local`. |
| `~/.mari/<repo-slug>-<hash8>/` | Personal workspace for one repo: tracked-ref config, personal settings, state DB, index catalog, local credentials. |
| `~/.mari/_global/` | Workspace for globally-scoped connectors (same shape as a repo workspace). |
| `~/.mari/plugins/` | Drop-in connector plugins (§6.14). |
| `~/.mari/skills/` | Vendored external skills (e.g. humanizer). |
| `<repo>/.mari/config.json` | Committed, team-shared config: tracked refs, detector settings, tags policy, edit-notify rules. Versioned with code. |
| `<repo>/.mari/config.local.json` | Personal, gitignored overrides (deep-merged over committed; `null` deletes a key). |
| `<repo>/.mari/catalog/` | (git cloud backend only) shared index catalog, data files on Git LFS. |
| `<repo>/.mari/knowledge/` | Gitignored markdown mirror of scanned external docs (§5.2.9). |
| `<repo>/PRODUCT.md` | Editorial context: audience, register, voice, banned words, reading-grade target. |
| `<repo>/STYLE.md` | House style: base guide, terminology table, formatting rules, forbidden phrasings, glossary. |
| `<repo>/FACTS.md` | Facts ledger: one fact per line, `- fact  (source)`. |

Workspace identity: `<repo-slug>-<first-8-hex-of-hash(abs-path)>`.

### 3.2 Scopes

Every connector is scoped `global` (one index shared across all repos, lives in `_global`) or `local` (per-repo). Defaults per source are listed in §6. Changing scope (`mari scope <source> <value>`):

1. Moves tracked-ref lists between the repo workspace and `_global`.
2. Moves the credential file.
3. Purges the old workspace's index rows and vectors for that source.
4. Updates `scopes.json` and prints a reminder to run `mari sync`.

Searches automatically union the repo workspace and `_global` whenever any connector is global; results dedupe by `(source, doc_id, chunk_id)`.

### 3.3 Config resolution

Effective config = deep-merge, later wins:

```
DEFAULTS → ~/.mari/config.json → <repo>/.mari/config.json → <repo>/.mari/config.local.json → personal workspace settings
```

List-valued tracked refs **union** across layers; scalars from more-personal layers win. `chunking` resolves as global `chunking` with `<source>.chunking` merged on top. `mari config set` coerces values to the type of the default at that dotted path (booleans accept `1/true/yes/on`).

---

## 4. Configuration schema

Complete key registry with defaults. All keys settable via `mari config set <dotted.path> <value>` and readable via `mari config get`.

### 4.1 Indexing & embedding

```
embedding.batch_size          = 64
embedding.plugin              = null      # path/module exposing embed(texts) [+ embed_query(q)];
                                          # null = built-in local model (implementation-chosen,
                                          # task-aware doc/query prompts, normalized vectors)
chunking.lines                = 40        # lines per window
chunking.overlap              = 8         # shared lines between windows
chunking.max_chars            = 2000
chunking.min_chars            = 40        # windows shorter than this are dropped
chunking.title_prefix         = true      # prepend doc title to EMBEDDED text only
chunking.large_chunks         = false     # coarse vector-only chunks
chunking.large_chunk_ratio    = 4         # base chunks joined per large chunk
```

Per-source chunking overrides (defaults ship for chat-like sources):

```
slack.chunking    = {lines:15, overlap:3, max_chars:1000, min_chars:20}
git.chunking      = {lines:15, overlap:3, max_chars:1000, min_chars:10}
```

Changing any `embedding.*` or `*.chunking.*` key prints a reminder to run `mari sync --rebuild`.

### 4.2 Search

```
search.hybrid          = true    # false = vector only
search.k               = 8       # default result count
search.rrf_k           = 60      # reciprocal-rank-fusion constant
search.keyword_pool    = 200     # candidate pool floor (actual pool = max(k*4, keyword_pool))
search.expand          = 1       # neighbor chunks per hit (only when merge_sections=false)
search.vector_weight   = 1.0
search.keyword_weight  = 1.0
search.auto_weight     = true    # query-type routing (§7.4)
search.recency_decay   = 0.0     # 0 = off; score *= max(1/(1+decay*age_years), recency_floor)
search.recency_floor   = 0.75
search.merge_sections  = true    # coalesce adjacent same-doc chunks into sections
search.rerank.enabled  = false
search.rerank.model    = <implementation-chosen cross-encoder id>
search.rerank.pool     = 40
search.tag_boosts      = {canonical: 1.15, draft: 0.9, stale: 0.7, deprecated: 0.5}   # §10.1
```

### 4.3 Sync, sources & OCR

```
sync.stale_days        = 7       # read commands warn when index older; 0 = never warn
slack.lookback_days    = 14      # first-sync backfill window (0 = all)
discord.lookback_days  = 14
gdocs.lookback_days    = 30
gdocs.comments         = true    # index Drive comments as separate docs
github.include         = ["issues","pulls"]
zendesk.brands         = []      # optional brand filter
ocr.backend            = "auto"  # auto | text | ocr-model  (§8.6)
ocr.model              = <implementation-chosen OCR/VLM id>
ocr.dpi                = 200
ocr.auto_install       = true    # provision OCR toolchain on first use
graph.enabled          = true    # deterministic edge graph (§8.4)
plugins.paths          = []      # extra connector-plugin directories
```

Tracked-ref lists (populated by `mari track`, may live in committed or personal config; see §6 per-source):

```
slack.channels, google.docs, google.folders, github.repos, git.repos,
confluence.spaces, confluence.pages, jira.projects, zendesk.include,
salesforce.objects, hubspot.include, microsoft.drives, microsoft.mail,
microsoft.teams, discord.channels, discord.guilds, localfiles.paths
```

Any source block also accepts a per-block `lookback_days` override (resolution: source block → `<key>.lookback_days` → built-in default).

### 4.4 Cloud sharing

```
cloud.enabled  = false
cloud.backend  = "s3"       # s3 | git
cloud.role     = "writer"   # writer | consumer
cloud.bucket   = ""
cloud.prefix   = ""
cloud.region   = ""
```

### 4.5 Detector & editorial

```
detector.styleGuide     = "microsoft"   # microsoft | google | ap | chicago | plain
detector.ignoreRules    = []            # rule ids waived project-wide
detector.ignoreFiles    = []            # globs (repo-relative path OR basename; **, *, ? supported)
detector.ignoreValues   = {}            # {ruleId: [exact values to waive]}
detector.ignoreReasons  = {}            # {ruleId|glob|value: "reason"}
detector.zeroTolerance  = []            # rule ids that fire on FIRST occurrence, bypassing density gates
detector.grammar        = false         # opt-in grammar pass (§11.7)
```

Waivers live **only** in config JSON — there are no inline in-file disable comments.

### 4.6 Hook

```
hook.enabled      = true
hook.quiet        = true    # suppress output when a file is clean
hook.maxFindings  = 10      # per-file cap in hook output
hook.grammar      = false
hook.lineage      = true    # lineage impact notices on/off
hook.knowledge    = true    # knowledge pending-impact notices on/off
```

### 4.7 Edit-notify rules

```
rules = [ {name, paths: [globs], notify: "message", exclude: [globs]} ]
```

When any edited file matches a rule's `paths` and none of `exclude`, the post-edit hook reminds the agent to do `notify`. Fires on **any** file type, not just markdown.

### 4.8 Curation

```
tags.statuses  = ["canonical","stale","deprecated","draft","internal","customer-facing","needs-review"]
tags.entries   = {}    # {path-or-doc-ref: {status, by, at, note}}  — committed config; team-shared
facts.file     = "FACTS.md"
glossary.file  = "STYLE.md"   # glossary terms live in STYLE.md's Terminology section
```

### 4.9 Scan / attention / associations

```
scan.google.docs        = []
scan.google.folders     = []
scan.slack.channels     = []
scan.slack.lookbackDays = 14
attn.model              = null   # path to local attention-capable model (enables --deep passes)
assoc.attn              = 0.5    # attention blend weight for assoc scoring
```

---

## 5. Command reference

Conventions for all commands:

- **Exit codes:** `0` success; `1` runtime/operation error or "no results"; `2` usage error / unknown argument. Detector-family commands: non-zero when any `error` finding exists; `--strict` also fails on `warn`.
- Long option forms `--opt value` and `--opt=value` both accepted.
- `--json` produces machine-readable output wherever listed.
- Mutating commands print `✓`/`✗` result lines; read commands print results or a "no matches — have you run mari sync?" nudge.
- Read commands (`search recent doc thread neighbors related sql explore`) auto-pull the cloud replica first when cloud-enabled (best-effort; on failure they warn to stderr and read the stale replica), and print a staleness warning to stderr when index age ≥ `sync.stale_days`.

### 5.1 Setup & lifecycle

#### `mari init [search|style|all]` (default `all`)
Interactive, assistant-guided setup.
- `search`: prints connection status for every source — `[x]/[ ]`, label, scope, connection state or the exact `mari auth <provider>` command, credential file path and required fields, config file path and list keys, whether it auto-indexes once connected, and current `lookback` where applicable. Ends with scope guidance and the three credential-handling paths (assistant runs it / user runs it / user writes the credential file).
- `style`: one-time editorial setup — ask register + base style guide, sample existing writing for voice, write `PRODUCT.md`, offer `STYLE.md`, offer hook install and `mari rules discover`.
- Exit 0.

#### `mari status`
Prints: workspace dir; cloud role/remote/last-pull (if cloud); embedding identity (warns on model mismatch → suggest `mari sync --rebuild`); last-sync age + staleness warning; per-source line `label scope connected|local tracked=N indexed=M`; detector style guide + hook state; tag counts by status.

#### `mari auth <provider> [--token T] [--url U] [--email E] [--subdomain S] [--key K] [--secret S] [--method M]`
Providers: `confluence discord github google hubspot jira microsoft salesforce slack zendesk`. (Auth provider `google` maps to source key `gdocs`.) Interactive providers (`google`, `microsoft`) with no flags run a browser/device-code flow; others validate the supplied credential against the service and save it to the source's scope location. Exit `0`/`1` (connect error)/`2` (unknown provider or missing required field).

#### `mari scope [source] [global|local]`
No args → list all sources and scopes. One arg → print that source's scope. Two args → change scope per §3.2.

#### `mari track <add|remove|list> [source] [ref…]`
The single routing command for tracked refs (replaces the prototypes' write-the-config-file-directly convention and the phantom `add` command).
- `add <source> <ref…>` — parse the ref (URL, `#channel`, `owner/repo`, `PROJ`, `source:kind:id`, path), normalize it, and append it to the right list in the right config layer (asks: personal or team-shared committed config).
- `remove <source> <ref>` — remove; next sync prunes everything under it.
- `list [source]` — show tracked refs per source and which config layer each came from.
Ref grammar per source is defined in §6.

#### `mari config [get PATH | set PATH VALUE | list] [--json]`
`get` prints the JSON value at a dotted path. `list` (or bare `mari config`) prints the whole resolved config, annotated with where each value can be set. `set` writes to global config with type coercion; prints a `--rebuild` reminder when the path touches `embedding.*` or `*.chunking.*`. Unknown path → prints all known dotted paths, exit 2.

#### `mari features [--json]`
Self-description catalog: every capability grouped by intent, with the command that provides it. (Used by the skill to answer "what can Mari do?")

#### `mari install [--providers=claude,cursor,codex,copilot] [--force]`
Wires post-edit hooks and installs the skill for each provider (§15). Default: Claude always, plus any provider whose config dir already exists. Idempotent; prunes stale prior hook entries.

#### `mari update`
Rebuilds installed skills and re-wires hooks from the current package. Idempotent.

#### `mari hooks <status|on|off|reset|ignore-rule <id>|ignore-file <glob>|ignore-value <rule> <value>> [--reason "…"]`
Hook management + hook-scoped waivers.

#### `mari ignores <list|add-rule <id>|add-file <glob>|add-value <rule> <value>> [--reason "…"]`
Detector waivers, written to committed `.mari/config.json`.

#### `mari zero <list|add <rule-id>|remove <rule-id>>`
Zero-tolerance list. A zero-tolerance rule fires on the first occurrence, bypassing density/co-occurrence gates. No-op for whole-document aggregate rules (`uniform-cadence`, `reading-grade`).

#### `mari rules <list|discover [--json] [--write]|add <name> --paths "<globs>" --notify "<msg>" [--exclude "<globs>"]|remove <name>>`
Edit-notify rules (§4.7). `discover` scans the repo for code↔docs couplings (API code ↔ API docs, config ↔ config reference, …) and proposes rules; `--write` saves them.

#### `mari pin <command>` / `mari unpin <command>`
Creates/removes `.claude/commands/<command>.md` so the verb is a standalone slash command. Pinnable set: `search sync audit deslop understate tighten clarify critique polish document draft outline glossary sharpen soften harden voice cadence format delight adapt localize live factcheck docsite tag extract`.

#### `mari plugins`
Lists core connectors (always available) and drop-in plugins loaded from plugin dirs.

### 5.2 Knowledge: sync & retrieval

#### `mari sync [source] [--rebuild] [--since N]`
Sync tracked sources into the index. Never runs automatically.
- `source` — restrict to one source key.
- `--rebuild` — full resweep: ignore cursors, re-fetch back `--since` days, re-embed every stored doc. Unsupported on a cloud consumer/cloud index (rebuild locally, then re-`cloud init`).
- `--since N` — reach-back days for rebuild/first-sync (default **90**).
Runs local-scoped sources into the repo workspace, global-scoped into `_global`. Per-doc progress to stderr. Summary: `✓ N document(s) updated, M removed — C chunk(s) embedded.` or `✓ knowledge base is up to date.` Git-backed cloud writer prints a "commit .mari" nudge. Exit 1 if any source errored (other sources still complete).

#### `mari search "question" [flags]`
Hybrid search (§7). Flags:
- `--full [N]` — print full bodies capped at N chars/hit (bare `--full` = 4000; `--full 0` = uncapped). Default off = 5-line × 110-char preview.
- `--variant "<q>"` — repeatable; extra query phrasings fused via weighted RRF (main query weight 1.0, each variant 0.7). The **agent** is the query-expansion step; Mari never calls an LLM for it.
- `--k N` — result count (default `search.k`).
- `--source <key>` — restrict to one source.
- `--doc <substr>` — restrict to docs whose id/title contains substring.
- `--author <substr>`, `--since YYYY-MM-DD`, `--before YYYY-MM-DD` — metadata filters.
- `--tag <status>` / `--no-tag <status>` — filter by curation tag (e.g. `--tag canonical`, `--no-tag deprecated`).
- `--expand N` — neighbor chunks per hit (only when `search.merge_sections=false`).
- `--json`.
Empty result → nudge + exit 1. Hits show curation tag badges when tagged.

#### `mari recent [--source] [--doc] [--author] [--since] [--before] [--limit N] [--full [N]]`
Most recently changed docs/messages, sorted by `COALESCE(modified_at, fetched_at) DESC`. `--limit` default 20.

#### `mari doc <ref> [--source S] [--full N]`
Full document body for up to 5 best id/title matches. `--full` default 0 (uncapped).

#### `mari thread <ref> [--source S] [--full N]`
Whole thread/conversation as one block (alias of `doc` for threaded sources).

#### `mari neighbors <chunk-id> [--radius N] [--full N]`
Chunks surrounding a chunk id in document order. `--radius` default 3.

#### `mari related <ref> [--source] [--limit N] [--full N]`
Docs one hop away in the edge graph (§8.4) from the best id/title match; each hit carries a `reason` (shared author / repo / project / channel / link). `--limit` default 20.

#### `mari sql "SELECT …" [--global]`
Read-only SQL over the catalog (`SELECT`/`WITH` only, else exit 2). No query → prints the schema doc. Tables: `documents`, `revisions`, `edges`, `tags`, `state`, `_chunks`. ASCII table output, cells truncated to 80 chars, `N row(s)` footer.

#### `mari scan <sub>` — lightweight external-doc mirror
For teams that want gdocs/Slack snapshots in-repo (gitignored) rather than only in the index:
- `auth google [--method gcloud|oauth] [--credentials <file>]`, `auth slack [--token xoxp-…]`
- `add <gdoc-url|folder-url|#channel>` / `remove <item>`
- `sync [google|slack] [--full] [--since N] [--json]` — snapshot to `.mari/knowledge/gdocs/<slug>--<id>.md` and `.mari/knowledge/slack/<channel>/<week>.md`, re-embed, then run lineage impact.
- `status`.
Slack lookback default 14 days; `--since` default 90.

#### `mari cloud <init|connect|role> …` and `mari pull`
See §9.

### 5.3 Curation

#### `mari tag <path-or-ref> <status> [--note "…"] | mari tag list [--status S] [--json] | mari tag remove <path-or-ref>`
Tag a repo file or an indexed doc ref with one status from `tags.statuses` (`canonical stale deprecated draft internal customer-facing needs-review`). Tags are stored in committed `.mari/config.json` (`tags.entries`) so they are team-shared and versioned, and mirrored into the catalog `tags` table at sync/search time. Effects:
- **Search ranking:** fused scores multiply by `search.tag_boosts` (canonical up-ranked; stale/deprecated down-ranked). `--tag`/`--no-tag` filters available on `search`/`recent`.
- **Result display:** tag badge shown on every hit; `deprecated` hits print their replacement pointer if a lineage edge exists.
- **Factcheck trust:** claims supported only by `stale`/`deprecated` sources are reported as `unsupported-claim` with a "source is stale" note; `canonical` sources are preferred evidence.
- **Hooks:** editing a file tagged `deprecated` or `stale` produces an advisory notice; `needs-review` files are surfaced by `mari audit kb`.

#### `mari glossary [harvest|list|add <term> --use "<canonical>" --not "<variants,…>"]`
Manages the Terminology table in STYLE.md.
- `harvest` — agent-driven: mine canonical terms and observed variants from the repo + knowledge base, propose Use/Not rows.
- `list` — print current terms.
- `add` — append a row.
Glossary rows feed the `terminology-consistency` detector rule (§11.3), so approved terms are enforced deterministically.

#### `mari facts <list|add "<fact>" [--source "<ref>"]>`
Manages `FACTS.md` (one fact per line: `- fact  (source)`). `mari extract` (below) is the bulk path.

#### `mari extract facts [--source <key>] [--doc <substr>] [--since D] [--json]`
Agent-assisted: pull candidate factual statements (numbers, dates, pricing, limits, launch claims) from recent knowledge-base content (e.g. `/mari extract facts from recent slack messages in #product`); the agent reviews and writes accepted ones to FACTS.md via `mari facts add`.

#### `mari audit kb [path…] [--json] [--strict]`
Knowledge-base audit: finds stale pages (no update past threshold), contradiction candidates (near-duplicate embeddings + NLI-contradiction when models available), missing links, duplicated content (embedding near-dupes), unsupported claims, inconsistent terminology, `needs-review` backlog, and content diverging from PRODUCT.md. Produces a prioritized report; does not edit.

### 5.4 Editorial: detector & rewriting

#### `mari detect <path|.> [--stdin] [flags]`
The deterministic detector. Reads markdown only (`.md .markdown .mdx .mdc`); non-markdown file args print a note and are skipped; no args → walk `.`.
- `--json` — findings + summary (+ score block with `--score`).
- `--summary` — worst files + rule histogram (for large trees).
- `--score` — 0–100 slop score with breakdown (§12).
- `--strict` — fail on `warn` too.
- `--quiet` — findings only, no banner.
- `--style=<microsoft|google|ap|chicago|plain>` — per-run pack override.
- `--models` — enable local ML tier (machine-likelihood, NLI; §17).
- `--slop-spans` — zero-shot slop-span extraction (requires `--models`).
- `--grammar` — opt-in grammar pass.
- `--no-config` — ignore project config.
Tree-walk skips: `node_modules .git dist build .next coverage .mari testdata test-data fixtures __fixtures__ golden snapshots __snapshots__ target out vendor vendored 3rdparty thirdparty third_party third-party`; also skips non-Latin/CJK prose, data-like files (few sentences, 2000+ char lines), generated files (CHANGELOG/HISTORY/LICENSE/NOTICE/llms.txt), and localized translation files. Code blocks, front matter (YAML/TOML), HTML comments, and template shortcodes are masked before rules run. Findings shape: `{ruleId, family, severity, offset, length, span, message, ref?}`.

#### `mari audit [path]`
Human-facing detector report grouped by family, each finding paired with a bad→good example fix. Report only; no edits.

#### Agent editorial verbs (run through the skill, backed by `mari detect` before/after)
Each verb has an authoritative reference flow the skill loads (§13). All preserve author meaning and voice; "rewrite, not delete"; each finishes by re-running the detector to verify no regression.

`deslop` (strip AI tells; `--narrative` adds discourse tier §13.3) · `understate` (cut over-explanation — the #1 durable tell) · `tighten` (concision) · `clarify` (jargon, acronyms, passive→active, error-message formula) · `sharpen` (cut hedges/weasels, commit to claims without inflating) · `soften` (superlatives→checkable facts) · `critique` (score 1–5 on argument/clarity/voice-fidelity/reader-experience; no rewrite) · `polish` (final pass: resolve critique + findings error→warn→advisory, align to STYLE.md, read aloud) · `voice` (inject brand voice from PRODUCT.md) · `cadence` (vary rhythm, thin tricolons) · `format` (headings, lists, emphasis, link text, backticks) · `delight` (restrained human touches) · `harden` (edge-case microcopy, error formula, i18n expansion budget ~30%) · `adapt` (rework for another channel) · `localize` (prep for translation + global English) · `draft` (outline→write→self-deslop→detect) · `outline` (annotated outline only) · `document` (infer STYLE.md from good existing writing) · `humanize` (apply vendored humanizer skill, then re-detect).

#### `mari live [<file>] [--n=K] [--stdin]`
Sentence iteration: prints a tighter deterministic variant (lexicon swaps) plus its findings; the agent supplies bolder/quieter variants in-session.

#### `mari humanize [ensure|update|status] [--json]`
Vendored external humanizer skill management: `ensure` clones on first use into `~/.mari/skills/humanizer` and prints the SKILL.md path; `update` fetches + hard-resets that checkout only; `status` prints revision.

#### `mari narrative <questions [--register prose|fiction] [--json] | score --answers <file> [--json]>`
Narrative-slop scoring (0–100, lower = more human) from a fixed research questionnaire (prose register = 15 items, fiction = 33). The CLI does arithmetic only; the agent answers the questionnaire. Human baseline ~30–35; the flow explicitly does not chase zero and never fabricates mess.

### 5.5 Grounding

#### `mari factcheck <file> [flags]`
Checks the file's claims against ground truth. Depths:
1. **Deterministic (default):** typed-span extraction (number, money, percent, year, date, entity) matched against `FACTS.md` (or `--source <file>` e.g. `--source PRODUCT.md`, or `--kb` to ground against canonical-tagged knowledge-base docs).
2. **`--models`:** adds local NLI entailment/contradiction.
3. **`--decompose` / `--claims <file>`:** atomic-claim grounding. `--emit-claim-targets` prints candidate sentences as JSON; the **agent** decomposes them into atomic claims in-session (the CLI never calls an LLM) and feeds them back via `--claims`.
4. **`--deep` / `--ground=attention` [--threshold t]:** on-device attention grounding of each sentence against the source (requires `--source` and a configured local model).
Other flags: `--json --strict --quiet --lookback`. Finding rules: `number-date-mismatch` (error), `contradicts-fact` (error), `unsupported-claim` (warn/advisory), `ungrounded-span` (advisory). Sources tagged `stale`/`deprecated` cannot *support* a claim (§5.3).

### 5.6 Documentation systems

#### `mari asset <detect <file> | check <file> [--strict] | scaffold <type> [title]>`
Document archetypes: `runbook adr postmortem rfc contributing code-of-conduct governance security` (canonical sections and rubrics in §14). `detect` infers the type; `check` validates required sections (`asset-missing-section`, plus `postmortem-blame` for blame language in postmortems); `scaffold` writes a template (never overwrites).

#### `mari platform <detect | list [--json] | scaffold <id> [--name "<title>"] [--force]>`
Doc-platform detection and scaffolding. Scaffoldable: `mkdocs docusaurus sphinx hugo jekyll mdbook antora docsify`. Detect-only: `vitepress starlight gitbook readthedocs`. Refuses to scaffold a second platform or overwrite without `--force`.

#### `mari check [--json] [--strict] [--deep [--limit N] [--threshold 0.3]]`
Whole-project docs validation: internal links + anchors resolve; nav↔files agree; community-health files present (README/LICENSE/CONTRIBUTING required; CODE_OF_CONDUCT/SECURITY/CHANGELOG recommended) and structurally valid. Rules: `link-broken`, `nav-missing-target`, `nav-orphan-page`, `community-missing-file`, plus asset rules. Respects `ignoreRules` but **not** `ignoreFiles` (structural defects can't be hidden by prose waivers). `--deep` adds attention passes over the public API surface: undocumented symbols and doc sentences anchored to nothing.

#### `mari surface [dir] [--json]`
Prints the extracted public API surface (exports/public symbols for the repo's languages) with `file:line`. Feeds `check --deep` and docsite grounding.

#### `docsite` (agent flow; entry `mari docsite` via pin or `/mari docsite`)
Seven phases: survey codebase → choose platform (`mari platform`) → design IA (Diátaxis) → write every page grounded in code (`mari surface`, `mari explore`) → community-health files (license copied verbatim, everything else templated with `<placeholders>`) → validate `mari check --strict` (+ `--deep`) → keep alive (hook + `rules discover` + CI gate).

### 5.7 Localization

#### `mari i18n <file>`
List a file's translations/source across supported localization layouts (suffix `README.es.md`; dir `docs/{en,fr}/`; Hugo `content.zh`; Docusaurus `i18n/<lang>/...`).

#### `mari i18n conform <file|dir> [--deep [--limit N]] [--strict]`
Check translations share the source's structure (headings, code blocks, links). Directory = one-pass sweep. `--deep` adds attention prose-coverage. Mari never translates — structural lockstep only.

#### `mari i18n coverage <source> [translation]`
Attention pass: flag source passages the translation barely covers.

The post-edit hook raises an i18n staleness note when a source-language file with siblings is edited (e.g. editing `docs/en/pricing.md` flags `docs/es/pricing.md`, `docs/fr/pricing.md`).

### 5.8 Context graph & exploration

#### `mari explore "<question>" | <file> [flags]`
RAG search over the current repo's own content (distinct from `search`, which queries connected sources). Flags: `--k N` (default 20), `--deep` (attention rerank), `--focus` (localize attention within top files), `--limit N`, `--threshold t`, `--knowledge` (search only the `.mari/knowledge/` mirror), `--json`, `--build`, `--keep-comments`. Auto-builds its index on first use and self-maintains from git diffs. A file argument explores from that file's mean-chunk embedding.

#### `mari assoc <build [--attn] | update | list [file] [--json] | check <file>>`
Derived semantic associations between repo files (embeddings + nearest-neighbor + optional attention blend). `update` is git-diff-driven incremental. Powers hook "related files" notices.

#### `mari lineage <sub>` — curated span↔span edges (the maintained context graph)
- `propose [--symbols|--assoc] [--min-score s]` — candidate edges from symbol mentions and the assoc index.
- `refine [--limit N] [--threshold t] [--id N…]` — attention pass shrinks coarse spans to precise ones.
- `review [--limit N] [--json]` / `show <id> [--json]` — inspect.
- `confirm <id…> [--rel r] [--note "…"] [--by llm|human]` / `reject <id…>`.
- `link <fileA:start-end> <fileB:start-end> [--rel r]` — manual edge.
- `impact [file…] [--json]` — edges whose endpoints drifted (content-hash change) since stamping.
- `stamp [file…|--all] [--id N…]` — mark current content as reconciled.
- `list [--status proposed|confirmed|rejected] [--file f] [--json]` / `stats`.
Relations: `documents implements describes duplicates derives-from related`. A confirmed edge is a maintenance promise: the post-edit hook notices drift on either endpoint. Deprecated docs should carry a `derives-from`/`documents` edge to their replacement (used by tag display, §5.3).

---

## 6. Connectors

### 6.0 Common contract

Each source defines: `key`, config block, label, tracked-ref list keys, auth provider (or none), scope default, sync function, and flags `interactive_auth` / `always_when_connected`. A source is **active** when it has tracked refs OR (`always_when_connected` AND connected). Registry order: 10 cloud connectors → `git` → discovered plugins → `localfiles` **last** (path catch-all).

Shared sync semantics:
- **Change detection:** per-doc revision signal (listed per source) decides *fetch*; a 16-hex content hash is the final authority for *re-embed* — a revision bump with identical text updates metadata only.
- **Resumable embedding:** docs whose `embedded_hash != hash` re-embed oldest-first; checkpoint per doc, so interrupted syncs resume cleanly.
- **Error tolerance:** one bad doc is logged and skipped; one source's failure never aborts others; a tracked-but-unconnected source (common from committed config) is a nudge, not an error.
- **HTTP:** retries 429 and ≥500 up to 4 attempts honoring `Retry-After` (else exponential backoff); 401 → one token-refresh attempt then auth error; 60s timeout.
- **Lookback:** chat-like sources backfill `lookback_days` on first sync (0 = all); `--rebuild` reaches `--since` days.
- **Pruning:** item-tracked sources prune docs that vanish or whose ref was untracked; incremental/whole-collection sources (Zendesk tickets, Salesforce, HubSpot, Microsoft mail/Teams) never prune.

### 6.1 Slack — `slack` · lists `channels` · auth `slack` · default scope **global** · always-when-connected
- **Credential:** User OAuth token `xoxp-…` (sees DMs + private channels) or Bot token `xoxb-…` (invited channels only). Scopes: `channels:history groups:history im:history mpim:history channels:read groups:read users:read`. Missing `groups:read` degrades to public channels (logged, not fatal). Stored: `{token, team, user, url}`.
- **Documents:** one per thread (root + replies), one per standalone message. `doc_id = <channel>/<root_ts>`; URL = permalink; author + created/modified (last activity).
- **Tracking:** default = all channels the token is a member of; explicit `channels` list (or `all`/`*`) narrows.
- **Incremental:** per-channel timestamp cursor + trailing 7-day re-scan window (catches edits/late replies). First sync backfills 14 days. User directory cached in state.

### 6.2 Google Drive — `gdocs` · config block `google` · lists `docs, folders` · auth `google` · interactive · default **global** · always-when-connected
- **Credential:** rides the user's gcloud session (browser sign-in with Drive access; per-sync short-lived access token, cached ~50 min). No OAuth client or GCP project required. Stored: `{method: gcloud, account}`.
- **Documents:** Google Docs exported as Markdown (fallback plain text); PDFs downloaded and text-extracted (§8.6). With nothing tracked, auto-indexes docs+PDFs the user owns; explicit `docs`/`folders` (Drive URLs; folders crawled recursively) narrow and disable auto-index.
- **Comments:** with `gdocs.comments=true`, each Drive comment (+replies) is a separate doc `<fileId>#comment:<id>`, author-attributed, mime `text/x-comment`.
- **Incremental:** per-file head-revision id; auto-mode discovery cursor on newest modified time; first sync 30-day lookback (0 = all); already-indexed files persist past the window; trash/access-loss evicts.

### 6.3 GitHub — `github` · lists `repos` · auth `github` · default **local**
- **Credential:** fine-grained PAT (`github_pat_…`; read: Contents, Issues, Pull requests, Metadata) or classic (`ghp_…`; `repo`/`public_repo`). Stored: `{token, login}`.
- **Documents:** issues + PRs (title, body, comments) of tracked repos. `github.include` narrows to `["issues"]`/`["pulls"]`. `doc_id = owner/repo#N`. No auto-index; must track ≥1 repo. No lookback.
- **Incremental:** per-repo `updated_at` high-water cursor; prunes untracked repos' docs.

### 6.4 Git history — `git` · lists `repos` · **no auth** · default **local** · always-when-connected
- Shells out to local `git log`. With nothing tracked, indexes the cwd repo; `repos` adds other clones. One document per commit; `doc_id = <repo>:<sha>`; URL derived from origin remote when GitHub/GitLab-shaped. Chat-sized chunking.
- **Incremental:** last-HEAD cursor, reads `last..HEAD`; rebase/force-push triggers full scan and prune of vanished commits.

### 6.5 Confluence — `confluence` · lists `spaces, pages` · auth `confluence` · default **local**
- **Credential:** Cloud = email + API token (Basic; URL includes `/wiki`); Server/DC = PAT (Bearer). Method inferred from presence of `--email`. Stored: `{method, url, email, token, name}`.
- **Documents:** every page, storage HTML flattened to text, `# title` prepended. Refs: page/space URL, `confluence:SPACEKEY`, `confluence:page:<id>`. Must track ≥1. `doc_id` = page id.
- **Incremental:** version number; list endpoint carries metadata, bodies fetched lazily for changed pages; prunes unseen pages.

### 6.6 Jira — `jira` · lists `projects` · auth `jira` · default **local**
- **Credential:** as Confluence (Cloud Basic / DC PAT), URL without trailing path.
- **Documents:** one per issue (summary + description + comments). Refs: `jira:PROJ` or `/browse/PROJ-123` URL. `doc_id` = issue key; author = reporter. Must track ≥1.
- **Incremental:** per-project `updated >` cursor; prunes untracked projects.

### 6.7 Zendesk — `zendesk` · lists `include` · auth `zendesk` · default **global** · always-when-connected
- **Credential:** subdomain + email + API token (Basic `email/token:token`). Stored: `{subdomain, email, token, name}`.
- **Documents:** tickets (subject + description + public/internal comments) and help-center articles (HTML→text). Both index once connected; `include` narrows to `zendesk:tickets`/`zendesk:articles`; optional `zendesk.brands` filter. `doc_id` = `ticket/<id>` / `article/<id>`.
- **Incremental:** tickets via incremental-export epoch cursor; articles paged in full; **never prunes**.

### 6.8 Salesforce — `salesforce` · lists `objects` · auth `salesforce` · default **global** · always-when-connected
- **Credential:** OAuth access token + instance URL (via Salesforce CLI, a Connected App, or an existing session). Tokens short-lived, not refreshed — re-auth on 401. Stored: `{token, url, name}`.
- **Documents:** Knowledge articles + Cases via SOQL. `objects` narrows to `salesforce:articles`/`salesforce:cases`. Whole-collection: never prunes; re-embeds when last-modified advances. `doc_id` = `article/<Id>` / `case/<Id>`.

### 6.9 HubSpot — `hubspot` · lists `include` · auth `hubspot` · default **global** · always-when-connected
- **Credential:** private-app token `pat-…` (Bearer; read scopes Tickets, Notes/engagements, Knowledge Base). Stored: `{token, portal_id}`.
- **Documents:** tickets, notes (HTML→text), KB articles (tolerated-if-absent). `include` narrows to `hubspot:tickets`/`hubspot:notes`/`hubspot:kb`. Whole-collection: never prunes. Cursor-paged; revision = `updatedAt`.

### 6.10 Microsoft 365 — `microsoft` · lists `drives, mail, teams` · auth `microsoft` · interactive · default **global**
- **Credential:** device-code flow against the public Azure CLI client (no app registration/admin consent; refresh token stored and rotated), or reuse an existing `az` session. Scopes: `offline_access Files.Read.All Mail.Read Chat.Read Sites.Read.All User.Read`.
- **Documents:** OneDrive/SharePoint files (office/pdf/html/text extraction; refs `me`, drive id, `ms:file:<itemId>`); Outlook mail — one doc per conversation (refs `ms:mail:<folder>`); Teams — one doc per message (refs `ms:teams:<teamId>/<channelId>`). Must track ≥1.
- **Incremental:** files by eTag/lastModified (files prune on delete); mail by newest received time; Teams messages carry no revision. Mail and Teams never prune.

### 6.11 Discord — `discord` · lists `channels, guilds` · auth `discord` · default **global**
- **Credential:** bot token; bot invited with View Channels + Read Message History and the **Message Content intent**. Stored: `{token, name, id}`.
- **Documents:** one per message in tracked channels (`discord:<channelId>` or URL) and all text channels of tracked guilds (`discord:guild:<id>`). Text channel types `{0,5,10,11,12}`. Must track ≥1. `doc_id = <channelName>/<messageId>`.
- **Incremental:** per-channel timestamp cursor, backward snowflake pagination; 14-day first-sync lookback.

### 6.12 Local files — `localfiles` · lists `paths` · no auth · default **local** · always last
- `paths` = files or folders (recursive; dotfiles/dot-dirs skipped). Formats: markdown/text (`.md .markdown .mdown .mkd .mkdn .mdx .txt .text .rst .org .adoc .asciidoc .asc .textile .tex .me`), HTML (`.html .htm .xhtml`), Office (`.docx .docm .odt .fodt .rtf .pptx .xlsx`), PDF. **Deliberately excludes logs and source code.**
- Change detection: mtime, content hash authoritative. Prunes vanished files. `doc_id` = absolute path; URL `file://…`.

### 6.13 Linear — `linear` · lists `teams, projects` · auth `linear` · default **local**
(Named in PRODUCT.md; not in the prototypes. Specified to the GitHub/Jira pattern.)
- **Credential:** personal API key. Stored: `{token, name}`.
- **Documents:** one per issue (title + description + comments). Refs: `linear:TEAM`, issue/project URL. Must track ≥1. Incremental: per-team `updatedAt` cursor; prunes untracked teams.

### 6.14 Connector plugins
Any plugin file in `~/.mari/plugins/` (plus `plugins.paths`) exporting a source definition (single `SOURCE`, list `SOURCES`, or a `register()` factory) is loaded at startup, appended after core connectors and before `localfiles`. A broken plugin is logged and skipped, never fatal. A documented connector template ships with the plugin.

---

## 7. Indexing & retrieval

### 7.1 Embedding
One built-in local embedding model (implementation-chosen; must run fully offline), task-aware (distinct document vs query encoding), normalized vectors. `embedding.plugin` swaps in any module exposing `embed(texts) -> vectors` (+ optional `embed_query`). The embedding identity (model id or `plugin:<ref>`) is recorded in state; `status` warns on mismatch with the index and recommends `mari sync --rebuild`. No silent fallback — embedding failure is loud.

### 7.2 Chunking
Fixed line windows: `lines` per window, `overlap` shared, step `max(1, lines−overlap)`; windows `< min_chars` dropped; each capped at `max_chars`. **Stable chunk ids** `<source>/<doc_id>#L<start>` (1-based) so unchanged docs re-embed nothing. `title_prefix` prepends the doc title to embedded text only (stored text stays raw). `large_chunks` joins every `large_chunk_ratio` base chunks into a coarse vector-only chunk (excluded from keyword and neighbor queries).

### 7.3 Hybrid retrieval
- **Vector:** cosine similarity over the chunk store; score `round(1 − distance, 3)`. ANN index built only past a row floor (~4096; partitions ≈ √rows capped 1024); brute-force below it. Scalar indexes on `source`/`doc_id`.
- **Keyword:** deterministic scoring directly over the same chunk store — count of distinct query terms present (tokens `[\w#/.-]{2,}`) plus a `+2` whole-phrase bonus. Excludes large chunks.
- **Fusion:** weighted reciprocal-rank fusion; each list contributes `weight/(rrf_k + rank)`. Main query weight 1.0, each `--variant` 0.7; vector/keyword lists weighted by config. Candidate pool `max(k*4, keyword_pool)`.

### 7.4 Auto weighting (query-type routing)
When `search.auto_weight`: identifier-like/quoted/short-numeric queries scale `vector×0.6, keyword×1.6`; natural-language questions (ends with `?`, or ≥5 tokens containing a question word) scale `vector×1.3, keyword×0.8`.

### 7.5 Post-fusion adjustments (applied in order)
1. **Filters:** source, doc-substring, author-substring, since/before on `modified_at` (accepted date forms: `YYYY-MM-DD`, ISO, `YYYY/MM/DD`), tag filters.
2. **Tag boosts:** multiply by `search.tag_boosts[status]` when the doc is tagged (§5.3).
3. **Recency:** if `recency_decay > 0`, multiply by `max(1/(1+decay*age_years), recency_floor)`; missing `modified_at` treated as ~0.25 years.
4. **Section merge** (`merge_sections`, default on): coalesce adjacent same-doc chunks into one section (line-range union, text from the doc body). When on, `--expand` is skipped.
5. **Rerank** (opt-in): local cross-encoder over the fused top-`pool` (default 40). Missing model → skipped, never fatal.
6. **Scope union & dedupe** across repo + `_global` workspaces.

### 7.6 Canned retrieval primitives
`recent` (newest first), `doc`/`thread` (full body, best id/title matches, limit 5), `neighbors` (± radius by chunk order), `related` (graph one-hop with reasons), `sql` (read-only).

---

## 8. Data model & storage

### 8.1 Catalog tables (shared, syncable)
- **documents**(source, doc_id, title, url, revision_id, hash, body, created_at, modified_at, author, mime, fetched_at) — upsert key `(source, doc_id)`. Timestamps are source-native.
- **revisions**(source, doc_id, revision_id, hash, fetched_at) — append-only history.
- **edges**(source, src_doc, rel, dst_kind, dst) — `rel ∈ {authored_by, in_repo, in_project, in_channel, links_to}`, `dst_kind ∈ {person, container, link, doc}`.
- **chunks**(id, source, doc_id, title, url, start, end, text, vector, ord) — vector width fixed by the embedding model at first write; `ord` = base-chunk position (null for large chunks).
- **tags**(ref, status, by, at, note) — mirror of `tags.entries` for query-time joins.

### 8.2 Private state (per workspace, never shared)
- **state**(key, value-json) — sync cursors and checkpoints: `last_sync`, `last_pull`, `embedding.model`, per-source cursors (`slack.cursor.<id>`, `github.since.<repo>`, `jira.since.<PROJ>`, `zendesk.tickets.start_time`, `git.head.<root>`, `gdocs.cursor`, `discord.cursor.<id>`, `localfiles.mtime.<path>`), cached user directories.
- **embedded**(source, doc_id, embedded_hash) — embed checkpoint; deliberately not in the shared catalog so migrated/pulled docs re-embed locally as needed.

### 8.3 Lineage store (per repo)
Edge table: id, endpoints (`file`, `start`, `end`, content-hash at stamp time ×2), `rel`, `status ∈ {proposed, confirmed, rejected}`, score, provenance (`--by llm|human`), note, timestamps.

### 8.4 Deterministic edge graph
Built at sync, no LLM: `authored_by → person(author)`; container edges from doc_id shape — GitHub/git `in_repo`, Jira `in_project`, Slack/Discord `in_channel`; markdown links → `links_to`. Powers `related` and the tag replacement pointer.

### 8.5 Content extraction
- **HTML:** flattened to markdown-lite (headings, bullets, links); script/style/head dropped.
- **Office:** docx/docm/odt/fodt/rtf/pptx (shapes + tables + speaker notes, per-slide headings)/xlsx (computed values, per-sheet). Legacy binary `.doc`/`.ppt` unsupported.
- **PDF (§8.6):** `ocr.backend = text` (embedded text only) | `auto` (embedded text; OCR only pages with <16 extractable chars) | `ocr-model` (every page through the configured local OCR/VLM). OCR toolchain auto-provisioned on first use unless `ocr.auto_install=false`; runs on GPU or CPU; render DPI configurable.

### 8.6 Concurrency & durability
Index writes are atomic upserts/appends with commit-conflict retry (up to 5 attempts). SQL surface is read-only. Legacy-format catalogs migrate idempotently behind a state flag.

---

## 9. Team sharing (cloud)

One authoritative shared catalog per repo; every machine keeps a full local replica; **reads always run on the replica**.

- `mari cloud init --backend git` — catalog lives at `<repo>/.mari/catalog`, data files on Git LFS (a `.gitattributes` is written). This machine becomes writer; teammates are read-only consumers via normal git pulls.
- `mari cloud init --bucket B [--prefix P] [--region R]` — S3-backed writer; pushes the local index up.
- `mari cloud connect --bucket B [...]` — read-only consumer; pulls down.
- `mari cloud role <writer|consumer>` — set this machine's role.
- `mari pull` — fetch latest cloud index into the replica (errors if not cloud-enabled); read commands also auto-pull, throttled to once per 60s.

**One-writer rule:** exactly one writer per shared catalog (index versions don't merge). `--rebuild` is unsupported against a cloud index — rebuild locally, then re-init. The git backend's sync summary nudges the writer to commit `.mari`. Alternatively teams skip cloud entirely and let each member sync from sources directly (config lists are shared via committed `.mari/config.json`; embeddings stay per-machine).

---

## 10. Curation model

### 10.1 Tag statuses and semantics

| Status | Meaning | Search | Factcheck | Hook |
|---|---|---|---|---|
| `canonical` | Source of truth | boost ×1.15 | preferred evidence | — |
| `draft` | Not yet trusted | ×0.9 | cannot support claims | — |
| `stale` | Known out of date | ×0.7 | cannot support; flagged | advisory on edit |
| `deprecated` | Superseded | ×0.5, shows replacement | contradiction candidate | advisory on edit |
| `internal` | Not customer-facing | badge only | — | warns if referenced from customer-facing docs |
| `customer-facing` | Published surface | badge only | held to `--strict` | stricter hook lint |
| `needs-review` | Flagged for a human | badge only | — | surfaced by `audit kb` |

Boost values are config (`search.tag_boosts`). Tags apply to repo paths and to indexed doc refs (`source:doc_id`).

### 10.2 Glossary
Approved terms live in STYLE.md's Terminology table (Use / Not columns). `mari glossary harvest` proposes rows from the repo + knowledge base; accepted rows are enforced by the `terminology-consistency` rule and loaded into the skill's editorial context.

### 10.3 Facts
FACTS.md is the deterministic grounding source: one fact per line with optional `(source)` attribution. Populated manually (`mari facts add`), or in bulk via `mari extract facts` (agent reviews before writing). `factcheck` treats FACTS.md as ground truth; contradictions are errors.

---

## 11. Detector rule registry

Rule shape: `{id, family, defaultSeverity, pack?}`. Families: **A** ai-slop · **B** clarity · **C** style · **D** inclusive · **grounding** · **grammar**. Severities: `error > warn > advisory`. Density-gated rules never fire on a single match unless the rule is in `zeroTolerance`. Pack-gated rules run only under the selected style guide. Code, front matter, comments, and shortcodes are masked before rules run. Severity caps are deliberate (e.g. `overused-word` never exceeds warn) so CI gates don't fail on style noise.

### 11.1 Family A — AI-slop tells

| Rule | Sev | Detects |
|---|---|---|
| `overused-word` | warn/advisory | Weighted density of measured LLM-overused vocabulary. Tier 1 (measured excess ratio as weight): delve 28, meticulous 34.7, intricate 11.2, commendable 9.8, underscore 13.8, showcase 10.7 (+inflections). Tier 2 (weight 4): realm, pivotal, garner, boasts, adept, groundbreaking. Heuristic (1.2–1.5): tapestry, testament, leverage, robust, seamless, nuanced, multifaceted, potential, elevate. Gate: ≥2 distinct slop words OR (≥2 hits AND ≥4/1k words). |
| `marketing-buzzword` | warn | ~33-entry list: streamline, empower, supercharge, world-class, enterprise-grade, cutting-edge, game-changing/-changer, next-generation/next-gen, best-in-class, turnkey, mission-critical, synergy, holistic, paradigm shift, frictionless, bleeding-edge, unparalleled, unrivaled, state-of-the-art, "unlock the full potential", "harness the power", … |
| `cliche-opener` | warn | Sentence-initial: "In today's fast-paced/modern/digital world/age", "In the ever-evolving/rapidly changing landscape of", "In the realm of", "When it comes to", "At its core", "In the world of". |
| `filler-phrase` | warn | "It's important to note that", "It's worth noting", "Needless to say", "At the end of the day", "That being said", "It should be noted that", … |
| `manufactured-contrast` | warn | "not just/only/merely/simply … it's/but/rather"; "not only … but also". The strongest AI cadence tell. |
| `conclusion-restate` | warn | Line-initial: In conclusion / In summary / To sum up / In essence / Overall / Ultimately / All in all. |
| `vague-attribution` | warn | studies show, research suggests, experts say/argue/believe, many believe, it is widely regarded/known, industry reports, some say, critics argue — suppressed when a citation/link is nearby. |
| `despite-challenges-closer` | warn | "despite its/these challenges … continues to thrive/evolve/grow". |
| `significance-boilerplate` | warn | stands as a testament, marking a pivotal moment, leaving an indelible mark, enduring legacy, key turning point, plays a vital/crucial/pivotal role, rich history/tapestry, navigating the complexities of. |
| `em-dash-overuse` | warn | Density > 4 per 1k words (human baseline ~3). Zero-tolerance = ban outright. |
| `semicolon-overuse` | advisory | > 5/1k; skips HTML entities and table lines. |
| `emoji-decoration` | warn | Emoji used as bullets/decoration. |
| `bold-lead-in-list` | warn | ≥3 consecutive `**Header**:` list items (AI listicle template). |
| `assistant-meta` | **error** | "As an AI language model", "as of my knowledge cutoff/last update", "I hope this helps", "Certainly!", "I'd be happy to", "Let me know if you", "Feel free to ask", "Here's a breakdown", `[insert …]`, `[Your Name]`, `[Your Company]`. |
| `sycophancy` | warn | Great question, You're absolutely right, That's a great point, Excellent question, What a fascinating. |
| `smart-quotes` | advisory | ≥3 curly quotes/apostrophes. |
| `unicode-artifact` | warn | Invisible Unicode (NBSP, zero-width, BOM) chatbot residue. |
| `hedge-overuse` | warn/advisory | Density of: it could be argued, arguably, to some extent, in many ways, generally/broadly speaking, tends to, somewhat, sort of, kind of, … |
| `negative-parallelism` | advisory | ≥2 of: ", not X." / "Not X. Not Y." / "X rather than Y" / "Rather, ". |
| `tricolon-overuse` | advisory | ≥3 "A, B, and C" constructions. |
| `serves-as-copula` | advisory | ≥2 of: serves/stands/acts/functions as, represents a, exemplifies, embodies. |
| `media-coverage-boilerplate` | advisory | featured in, profiled in, strong social media presence, garnered attention. |
| `future-outlook-speculation` | advisory | the future of, evolving landscape, continues to evolve, is poised to, on the horizon, only time will tell. |
| `conversational-scaffolding` | advisory | let's delve/dive/explore/unpack, deep dive, think of it as, imagine a world where, here's the kicker/thing, buckle up, spoiler alert, plot twist. |
| `superficial-ing-participle` | advisory | ≥2 clause-final: highlighting/underscoring/emphasizing/reflecting/showcasing/fostering/ensuring/paving the way. |
| `transition-scaffolding` | advisory | ≥2 paragraph-initial: Additionally/Moreover/Furthermore/However/Consequently/Nevertheless. |
| `interrogative-answer` | advisory | Rhetorical "The X? Answer." cadence. |
| `excessive-bold` | advisory | ≥4 bold spans and ≥3 per 100 words. |
| `listicle-reflex` | advisory | ≥5 list items with ≥50% at ≤4 words. |
| `uniform-cadence` | advisory | Whole-doc: sentence-length coefficient of variation < 0.25 over ≥6 sentences (model-free burstiness). |
| `emphasis-as-heading` | advisory | Bold-only line used as a fake heading. |
| `hype-intensifier` | advisory | greatly, vastly, hugely, immensely, enormously, tremendously, remarkably, crucial, pivotal, paramount, invaluable, "one of the most", "a great deal of". |

### 11.2 Family B — Clarity & concision

| Rule | Sev | Detects |
|---|---|---|
| `passive-voice` | advisory (warn with by-agent) | Passive constructions, with irregular-participle/adjective-participle exclusion sets to limit false positives. |
| `long-sentence` | warn | > 30 words. |
| `wordy-phrase` | warn | Map with replacements: in order to→to, due to the fact that→because, at this point in time→now, with regard to→about, has the ability to→can, a number of→some, … |
| `complex-word` | advisory | utilize→use, facilitate→help, commence→start, ascertain→find out, numerous→many, methodology→method, demonstrate→show, subsequently→later, terminate→end, … |
| `nominalization` | advisory | make a decision→decide, conduct an investigation→investigate, provide assistance→assist, take action→act, … |
| `weasel-word` | advisory | Density of: very, really, quite, fairly, rather, somewhat, just, basically, actually, simply, literally, extremely, incredibly, totally. |
| `redundant-pair` | warn | each and every, first and foremost, end result, free gift, past history, future plans, various different, absolutely essential, unexpected surprise, added bonus, new innovation, true fact, … |
| `repeated-word` | warn | Duplicated adjacent word (excluding legitimate "that that", "had had"). |
| `there-is-expletive` | advisory | "There is/are X that…", "It is X that…". |
| `adverb-overuse` | advisory | ≥5 -ly adverbs and ≥25/1k (with non-adverb -ly stoplist). |
| `undefined-acronym` | advisory | 3–5 capital acronym used without first-use expansion; large allowlist (API, URL, JSON, plus callout labels NOTE/TIP/…). |
| `reading-grade` | advisory (plain pack) | Whole-doc reading grade (Flesch-Kincaid + Coleman-Liau average) > 8 (or PRODUCT.md target). |

### 11.3 Family C — Style (shared, always on)

`sentence-case-heading` · `heading-end-punctuation` · `word-swap` (leverage→use, e.g.→for example, execute→run, login→sign in [verb], e-mail→email, check box→checkbox, drop-down→dropdown, …) · `serial-comma` (Oxford; self-suppresses under AP) · `intro-comma` (comma after conjunctive-adverb/subordinate-clause openers, with technical-noun exclusions) · `use-contractions` · `second-person` ("the user can" → "you") · `present-tense` ("you will X" → "you X") · `singular-they` · `no-please-instructions` · `terminology-consistency` (built-in variant groups — sign in/log in/login, email/e-mail, dropdown/drop-down, website/web site, checkbox/check box, filename/file name, setup/set-up, username/user name — **plus every STYLE.md glossary row**) · `acronym-case` · `acronym-plural` ("UDF's"→"UDFs") · `inconsistent-capitalization` · `fenced-code-language` · `duplicate-heading` · `markup-leak` ("#Heading" missing space) · `thematic-break-before-heading` · `bullet-overuse` · `double-space` · `redundant-acronym` (ATM machine, PIN number, LCD display, …) · `indefinite-article` (a/an by sound) · `placeholder-citation` · `tracking-param-in-citation` · `malformed-doi-isbn` · `unused-named-ref`.

### 11.4 Family C — Style packs (gated by `detector.styleGuide` / `--style`)

**Microsoft pack:** no-space-em-dash · no-internal-caps · omit-you-can · avoid-we · spell-out-small-numbers · no-numeral-sentence-start · large-number-grouping · no-k-m-b · leading-zero; plus Vale-parity ports: microsoft-ampm · microsoft-accessibility (defines-by-disability wordlist) · microsoft-adverbs (≥2) · microsoft-auto-hyphenation · microsoft-avoid-words · microsoft-contractions · ms-date-format · ms-numbers · ms-order · ms-ellipses · ms-first-person (≥2) · ms-foreign-abbrev (e.g./i.e./viz./ergo) · ms-gender-slash · ms-gender-bias (~33 gendered→neutral pairs) · microsoft-general-url ("URL"→"address" in user-facing text) · microsoft-heading-acronyms · microsoft-heading-colons · ms-adverb-hyphen · ms-negative-number-endash · ms-ordinal-ly · ms-percentages · ms-plurals-parenthetical ("(s)") · microsoft-quotes-punctuation · microsoft-range-time · microsoft-semicolon · ms-suspended-hyphen · ms-term-swaps (~40-term map) · ms-url-of · ms-units-spelled-number · ms-vocab-az-wordlist · ms-wordiness (~120-entry phrase→concise map).

**Google pack:** no-gerund-heading · no-link-in-heading · latinism-abbreviation · minimizing-words (easy, easily, simple, simply, just, quick, obviously, trivial, …) · no-abbreviation-as-verb · no-periods-in-acronyms · no-exclamation · american-spelling · no-preannounce (currently, latest, soon, upcoming) · no-directional (above→preceding, below→following); plus ports: google-ampm · google-contractions · google-date-format · google-ellipses · google-dash-spacing · google-first-person · google-gender-neutral-pronoun · google-gender-bias · google-ly-hyphen · google-optional-plurals · google-ordinal · google-quote-punctuation · google-number-range-words · google-semicolons · google-slang (tl;dr, ymmv, rtfm, imo, fwiw) · google-units-nbsp · avoid-first-person-plural · avoid-will-future-tense · google-word-list (~45-term map: dev key→API key, cellphone→phone, k8s→Kubernetes, wifi→Wi-Fi, …).

**AP pack:** ap-serial-comma (flags Oxford comma) · ap-number-style (spell 0–9) · ap-percent · ap-time-format · ap-dollar-style · ap-over-quantity ("more than" for quantities) · ap-toward · ap-ampersand.

**Chicago pack:** chicago-number-style (spell 0–100) · chicago-directional-s · chicago-percent-symbol · chicago-em-dash-spacing · chicago-ellipsis · chicago-united-states-noun · chicago-ibid.

**Plain pack:** plain-long-sentence (21–30-word band) · plain-hidden-verb · plain-shall (→must) · plain-required-to · plain-legalese-phrase · plain-legalese-word (herein, thereof, notwithstanding, …) · plain-double-negative · reading-grade (§11.2).

### 11.5 Family D — Inclusive & accessible

| Rule | Sev | Detects |
|---|---|---|
| `gendered-language` | warn | chairman→chair, mankind→humanity, manpower→workforce, salesman→salesperson, policeman→police officer, freshman→first-year student, … |
| `ableist-language` | warn / advisory | warn: crazy, insane, psycho, lame, dumb, tone-deaf, cripple; advisory: sanity check→consistency check, dummy value→placeholder value. |
| `vague-link-text` | warn (WCAG) | click here, here, read more, this, this link, link, more. |
| `skipped-heading` | warn / advisory | Heading level jumps; >1 h1 (advisory). |
| `person-first-language` | warn | suffers from→has, the disabled→disabled people, normal people→people without disabilities, … |
| `gendered-address` | warn | guys, gentlemen, ladies → everyone. |
| `tech-historical-terms` | warn / advisory | warn: blacklist→blocklist, whitelist→allowlist, master/slave→primary/replica, grandfathered→legacy; advisory (context-exempt): master, slave, native, primitive, tribe. |
| `violent-tech-metaphor` | advisory | abort→stop, kill→end, hang→stop responding, blast radius→scope of impact, DMZ→perimeter network ("hit" deliberately excluded). |
| `ageist-classist-cultural` | warn | ghetto, gypsy/gypped, oriental, eskimo→Inuit, third-world→developing, illegal immigrant→undocumented, sketchy, … |
| `missing-alt-text` | warn | Images without alt text. |
| `all-caps-shouting` | advisory | Prose in all caps. |
| `bare-url` | advisory | Naked URLs in prose. |

### 11.6 Grounding rules (emitted by `factcheck`, `check --deep`)
`number-date-mismatch` (error) · `contradicts-fact` (error) · `unsupported-claim` (warn/advisory) · `ungrounded-span` (advisory, attention-based).

### 11.7 Grammar rules (opt-in)
Local grammar engine pass, high-precision categories only: agreement, grammar, eggcorns, malapropisms, nonstandard usage, boundary errors, redundancy, miscellaneous. Emitted as `grammar-<kind>` (family `grammar`, warn) with top-3 suggestions. Noisy categories (mass nouns, missing-preposition) disabled.

### 11.8 Documentation-structure rules
`link-broken` · `nav-missing-target` · `nav-orphan-page` · `community-missing-file` · `asset-missing-section` · `postmortem-blame`.

### 11.9 Fixture discipline
Every rule ships a bad→good fixture pair; the test suite asserts each rule fires on its bad fixture and stays silent on its good one, plus regression checks for table-aware number rules, masking (front matter, comments, shortcodes), CJK/generated/vendored skipping, and large-repo false-positive budgets.

---

## 12. Slop score

`mari detect --score` computes a 0–100 score (higher = sloppier):

- Severity weights: error 3, warn 2, advisory 1.
- Family weights: ai-slop 1.0, grounding 1.0, inclusive 0.5, clarity 0.4, style 0.3.
- Weighted density per 1k words → saturating curve `100 × (1 − e^(−per1k/35))`.
- Minus a human-signal discount (contractions + first-person usage), capped at 15.
- With `--models`, a machine-likelihood estimate blends in at 20%.
- Bands: `clean` < 12 · `light` 12–29 · `moderate` 30–59 · `heavy` ≥ 60.

`mari narrative score` is the separate whole-document narrative metric (§5.4).

---

## 13. Editorial flows

### 13.1 Registers
Every editing task runs under a register (from task cues, target surface, or PRODUCT.md):

| Register | Bars |
|---|---|
| **docs** (default) | ~25-word sentence ceiling, second person, imperative mood. |
| **marketing** | ~30 words, specificity over superlatives. |
| **editorial** | Voice/POV allowed, rhythm variation expected. |
| **microcopy** | ~12 words, error formula (what happened / why / how to fix), never blame the user, i18n discipline (~30% expansion budget, variables out of grammar). |

### 13.2 Skill setup phase (before any editing verb)
1. Load editorial context (PRODUCT.md, STYLE.md, FACTS.md; if no PRODUCT.md → run `init style`).
2. Load the verb's reference flow.
3. Read a representative file for voice.
4. Resolve the register.
5. Run the detector on the target.
6. If the target is a recognized asset type, load its archetype.
Deterministic commands (`detect audit asset i18n platform check`) skip setup and run directly.

### 13.3 Narrative tier (`deslop --narrative`)
Seven whole-document dimensions, one pass each, in order: (1) stated morals, (2) tidy structure, (3) machine parallelism, (4) performed embodiment, (5) vague allusion, (6) no concession / no reader, (7) flat time. Scored via `mari narrative`. Register-gated: docs/microcopy apply only dimensions 1, 3, 5. Never fabricate mess; don't chase zero.

### 13.4 Universal guardrails
Detector findings are leads, not verdicts. Never claim text is AI-written. Preserve meaning and author voice. Keep human signals (sentence-initial And/But/So, deliberate fragments). Re-run the detector after editing; a verb that introduces new findings must fix them.

---

## 14. Asset archetypes

Each archetype defines canonical required sections, tone norms, and a 5-point review rubric:

| Type | Required sections | Basis |
|---|---|---|
| `runbook` | Overview, Prerequisites, Steps, Rollback, Escalation | incident-response "5 A's" |
| `adr` | Status, Context, Decision, Consequences | Nygard / MADR |
| `postmortem` | Summary, Impact, Timeline, Root Cause, Action Items, Lessons | Google SRE; blameless (`postmortem-blame` enforced) |
| `rfc` | Summary, Motivation, Alternatives, Drawbacks (+ rollout plan, open questions) | Rust RFC / Oxide RFD |
| `contributing` | standard contributor guide sections | community health |
| `code-of-conduct` | Contributor Covenant v2.1 | copied, attributed |
| `governance` | roles, decision process | CNCF / Apache patterns |
| `security` | reporting process, supported versions | GitHub SECURITY.md |

Teams can bring their own templates: a template dropped in `.mari/templates/<type>.md` overrides the built-in archetype for `scaffold` and `check`. When Claude drafts a document of a recognized type, the skill points it at the archetype (e.g. an RFC draft is checked for tradeoffs, alternatives, rollout plan, open questions).

---

## 15. Hooks

### 15.1 Post-edit hook
Registered per provider: Claude Code `PostToolUse` matcher `Edit|Write|MultiEdit` (timeout 10s); Cursor `afterFileEdit`; Codex `PostToolUse`; Copilot `postEdit`. Jobs, in order, per edited file:

1. **Prose lint** — run the detector on edited markdown (+ grammar if enabled). Output capped at `hook.maxFindings`; silent when clean and `hook.quiet`.
2. **i18n staleness** — if the edited file has translation siblings, note which localized files likely need updates.
3. **Edit-notify rules** — for any edited file matching a rule, emit its `notify` message (e.g. "API code changed — update docs/api/").
4. **Lineage impact** — if a confirmed lineage edge's endpoint drifted, emit a semantic-lineage notice (`⛓ …`) telling the agent which spans to reconcile.
5. **Association notice** — derived-assoc "related files" note (suppressed when a lineage notice already fired).
6. **Knowledge pending-impact** — note when scanned knowledge affecting this file changed.
7. **Tag advisories** — editing a `stale`/`deprecated`-tagged file, or referencing `internal` content from a `customer-facing` file (§10.1).

Invariants: always exit 0; emit nothing on internal failure; respect `hook.*` toggles; never modify files.

### 15.2 Commit association (git hook, optional)
An opt-in `post-commit` hook associates new commits with relevant knowledge (issues, conversations, docs) via the edge graph and embedding neighbors, and flags commits that touched code covered by an edit-notify rule without a matching doc change — "context is never lost."

---

## 16. Skill routing

The `/mari` router skill:

- **Bare `/mari <file>` or no-arg** → run detector, surface the top 2–3 recommended verbs; never auto-edit.
- **`/mari <known-subcommand> …`** → route to the command (init, sync, status, search, tag, config, features, docsite, …).
- **Natural-language question** → knowledge flow: compose a toolbox, not one search — `search` with agent-generated `--variant`s, then `doc`/`thread`/`related`/`recent`/`neighbors`/`sql` as needed. Extract identifiers from early hits and feed them back as variants. **Never conclude from a truncated preview** — use `--full`. Answer from the current index even when stale; suggest `/mari sync` but never run it unprompted.
- **Editing intent phrases** map to verbs: "make it punchier"→sharpen, "cut it down"→tighten, "make it less salesy"→soften, "sounds like AI"→deslop, "prepare for launch"→polish, etc.
- **Connector setup** → the relevant `connect-<source>` skill: scope question (with per-source default), method choice, click-by-click credential walkthrough, the three credential-handling paths, `mari auth` + `mari track add` + first `mari sync`, confirmation.
- **Guardrails:** setup is assistant-guided end-to-end; the user never has to run anything (but always may). Sync is the one command never run unprompted.

Connector-setup skills ship per source: `connect-slack connect-github connect-gdocs connect-confluence connect-jira connect-zendesk connect-salesforce connect-hubspot connect-microsoft connect-discord connect-linear`.

---

## 17. ML capability tiers

Detection and grounding are layered by model size, never "rules vs AI":

1. **Tier 0 — deterministic (always on):** the full rule registry, typed-span factcheck, structural checks. Instant, offline, dependency-free.
2. **Tier 1 — local small models (default-on once provisioned, `--no-models` to skip):** machine-likelihood (perplexity), NLI entailment/contradiction (factcheck + audit contradictions), zero-shot slop-span extraction (labels: marketing buzzword, hype phrase, vague corporate jargon, empty filler phrase, overused cliché), embeddings (search/explore/assoc). Models load lazily into a resident sidecar; only structured output crosses the boundary.
3. **Tier 2 — local attention/generative (opt-in via configured model):** attention grounding with three modes — **coverage** (context the query ignores: dropped translation content, stale docs↔code), **grounding** (query sentences that ignore context: fabricated/unsupported), **focus** (where attention mass lands). Powers every `--deep` flag and `lineage refine`. ~seconds per document.
4. **Agent tier:** anything requiring generation — query expansion, claim decomposition, rewriting, glossary harvest, narrative questionnaire — is done by Claude in-session. The CLI never calls an LLM.

Capability env toggles (the only permitted env vars): model paths/ids for the sidecar and attention binary, device selection, and feature switches equivalent to `--models`/`--slop-spans`.

---

## 18. Output & UX conventions

- Human output colorized on TTY, grouped by family/source; plain otherwise.
- `--json` everywhere data is consumed by the agent.
- `--summary` for large trees (worst files + rule histogram).
- Previews: 5 lines × 110 chars; `--full [N]` for bodies.
- Staleness and auto-pull warnings go to **stderr** so they never corrupt JSON output.
- All destructive-ish operations (`scaffold`, `install`, `cloud init`) are idempotent and refuse to overwrite without `--force`.

---

## 19. Testing & quality bars (behavioral requirements)

- Per-rule bad→good fixture pairs (~180 assertions) — every rule must fire on bad and stay silent on good.
- Integration/regression suite (~35 checks) including masking, skip-lists, localized-file handling, table-aware rules.
- Model tests run real local inference (no stubs).
- Large-repo hardening: false-positive budget validated against big real documentation trees (hundreds of files).
- A deliberate-slop self-test fixture (`mari detect fixtures/sloppy.md` must find a known finding set).

---

## 20. Non-goals

- No SaaS requirement; no server component in the core product (a hosted sync layer may exist later as an optional backend).
- No translation (i18n checks structure and coverage only).
- No source-code linting (prose in code strings is out of scope for v1; deliberately disabled in the prototype).
- No autofix by the detector; no editing external services' content.
- No PII redaction of indexed content in v1 (credentials protection only) — flagged as future work.
- No automatic sync, no background daemons, no cron in-core (users may wire their own cron/CI around `mari sync`).
- Legacy binary Office formats (`.doc`, `.ppt`) unsupported.

---

## 21. Glossary (of Mari itself)

- **Mari** (never "mari"/"MARI" in prose) — the product.
- **detector** (not "linter"/"scanner") — the deterministic rule engine.
- **finding** — one detector result (a lead, not a verdict).
- **register** — the writing context (docs/marketing/editorial/microcopy).
- **hook** — the post-edit integration.
- **AI tell / slop** — machine-flavored writing patterns.
- **source / connector** — an ingested knowledge system.
- **workspace** — per-repo personal state dir.
- **catalog** — the shared document/chunk index.
- **tag** — a curation status on a doc or file.
- **lineage edge** — a confirmed span↔span maintenance promise.

---

## 22. Resolutions of prototype inconsistencies

Decisions made in this spec where the prototypes disagreed or were incomplete:

1. **`add` command:** bean's skills referenced a nonexistent `bean add`; bean's router said "write the config file directly." This spec introduces `mari track add|remove|list` as the single real command (§5.1).
2. **Connector count:** manifests variously said 10/11/5-more. Canonical: **10 cloud connectors + git + localfiles + Linear = 13 sources**, 11 with auth.
3. **Naming:** all bean state moves from `~/.bean`/`.bean` to `~/.mari`/`.mari`; bean's `search`/`sync`/etc. and mari-cli's `detect`/etc. merge under one `mari` binary with no namespace collisions (verified: the command sets are disjoint).
4. **Inline waivers:** legacy `<!-- mari-disable -->` comments are dropped; waivers are config-JSON-only (`ignores`, `zero`, `hooks ignore-*`).
5. **Env vars:** bean's "no env vars ever" holds for configuration; mari-cli's ML capability toggles survive as the narrow exception (§17.4).
6. **Tags, glossary harvest, `extract facts`, `audit kb`, commit-association hook, Linear connector, per-team templates:** promised in PRODUCT.md but absent from both prototypes — specified here for the first time (§5.3, §10, §15.2, §6.13, §14).
7. **mari-cli `scan` vs bean connectors:** both survive with distinct jobs — connectors feed the searchable index; `scan` keeps an in-repo, gitignored markdown mirror for lineage/impact tracking (§5.2.9).
8. **Source-string linting:** built but disabled in the prototype; remains out of scope (§20).
9. **Default local models:** the prototype shipped them opt-in but the roadmap intended default-on; this spec adopts default-on-once-provisioned with `--no-models` (§17).
10. **Readability:** stays plain-pack-only (deliberately not core), honoring the prototype's design note.
