# Mari — Product Specification (the "What")

This is the master behavioral specification for Mari, a local-first Claude Code plugin. Mari lets teams curate, search, and share their product knowledge layer, and enforces prose quality on everything Claude writes. This document defines every command, subcommand, switch, configuration key, rule, and behavior — independent of implementation language, library, or cloud choices. A companion document (the "how") will map this spec onto concrete technology.

---

## 1. Product overview

Mari answers "What should our AI know, trust, and reuse?" It has five pillars:

1. **Ingest & search** — make the knowledge teams already use retrievable by Claude with local hybrid search via a rich context graph. Sources: Slack, GitHub, Granola, Google Drive, Jira, Confluence, Zendesk, Salesforce, HubSpot, Microsoft 365, Discord, git history, and local files.
2. **Curate** — tag knowledge as canonical, stale, deprecated, draft, internal, customer-facing, or needs-review; maintain a glossary and a facts ledger; audit the knowledge base.
3. **Improve AI-authored content** — an editorial vocabulary (`deslop`, `tighten`, `understate`, `clarify`, `critique`, `polish`, …) plus a deterministic ~170-rule detector for AI slop, clarity, house style, and inclusive language.
4. **Ground claims** — factcheck content against FACTS.md, source-of-truth files, and the knowledge base; catch contradictions and unsupported claims before publish.
5. **Keep it alive** — deterministic post-edit hooks, edit-notify rules, doc↔code lineage, localization staleness checks, and docsite generation/validation.

### 1.1 Design invariants

These are non-negotiable behaviors, carried over from the prototypes:

- **Local-first.** All indexing, embedding, and search run on the user's machine. No hard SaaS dependency, no external LLM calls from the CLI. Team sharing goes through infrastructure the team already controls (Git LFS, S3, Mari SaaS).
- **Configuration is files, never environment variables.** No config env vars are read. (A small set of *capability toggles* for optional ML features are permitted; see §17.4.)
- **Credentials never enter the repo.** They live under the user's home Mari directory with restrictive permissions (dir `0700`, files `0600`).
- **Hooks never break the turn.** A hook always exits 0 and emits nothing on internal failure.

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
| `~/.mari/skills/` | Vendored external skills (e.g. humanizer). |
| `<repo>/.mari/config.json` | Committed, team-shared config: tracked refs, detector settings, tags policy, edit-notify rules. Versioned with code. |
| `<repo>/.mari/config.local.json` | Personal, gitignored overrides (deep-merged over committed; `null` deletes a key). |
| `<repo>/.mari/catalog/` | (git cloud backend only) shared index catalog, data files on Git LFS. |
| `<repo>/PRODUCT.md` | Editorial context: audience, register, voice, banned words, reading-grade target. |
| `<repo>/STYLE.md` | House style: base guide, terminology table, formatting rules, forbidden phrasings, glossary. |
| `<repo>/FACTS.md` | Facts ledger: one fact per line, `- fact  (source)`. |

Workspace identity: `<repo-slug>-<first-8-hex-of-hash(abs-path)>`.

### 3.2 Scopes

Every connector is scoped `global` (one index shared across all repos, lives in `_global`) or `local` (per-repo). Defaults per source are listed in §6.

Searches automatically union the repo workspace and `_global` whenever any connector is global; results dedupe by `(source, doc_id, chunk_id)`.

### 3.3 Config resolution

Effective config = deep-merge, later wins:

```
DEFAULTS → ~/.mari/config.json → <repo>/.mari/config.json
```

List-valued tracked refs **union** across layers; scalars from more-personal layers win. `chunking` resolves as global `chunking` with `<source>.chunking` merged on top. `mari config set` coerces values to the type of the default at that dotted path (booleans accept `1/true/yes/on`).

---

## 4. Configuration schema

Complete key registry with defaults. All keys settable via `mari config set <dotted.path> <value>` and readable via `mari config get`.

### 4.1 Indexing & embedding

```
embedding.batch_size          = 64
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
slack.chunking    = {lines:5, overlap:3, max_chars:1000, min_chars:20}
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
```

Any source block also accepts a per-block `lookback_days` override (resolution: source block → `<key>.lookback_days` → built-in default).

### 4.4 Cloud sharing

```
cloud.enabled  = false
cloud.backend  = "s3"       # s3 | git
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
hook.maxFindings  = 20      # per-file cap in hook output
hook.grammar      = false
```

### 4.7 Edit-notify rules & nudges

```
rules  = [ {name, paths: [globs], notify: "message", exclude: [globs]} ]
nudges = [ {name,
            when:    {path: "<glob>", symbol: "<symbol>"?},     # trigger (source)
            edit:    [{path: "<file>", symbol: "<symbol>"?}],   # targets (sinks)
            message: "…"?,                                      # optional context for the agent
            exclude: [globs]?} ]
```

When any edited file matches a rule's `paths` and none of `exclude`, the post-edit hook reminds the agent to do `notify`.

A **nudge** is stronger: when an edited file matches `when` (and none of `exclude`), the hook directs the agent to **edit** each `edit` target now — a directed edit obligation, not just a reminder. The hook itself still never modifies files (§15.1 invariants); the agent makes the edits in-session.

**Span scoping via `symbol`.** Either side may name a symbol, written `path#symbol` on the CLI:
- in code files — an exported function/class/const name, resolved to its definition span with the same symbol extraction lineage proposals use (§8.3);
- in markdown — a heading, resolved to its section span (§11.0.4).

With `when.symbol` set, the nudge fires only when the edit intersects that span, not on any edit to the file. A `symbol` on an `edit` target scopes *what* to edit there ("update the `## Rate limits` section", not "touch the file somewhere"). Symbols re-resolve at hook time, so nudges survive file rewrites where line-based spans would drift; a symbol that no longer resolves falls back to whole-file matching with a warning.

A nudge is the hand-declared counterpart of a confirmed lineage edge (§8.3): the same span↔span maintenance promise, but stated by name/glob up front instead of curated from machine proposals, and matched by symbol rather than by line span + content hash. Both `rules` and `nudges` live in committed `.mari/config.json` — team-shared.

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
assoc.attn              = 0.5    # attention blend weight for assoc scoring
```

---

## 5. Command reference

Conventions for all commands:

- **Exit codes:** `0` success; `1` runtime/operation error or "no results"; `2` usage error / unknown argument. Detector-family commands: non-zero when any `error` finding exists.
- Mutating commands print `✓`/`✗` result lines; read commands print results or a "no matches — have you run mari sync?" nudge.
- Read commands (`search recent doc thread neighbors related sql`) auto-pull the cloud replica first when cloud-enabled; on failure they warn to stderr and read the stale replica. They also warn to stderr when index age ≥ `sync.stale_days`.

### 5.1 Setup & lifecycle

#### `mari init [search|style|all]` (default `all`)
Interactive, assistant-guided setup.
- `search`: prints connection status for every source. Per source: `[x]/[ ]`, label, scope, connection state or the exact `mari auth <provider>` command, credential file path and required fields, config file path and list keys, auto-index behavior, and current `lookback` where applicable. Ends with scope guidance and the three credential-handling paths (assistant runs it / user runs it / user writes the credential file).
- `style`: one-time editorial setup — ask register + base style guide, sample existing writing for voice, write `PRODUCT.md`, offer `STYLE.md`, offer hook install and `mari rules discover`.
- Exit 0.

#### `mari status`
Prints: workspace dir; cloud role/remote/last-pull (if cloud); embedding identity (warns on model mismatch → suggest `mari sync --rebuild`); last-sync age + staleness warning; per-source line `label scope connected|local tracked=N indexed=M`; detector style guide + hook state; tag counts by status.

#### `mari auth <provider> [--token T] [--url U] [--email E] [--subdomain S] [--key K] [--secret S] [--method M]`
Providers: `confluence discord github google hubspot jira microsoft salesforce slack zendesk`. (Auth provider `google` maps to source key `gdocs`.) Interactive providers (`google`, `microsoft`) with no flags run a browser/device-code flow; others validate the supplied credential against the service and save it to the source's scope location. Exit `0`/`1` (connect error)/`2` (unknown provider or missing required field).

#### `mari scope [source] [global|local]`
No args → list all sources and scopes. One arg → print that source's scope. Two args → change scope per §3.2.

#### `mari config [get PATH | set PATH VALUE | list] [--json]`
`get` prints the JSON value at a dotted path. `list` (or bare `mari config`) prints the whole resolved config, annotated with where each value can be set. `set` writes to global config with type coercion; prints a `--rebuild` reminder when the path touches `embedding.*` or `*.chunking.*`. Unknown path → prints all known dotted paths, exit 2.

#### `mari features [--json]`
Self-description catalog: every capability grouped by intent, with the command that provides it. (Used by the skill to answer "what can Mari do?")

#### `mari hooks <status|on|off|reset|ignore-rule <id>|ignore-file <glob>|ignore-value <rule> <value>> [--reason "…"]`
Hook management + hook-scoped waivers.

#### `mari ignores <list|add-rule <id>|add-file <glob>|add-value <rule> <value>> [--reason "…"]`
Detector waivers, written to committed `.mari/config.json`.

#### `mari zero <list|add <rule-id>|remove <rule-id>>`
Zero-tolerance list. A zero-tolerance rule fires on the first occurrence, bypassing density/co-occurrence gates. No-op for whole-document aggregate rules (`uniform-cadence`, `reading-grade`).

#### `mari rules <list|discover [--json] [--write]|add <name> --paths "<globs>" --notify "<msg>" [--exclude "<globs>"]|remove <name>>`
Edit-notify rules (§4.7). `discover` scans the repo for code↔docs couplings (API code ↔ API docs, config ↔ config reference, …) and proposes rules; `--write` saves them.

#### `mari nudge <list [--json]|add <name> --when "<glob>[#symbol]" --edit "<file>[#symbol]" [--edit "…"]… [--message "…"] [--exclude "<globs>"]|remove <name>|check [--json]>`
Nudges (§4.7): directed edit obligations — when a file matching `--when` is edited, the agent is told to edit every `--edit` target. `--edit` is repeatable (one nudge, many targets). `#symbol` scopes either side to a code symbol's definition span or a markdown heading's section. `add` validates that every named symbol resolves — unresolvable → `✗` + exit 1. `check` re-verifies all endpoints (files exist, symbols still resolve), for CI; exit 1 on any broken endpoint. Written to committed `.mari/config.json`.

### 5.2 Knowledge: sync & retrieval

#### `mari sync [source] [--rebuild] [--since N]`
Sync tracked sources into the index. The last sync time should be injected to remind the user to resync if too much time has gone by.
- `source` — restrict to one source key.
- `--rebuild` — full resweep: ignore cursors, re-fetch back `--since` days, re-embed every stored doc. Unsupported on a cloud consumer/cloud index (rebuild locally, then re-`cloud init`).
Runs local-scoped sources into the repo workspace, global-scoped into `_global`. Per-doc progress to stderr. Summary: `✓ N document(s) updated, M removed — C chunk(s) embedded.` Git-backed cloud writer prints a "commit .mari" nudge. Exit 1 if any source errored (other sources still complete).

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

#### `mari cloud <init|connect|role> …`
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
Knowledge-base audit. Finds: stale pages (no update past threshold), contradiction candidates (near-duplicate embeddings, plus NLI contradiction when models are available), missing links, duplicated content, unsupported claims, inconsistent terminology, the `needs-review` backlog, and content diverging from PRODUCT.md. Produces a prioritized report; does not edit.

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

#### `mari humanize [ensure|update|status] [--json]`
Vendored external humanizer skill management: `ensure` clones on first use into `~/.mari/skills/humanizer` and prints the SKILL.md path; `update` fetches + hard-resets that checkout only; `status` prints revision.

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

#### `docsite` (agent flow; entry `mari docsite` via pin or `/mari docsite`)
Seven phases: survey codebase → choose platform (`mari platform`) → design IA (Diátaxis) → write every page grounded in code (`mari surface`, `mari explore`) → community-health files (license copied verbatim, everything else templated with `<placeholders>`) → validate `mari check --strict` (+ `--deep`) → keep alive (hook + `rules discover` + CI gate).

### 5.7 Localization

#### `mari i18n <file>`
List a file's translations/source across supported localization layouts (suffix `README.es.md`; dir `docs/{en,fr}/`; Hugo `content.zh`; Docusaurus `i18n/<lang>/...`).

#### `mari i18n conform <file|dir> [--deep [--limit N]] [--strict]`
Check translations share the source's structure (headings, code blocks, links). Directory = one-pass sweep. `--deep` adds attention prose-coverage.

#### `mari i18n coverage <source> [translation]`
Attention pass: flag source passages the translation barely covers.

The post-edit hook raises an i18n staleness note when a source-language file with siblings is edited (e.g. editing `docs/en/pricing.md` flags `docs/es/pricing.md`, `docs/fr/pricing.md`).


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

---

## 7. Indexing & retrieval

### 7.1 Embedding
One built-in local embedding model (implementation-chosen; must run fully offline), task-aware (distinct document vs query encoding), normalized vectors. `status` warns on mismatch with the index and recommends `mari sync --rebuild`. No silent fallback — embedding failure is loud.

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

### 7.7 Rust implementation candidates

| Mechanism | Crate(s) |
|---|---|
| Embedding inference (local, task-aware) | `candle` or `ort` + `tokenizers`; `fastembed` as the batteries-included path |
| Vector store + ANN (IVF-PQ, scalar indexes) | `lancedb` / `lance` (native Rust) |
| Keyword scoring over chunks | SQL via `duckdb` (bundled), or `tantivy` if a dedicated inverted index is preferred over the count-based scorer |
| Cross-encoder rerank | `fastembed` (TextCrossEncoder) or `ort` |
| Connector HTTP (retry/backoff per §6.0) | `reqwest` + `tokio`; `backoff` for the retry policy |
| Git history connector | `git2`, or shell out to `git log -z` exactly as specified |
| Date parsing (`--since`/`--before`, cursors) | `chrono` |

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

*Rust:* the catalog maps to `lancedb` (merge_insert, delete-predicate, versioned commits — the conflict-retry semantics above are its native model); private state maps to `duckdb` (bundled) or `rusqlite`; the read-only `mari sql` surface registers the Lance datasets as DuckDB views via `duckdb`'s Arrow integration. Office/PDF/HTML extraction: `zip` + `quick-xml` for docx/pptx/xlsx/odt, `pdfium-render` or `lopdf`+`pdf-extract` for PDF text, `scraper` or `html2text` for HTML flattening; the OCR fallback runs through the Tier-2 model runtime (§17).

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

This section specifies the complete deterministic detector: the text-processing engine, every rule's exact mechanic (pattern, word list, gate, severity), and — where a Rust library can carry the mechanic — the crate to use. Word lists are normative: an implementation must match these lists exactly (they are the tested, calibrated sets from the prototype; every entry has a fixture).

Conventions used throughout:

- **Rule shape:** `{id, family, defaultSeverity, pack?}` with a `run(ctx, emit)` body. `emit` records `{ruleId, family, severity, offset, length, span, message, ref?}`. `span` is the matched source text capped at 80 chars, whitespace-collapsed.
- **Families:** `A` ai-slop · `B` clarity · `C` style · `D` inclusive · `grounding` · `grammar`. Severities: `error > warn > advisory`.
- **Offsets always refer to the original text**; rules scan the *masked* text (same length, code blanked), so a finding always points at the right source position.
- **Density gates:** a density-gated rule never fires on a single match. Zero tolerance (`detector.zeroTolerance`) bypasses the gate and fires per occurrence; it is a no-op for whole-document aggregate rules (`uniform-cadence`, `reading-grade`) — there is no single occurrence to flag.
- **Pack gating:** a rule with `pack` runs only when `detector.styleGuide`/`--style` selects that pack.
- **Severity caps are deliberate** (e.g. `overused-word` never exceeds warn): meta-documents about writing quote slop words densely, and style judgments must not fail CI.
- **Case-insensitive matching** unless a rule states otherwise.

### 11.0 Engine mechanics

#### 11.0.1 Pipeline

For each markdown file: read → file-level skip checks (§11.0.6) → build `ctx` via segmentation (§11.0.2–11.0.4) → run every active rule (always-on + selected pack) → apply waivers (`ignoreRules`, `ignoreFiles`, `ignoreValues`) → sort and render. The context object exposes: `text`, `masked`, `locate(offset)→{line,col}`, `blocks`, `sentences`, `wordCount`, `countWords(s)`, `headings`, `listItems`, `links`, `images`, `boldSpans`, `thematicBreaks`, `tableLines`, `isTableLine(offset)`, `refDefs`, `refUses`, `styleGuide`, `zeroTolerance`.

#### 11.0.2 Masking

Code and metadata are replaced with **spaces of equal length** (newlines preserved) so offsets survive. Blanked regions, in order:

1. Front matter at file start: YAML `--- … ---` or TOML `+++ … +++`.
2. Fenced code blocks: ``` ``` … ``` ``` and `~~~ … ~~~`.
3. Inline code: `` `…` `` (single line).
4. HTML comments `<!-- … -->` (license headers, notes — not prose).
5. Template shortcodes `{{ … }}` (Hugo/Liquid: `{{< ref >}}`, `{{% %}}`, `{{ .Var }}`).
6. Inline HTML tags `</?[a-zA-Z][^>]*>`.

Two rules (`passive-voice`, `indefinite-article`) additionally verify that the matched span is byte-identical in `text` and `masked` — a mismatch means the match spans a blanked inline-code hole ("is \`RocksDB\` based" → "is … based") and must be discarded.

*Rust:* `pulldown-cmark` yields byte ranges for code blocks/inline code/HTML, which map directly onto this blanking approach; the front-matter and shortcode patterns are plain regexes.

#### 11.0.3 Segmentation

- **Word counting:** tokens matching `[A-Za-z0-9]+(?:['’-][A-Za-z0-9]+)*`.
- **Blocks:** the masked text splits on blank lines; a heading line (`^\s{0,3}#{1,6}\s`) or list-item line (`^\s*([-*+]|\d+[.)])\s`) is its own block; consecutive plain lines merge into one paragraph block.
- **Sentences** (within non-heading blocks): terminator regex `[.!?]+["')\]”’]?(\s+|$)`, with two suppressions — a decimal point (digit before, `.digit` after) and a trailing abbreviation from the set: `mr mrs ms dr prof sr jr st vs etc inc ltd co no fig al eg ie e.g i.e u.s u.k a.m p.m approx`. Trailing text after the last terminator is a final sentence if non-blank.
- **Sentence-start test** (`isSentenceStart`): walk left over `[ \t>*_#-]` (blockquote/list/emphasis markers); the position starts a sentence if the preceding significant char is `.!?` or a newline, or start-of-file.

*Rust:* the splitter is small enough to port verbatim; `unicode-segmentation` (UAX-29) is available but is *not* a drop-in — the abbreviation and decimal suppressions above are the behavior contract.

#### 11.0.4 Markdown structure extraction

From the raw + masked line pair (a line fully blanked by masking is skipped):

- **Headings:** `^(\s{0,3})(#{1,6})\s+(.*?)\s*#*\s*$` → `{level, text, line, start, raw}`.
- **List items:** `^(\s*)([-*+]|\d+[.)])\s+(.*)$` → `{indent, marker, text, line, start}`.
- **Images** (parsed first so links can skip them): `!\[alt\](target …)`.
- **Links:** `\[text\](target …)` not preceded by `!`; scanned over masked text so code-span links don't count.
- **Bold spans:** `\*\*…\*\*` or `__…__` (single line).
- **Thematic breaks:** `^\s{0,3}([-*_])(\s*\1){2,}\s*$` tested on the masked line (so front-matter `---` doesn't count).
- **Table lines** (a set of line numbers): a line starting with `|`, a separator row `:?-{2,}:?(\|…)+`, or any line containing ≥2 pipes. `isTableLine(offset)` gates many rules — data cells aren't prose.
- **Reference definitions** `^\s{0,3}\[id\]:\s+\S+` and **uses** `][id]` plus shortcut `[id]` (not followed by `[`, `(`, `:`; not an image).

*Rust:* `pulldown-cmark` (or `comrak`) supplies all of these with source ranges; the table-line set and thematic-break checks are line regexes.

#### 11.0.5 Matching helpers

- `wordList(words)` → one alternation regex `\b(w1|w2|…)\b` case-insensitive, entries regex-escaped. *Rust:* for large lists use `aho-corasick` (with `MatchKind::LeftmostLongest` + ASCII case-insensitive) and verify word boundaries at match edges; `regex`'s alternation literal optimization also compiles these to Aho-Corasick internally, so a single `regex::RegexBuilder` with `case_insensitive(true)` is equally acceptable.
- `phraseList(phrases)` → alternation sorted **longest-first**, guarded by lookarounds instead of `\b`: `(?<![A-Za-z0-9_])(…)(?![A-Za-z0-9_])` — keys may end in punctuation (`e.g.`, `etc.`) where a trailing `\b` never matches. *Rust:* lookbehind/lookahead require `fancy-regex` (the `regex` crate has no lookarounds); alternatively match with `aho-corasick` leftmost-longest and check the neighbor bytes manually (faster, no backtracking).
- `scan(ctx, re, cb)` → iterate all matches over `ctx.masked`, advancing one char on zero-width matches.
- `emitAt` → builds the ≤80-char whitespace-collapsed span from the *original* text.
- `zeroTol(ctx, id)` → membership in the config zero-tolerance set.

Rules that use regex lookbehind (and therefore `fancy-regex` or manual neighbor checks in Rust): `em-dash-overuse` (`(?<=\s)--(?=\s)`), `semicolon-overuse` (HTML-entity lookbehinds), `spell-out-small-numbers` / `ap-number-style` / `chicago-number-style` (`(?<![\w.$%/-])`), `large-number-grouping`, `ms-negative-number-endash` (`(?<=\s)`), `no-abbreviation-as-verb` (`(?<!use )(?<!using )`), `indefinite-article` (`(?<![&\w.])`), `bare-url` (`(?<![("'<=\]])`), reference shortcut uses (`(?<!\!)`).

#### 11.0.6 File-level skip heuristics

Applied before segmentation (already listed under `mari detect`, restated as the engine contract):

- Extensions: only `.md .markdown .mdx .mdc`.
- Skip directories: `node_modules .git dist build .next coverage .mari testdata test-data fixtures __fixtures__ golden snapshots __snapshots__ target out vendor vendored 3rdparty thirdparty third_party third-party`.
- Skip generated files: `CHANGELOG`, `HISTORY`, `LICENSE`, `NOTICE`, `llms.txt`.
- **Non-Latin prose:** count Latin letters vs non-Latin script chars (CJK, Cyrillic, Arabic, Thai, Hangul ranges); skip when `nonLatin > 80 && nonLatin × 3 > latin` (≥25% of letters non-Latin — English rules are meaningless and half-translated docs would be pure noise).
- **Data-like files:** many words with almost no terminal punctuation, or lines ≥2000 chars.
- **Localized translation files** (per the i18n layout detection) are skipped — the source language is the lintable surface.

*Rust:* the `ignore` crate handles tree walking with the skip-dir set; `globset` implements `ignoreFiles` globs (repo-relative path OR basename, `**`/`*`/`?`). Parallelize per-file with `rayon`.

#### 11.0.7 Rust implementation candidates (engine-wide)

| Mechanism | Crate(s) |
|---|---|
| Plain regex rules (no lookaround) | `regex` |
| Lookbehind/lookaround rules (§11.0.5 list) | `fancy-regex` (or `aho-corasick` + manual edge checks) |
| Large word/phrase lists | `aho-corasick` (leftmost-longest, ASCII case-insensitive) |
| Markdown structure + masking ranges | `pulldown-cmark` (or `comrak`) |
| Tree walk + skip dirs | `ignore` |
| Waiver globs | `globset` |
| Per-file parallelism | `rayon` |
| Config (JSON, deep-merge) | `serde` / `serde_json` |
| CLI surface | `clap` |
| TTY color | `anstream` + `owo-colors` |
| Grammar pass | `harper-core` (Harper is native Rust; no WASM needed — §11.11) |
| Syllables/readability | port §11.12 verbatim (no crate dependency needed; `hyphenation` exists but changes the numbers) |
| NLI / embeddings / slop spans (ML tier) | `ort` (ONNX Runtime) or `candle`; `gline-rs` for GLiNER; `tokenizers` |
| Perplexity / attention (generative tier) | `llama-cpp-2` (llama.cpp bindings, GGUF models) |
| Date canonicalization (grounding) | plain code or `chrono` |

### 11.1 Family A — AI-slop tells

**`overused-word`** · warn/advisory · density + co-occurrence gated
Weighted word list; weights are measured LLM over-use ratios (Kobak 2025 / Liang 2024). Full map (word: weight; inflections share the base weight):

- Tier 1 (measured): `delve/delves/delving/delved` 28 · `meticulous/meticulously` 34.7 · `intricate/intricately` 11.2 · `commendable/commendably` 9.8 · `underscore/underscores/underscoring/underscored` 13.8 · `showcase/showcases/showcasing/showcased` 10.7
- Tier 2 (strong, unquantified, weight 4): `realm` · `pivotal` · `garner/garners/garnered` · `boasts/boast` · `adept` · `groundbreaking`
- Heuristic (low confidence): `tapestry` 1.5 · `testament` 1.5 · `leverage/leveraging` 1.5 · `robust` 1.5 · `seamless/seamlessly` 1.5 · `nuanced` 1.5 · `multifaceted` 1.5 · `potential` 1.2 · `elevate/elevates/elevating` 1.2 (active forms only — "elevated privileges" is legitimate)

Mechanics: collect all hits; `density = hits/words×1000`; `score = Σweights/words×1000`. **Gate:** ≥2 distinct slop words, OR (≥2 hits AND density ≥ 4/1k). **Severity:** warn when ≥3 distinct words or score ≥ 20, else advisory; never error.

**`marketing-buzzword`** · warn · fires per hit
Full list: `streamline, streamlines, streamlining, empower, empowers, empowering, supercharge, supercharges, world-class, enterprise-grade, cutting-edge, game-changing, game changer, game-changer, next-generation, next-gen, best-in-class, turnkey, mission-critical, synergy, synergies, holistic, paradigm shift, frictionless, bleeding-edge, unparalleled, unrivaled, state-of-the-art, unlock the full potential, unlocks the full potential, unlock the power, harness the power, harnessing the power`.

**`cliche-opener`** · warn · sentence-start only
Pattern (must pass `isSentenceStart`): `In today's (fast-paced|modern|digital) (world|age)` · `In the (ever-evolving|ever-changing|rapidly changing) (landscape|world) of` · `In the realm of` · `In the digital age` · `In an (era|age) of` · `When it comes to` · `At its core` · `In the world of`.

**`filler-phrase`** · warn
`It's important to note that` · `It is important to note` · `It's worth noting` · `It is worth noting` · `worth mentioning that` · `Needless to say` · `At the end of the day` · `That being said` · `It should be noted that` (apostrophes optional in the `It's` forms).

**`manufactured-contrast`** · warn · "the strongest AI cadence tell"
Two patterns, both confined to one sentence (no `.!?\n` inside the gap): `\bnot\s+(just|only|merely|simply)\b … \b(it's|but|rather|they're|we're)\b` and `\bnot only\b … \bbut( also)?\b`.

**`conclusion-restate`** · warn · line-start (blockquote `>` allowed)
Line-initial markers: `In conclusion` · `In summary` · `To sum up` · `In essence` · `Overall` · `Ultimately` · `All in all`.

**`vague-attribution`** · warn · suppressed near citations
Phrases: `studies show` · `research suggests` · `research shows` · `experts say|argue|believe` · `many believe` · `it is widely regarded|believed|known` · `industry reports` · `some say` · `critics argue`. Suppression: skip if the following 200 chars contain a markdown link `](`, `http(s)://`, a bracketed footnote `[1]`, or a caret footnote `^1`.

**`despite-challenges-closer`** · warn
One-sentence pattern: `despite (its|these|the|ongoing|numerous) … (challenges|difficulties|obstacles|setbacks) … (continues to|remains|still) (thrive|evolve|grow|serve|play|stand|endure)`.

**`significance-boilerplate`** · warn
`stands as a testament` · `marking a pivotal moment` · `leaving an indelible mark` · `enduring legacy` · `key turning point` · `plays a (vital|crucial|pivotal|key|significant) role` · `rich (history|tapestry|tradition)` · `navigat(e|ing) the (complexities|complex landscape) of`.

**`em-dash-overuse`** · warn · whole-doc density
Count `—` plus space-surrounded `--` (lookbehind/lookahead on whitespace). Gate: ≥3 dashes AND >4 per 1k words (human baseline ~3/1k); one finding at the first dash reporting count + rate. Zero tolerance: every dash flagged individually ("end the sentence, or use a comma or parentheses").

**`semicolon-overuse`** · advisory · whole-doc density
Count `;` excluding HTML entities (lookbehinds for `&name`, `&#nnn`, `&#xhh`) and table lines. Gate: ≥3 AND >5/1k; one finding at the first. Zero tolerance: each semicolon flagged at warn.

**`emoji-decoration`** · warn
Line-initial emoji, optionally after a bullet marker: `^\s*([-*+]\s*)?<emoji>` where emoji covers `☀-➿`, `⬀-⯿`, variation selector, and `U+1F000–U+1FAFF`.

**`bold-lead-in-list`** · warn
Over `ctx.listItems`: an item is *shaped* if its text matches `^\s*\*\*[^*]+\*\*\s*[:—-]`. Count maximal runs of shaped items on **consecutive lines**; a run of ≥3 emits one finding at the run head ("the AI listicle template").

**`assistant-meta`** · **error**
`As an AI language model` · `as of my (knowledge cutoff|last (update|training))` · `I hope this helps` · `Certainly!` · `I'd be happy to` · `Let me know if you` · `Feel free to (ask|reach)` · `Here's a breakdown` · `[insert …]` (not followed by `(`/`[`) · `[Your Name]` · `[Your Company]`.

**`sycophancy`** · warn
`Great question` · `You're absolutely right` · `That's a great point` · `Excellent question` · `What a fascinating`.

**`smart-quotes`** · advisory
Count `‘ ’ “ ”`; fire once at the first when ≥3 (or any, under zero tolerance).

**`unicode-artifact`** · warn · per char
Invisible characters: no-break space U+00A0, narrow no-break/thin space, zero-width space U+200B, zero-width non-joiner U+200C, zero-width joiner U+200D, BOM/zero-width no-break space U+FEFF. Message includes the codepoint.

**`hedge-overuse`** · warn/advisory · density-gated
Full list: `it could be argued, arguably, to some extent, in many ways, in some ways, more often than not, generally speaking, broadly speaking, in a sense, for all intents and purposes, tends to, somewhat, sort of, kind of`. Gate: ≥2 hits AND (≥3 hits OR ≥3/1k). Severity: warn when ≥4 hits, else advisory; every hit is emitted once the gate opens.

**`negative-parallelism`** · advisory · ≥2 across four patterns
`,\s+not\s+<2–30 chars>[.!?]` · `Not \w+. Not \w+` · `\w+ rather than \w+` · line-initial `Rather,\s`.

**`tricolon-overuse`** · advisory · ≥3
`\w+, \w+, and \w+`. The bar is ≥3 because the *reflex* is the tell — and a lower bar would fight `serial-comma`, which wants the Oxford comma this rule would then flag.

**`serves-as-copula`** · advisory · ≥2
`serves as, serve as, stands as, stand as, acts as, functions as, represents a, exemplifies, embodies` — ""is" often reads cleaner".

**`media-coverage-boilerplate`** · advisory · per hit
`featured in, profiled in, has been featured, and other prominent outlets, maintains a strong, a strong social media presence, an active digital presence, garnered attention`.

**`future-outlook-speculation`** · advisory · per hit
`the future of, evolving landscape, continues to evolve, is poised to, on the horizon, in the years to come, only time will tell, the road ahead`.

**`conversational-scaffolding`** · advisory · per hit
`let's delve into, let's break this down, let's dive in, let's explore, let's unpack, deep dive into, take a deep dive, think of it as, think of it like, imagine a world where, to put it simply, here's the kicker, here's the thing, buckle up, spoiler alert, plot twist`.

**`superficial-ing-participle`** · advisory · ≥2
Comma followed by a vague-significance participle: `, (highlighting|underscoring|emphasizing|reflecting|symbolizing|showcasing|fostering|ensuring|contributing to|paving the way)`. The finding anchors at the participle, not the comma (the separator may be comma+newline).

**`transition-scaffolding`** · advisory · ≥2
Line/paragraph-initial `Additionally|Moreover|Furthermore|However|Consequently|Nevertheless`.

**`interrogative-answer`** · advisory
Rhetorical-fragment cadence: `(^|[.!?]\s)((The|Its|Their|His|Her|Our)\s+\w+)\?\s+[A-Z]\w+\.` — "The answer? Simple."

**`excessive-bold`** · advisory · whole-doc
Fire once when bold spans ≥4 AND rate ≥3 per 100 words.

**`listicle-reflex`** · advisory · whole-doc
Fire once when list items ≥5 AND ≥50% of them are ≤4 words.

**`uniform-cadence`** · advisory · whole-doc aggregate (zero-tolerance no-op)
Per-sentence word counts (zeros dropped). Requires ≥6 sentences and mean ≥4 words. Coefficient of variation `CV = stddev/mean`; flag when `CV < 0.25`. Human engaging prose sits at CV ≈ 0.5–0.8+; this is the model-free burstiness check.

**`emphasis-as-heading`** · advisory
A whole line that is only a short bold phrase used as a fake header: `^[ \t]*(\*\*|__)(1–48 chars, not ending in [.:!?,;] or whitespace)\1[ \t]*$`, skipping table lines. A trailing colon means a label ("**Fields:**") and a period means emphasis — neither is a heading. Distinct from `bold-lead-in-list` (a run of list items).

**`hype-intensifier`** · advisory · per hit
`greatly, vastly, hugely, immensely, enormously, tremendously, remarkably, crucial, crucially, pivotal, paramount, invaluable, one of the most, a great deal of`.

### 11.2 Family B — Clarity & concision

**`passive-voice`** · advisory (warn with by-agent)
Pattern: auxiliary `am|is|are|was|were|be|been|being` + up to two `-ly` adverbs + a participle — either a regular `-ed`/`-en` form or one of the irregular participles:
`arisen awoken beaten begun broken brought built chosen done drawn driven eaten fallen forgotten frozen given gone grown hidden known made paid seen sold sent shown taken thrown told thought woven written found held kept led lost meant met put read run set`.
Exclusions, in order: (1) masked-adjacency check (§11.0.2); (2) the pseudo-participle stoplist (words ending -ed/-en that are not participles):
`even often seven open aspen been keen teen green screen then when hen pen ten amen omen alien barren brazen dozen garden golden heaven eleven hyphen kitchen linen listen oxygen siren sudden wooden woolen children happen chicken token red bed shed wed hundred indeed sacred naked wicked wretched crooked rugged ragged jagged hatred kindred`;
(3) predicate-adjective participles — skipped unless followed by `by`:
`interested located excited based related done born involved supposed used pleased concerned tired limited known given dedicated committed advanced detailed`;
(4) a following preposition `in|about|with|at|of|to|for` (unless `by`). A following ` by ` upgrades severity to warn.

**`long-sentence`** · warn — any sentence over **30 words**; message reports the count.

**`wordy-phrase`** · warn · map rule (phrase → replacement), full map:
`in order to→to · due to the fact that→because · at this point in time→now · at the present time→now · in the event that→if · in spite of the fact that→although · with regard to→about · with respect to→about · for the purpose of→to · has the ability to→can · have the ability to→can · a number of→some · a majority of→most · in the near future→soon · on a regular basis→regularly · in close proximity to→near · take into consideration→consider`.

**`complex-word`** · advisory · map rule, full map:
`utilize/utilizes/utilizing/utilization→use · facilitate/facilitates→help · commence/commences→start · endeavor→try · ascertain→find out · numerous→many · sufficient→enough · methodology→method · additional→more · approximately→about · demonstrate/demonstrates→show · individuals→people · subsequently→later · prior→before · initiate→start · terminate→end · component→part · functionality→features`.

**`nominalization`** · advisory · map rule, full map:
`make a decision→decide · made a decision→decided · conduct an investigation→investigate · provide assistance→assist · give consideration to→consider · reach a conclusion→conclude · perform an analysis→analyze · make an assumption→assume · come to an agreement→agree · take action→act · make a contribution→contribute · provide a description→describe · make an improvement→improve`.

**`weasel-word`** · advisory · density-gated
Full list: `very, really, quite, fairly, rather, somewhat, just, basically, actually, simply, literally, extremely, incredibly, totally`. Gate: ≥2 hits AND (≥3 hits OR ≥4/1k); all hits emitted once open.

**`redundant-pair`** · warn · per hit, full list:
`each and every, first and foremost, end result, free gift, past history, future plans, various different, absolutely essential, advance planning, close proximity, basic fundamentals, completely eliminate, final outcome, unexpected surprise, added bonus, new innovation, true fact`.

**`repeated-word`** · warn — `\b(\w+)\s+\1\b` case-insensitive, excluding the legitimate doublings `that that` and `had had`.

**`there-is-expletive`** · advisory · sentence-start only
`(There (is|are|was|were)|It is) <3–40 chars> (that|who|which)`.

**`adverb-overuse`** · advisory · whole-doc density
All `\w{3,}ly` tokens minus the non-adverb stoplist:
`only family reply apply supply july italy ally rely multiply early ugly holy likely lonely friendly daily weekly monthly yearly silly jelly belly fully`.
Gate: ≥5 hits AND ≥25/1k → one finding at the first hit. Zero tolerance: every `-ly` adverb flagged individually.

**`undefined-acronym`** · advisory · first occurrence per acronym
Token `[A-Z]{3,5}` (optional plural `s`), skipping: the allowlist below; a token followed by `.` (filename like `STYLE.md`); an acronym defined anywhere in the doc via parentheses (`ACR)` or `(ACR)`). Allowlist (full):
`API URL URI URN HTTP HTTPS JSON XML YAML TOML HTML CSS SQL DDL DML DOM ID UID UUID GUID UI UX CLI GUI OS RAM ROM CPU GPU SSD HDD VM JVM JDK JRE SDK PDF CSV TSV FAQ OK USA US UK EU UN AI ML NLP CI CD NPM CDN DNS IP TCP UDP SSH FTP SFTP TLS SSL REST SOAP RPC GRPC CRUD IDE JS TS MVP MVC TODO FIXME ASCII UTF UTF8 UTC GMT MIT BSD GPL LGPL ORM ENV PR QA RFC ABI ACID SaaS PaaS IaaS GB MB KB TB PB HZ KHZ MHZ GHZ FYI ETA AKA EOF EOL JAR WAR ZIP TAR GZIP POM POJO DTO DAO SPI JMX JDBC ODBC YARN HDFS S3 AWS GCP K8S ETL OLAP OLTP DAG AST LRU TTL QPS RPS SLA SLO IO NIO BIN LDAP SAML OAUTH JWT CORS XSS CSRF SHA MD5 RSA AES GZ EXE DLL JNI JIT GC OOM NPE WAL CDC NOTE TIP INFO WARNING IMPORTANT CAUTION DANGER ATTENTION HINT EXAMPLE SEE WARN ERROR DEBUG TRACE IDEA AND OR NOT NULL TRUE FALSE GET PUT POST HEAD CEP UDF UDTF UDAF KPI RocksDB FLIP JIRA`.

**`reading-grade`** · advisory · pack `plain` · whole-doc aggregate
Requires ≥30 words. Grade = mean of Flesch-Kincaid grade level and Coleman-Liau index (§11.12); flag when grade > 8 (or the PRODUCT.md target).

**`microsoft-adverbs`** · advisory · pack `microsoft` · ≥2 hits (family B)
The Vale Microsoft adverb list, matched whole-word; every hit emitted once ≥2 present ("Remove it if it's not important to the meaning"). Full list:
`abnormally absentmindedly accidentally adventurously anxiously arrogantly awkwardly bashfully beautifully bitterly bleakly blindly blissfully boastfully boldly bravely briefly brightly briskly broadly busily calmly carefully carelessly cautiously cheerfully cleverly closely coaxingly colorfully continually coolly courageously crossly cruelly curiously daintily dearly deceivingly deeply defiantly deliberately delightfully diligently dimly doubtfully dreamily easily effectively elegantly energetically enormously enthusiastically excitedly extremely fairly faithfully famously ferociously fervently fiercely fondly foolishly fortunately frankly frantically freely frenetically frightfully furiously generally generously gently gladly gleefully gracefully gratefully greatly greedily happily hastily healthily heavily helplessly honestly hopelessly hungrily innocently inquisitively intensely intently interestingly inwardly irritably jaggedly jealously jovially joyfully joyously jubilantly judgmentally justly keenly kiddingly kindheartedly knavishly knowingly knowledgeably lazily lightly limply lively loftily longingly loosely loudly lovingly loyally madly majestically meaningfully mechanically merrily miserably mockingly mortally mysteriously naturally nearly neatly nervously nicely noisily obediently obnoxiously oddly offensively optimistically overconfidently painfully partially patiently perfectly playfully politely poorly positively potentially powerfully promptly properly punctually quaintly queasily queerly questionably quickly quietly quirkily quite quizzically randomly rapidly rarely readily really reassuringly recklessly regularly reluctantly repeatedly reproachfully restfully righteously rightfully rigidly roughly rudely safely scarcely scarily searchingly sedately seemingly selfishly separately seriously shakily sharply sheepishly shrilly shyly silently sleepily slowly smoothly softly solemnly solidly speedily stealthily sternly strictly suddenly supposedly surprisingly suspiciously sweetly swiftly sympathetically tenderly tensely terribly thankfully thoroughly thoughtfully tightly tremendously triumphantly truthfully ultimately unabashedly unaccountably unbearably unethically unexpectedly unfortunately unimpressively unnaturally unnecessarily urgently usefully uselessly utterly vacantly vaguely vainly valiantly vastly verbally very viciously victoriously violently vivaciously voluntarily warmly weakly wearily wetly wholly wildly willfully wisely woefully wonderfully worriedly yawningly yearningly yieldingly youthfully zealously zestfully zestily`.

### 11.3 Family C — shared style rules (always on)

**`sentence-case-heading`** · advisory
For each heading, take the text before the first `:` or `—`; extract words; skip if <3 words. Count capitalized words (`[A-Z][a-z]…`) excluding the first word, all-caps acronyms, and the small-word set `a an the and or but for nor of to in on at by as is are with from into via per vs`. Flag when ≥2 are capped; the message shows the sentence-cased rewrite (first word and acronyms preserved).

**`heading-end-punctuation`** · warn — heading text ends with `.`, `:`, or `!`.

**`word-swap`** · advisory · map rule, full map:
`leverage→use · e.g.→for example · i.e.→that is · etc→and so on · execute→run · grayed out→unavailable · and/or→or · deselect→clear · login→sign in (verb) · log in→sign in · e-mail→email · check box→checkbox · drop-down→dropdown`.
(`abort` deliberately absent — `violent-tech-metaphor` covers it.) Pack precedence: under Microsoft, `e.g.`/`i.e.` are suppressed here (`ms-foreign-abbrev` owns them); under Google, `e.g.`/`i.e.`/`etc` (`latinism-abbreviation` owns them).

**`serial-comma`** · advisory
`\w+, \w+ (and|or) \w+` missing the Oxford comma. Skips sentence-initial matches (introductory adverbial, "Yesterday, John and Mary arrived" — not a list). Self-suppresses entirely under the AP pack (`ap-serial-comma` flags the opposite).

**`intro-comma`** · advisory
Two high-precision cases at sentence start (leading `>*_#-` markers stripped):
1. Conjunctive-adverb opener with no comma: `moreover furthermore nevertheless nonetheless consequently meanwhile additionally therefore conversely accordingly` followed directly by a word. (`However`, `Similarly`, `Subsequently` are deliberately excluded — "However you slice it", "Similarly designed systems" are premodifiers, not openers.)
2. Leading subordinate clause with no internal break: opener in `because although though if unless whereas whenever wherever while when once after before until since even though even if as long as as soon as`, sentence has no `,;:—` anywhere, ≥8 words, and the next word is **not** a tech noun (`loops?|statements?|blocks?|clauses?|conditions?|expressions?|keywords?|functions?|methods?|classes|hooks?|branches|cases?|comprehensions?` — "While loops are…" is a noun phrase). Introductory participial/infinitive phrases are deliberately out of scope (needs a parser to separate "To ship, we tested" from "To ship on time is hard").

**`use-contractions`** · advisory
The negation subset of the contraction map (keys containing `not`/`cannot`): `do not→don't · does not→doesn't · did not→didn't · is not→isn't · are not→aren't · was not→wasn't · were not→weren't · cannot→can't · can not→can't · will not→won't · would not→wouldn't · should not→shouldn't · could not→couldn't · have not→haven't · has not→hasn't`.

**`second-person`** · advisory — `(the user|users) (should|can|must|may|need to|needs to|will|might|have|has|access|get)` → "you …".

**`present-tense`** · advisory — `you will <verb>` → "you <verb>".

**`singular-they`** · warn · map rule:
`he or she→they · she or he→they · his or her→their · her or his→their · him or her→them · he/she→they · (s)he→they · s/he→they · his/her→their`.

**`no-please-instructions`** · advisory — any `please`.

**`terminology-consistency`** · advisory
Variant groups; flag (once per group, at the second variant found) when ≥2 distinct variants of one concept appear, located with a word-boundary regex (plain `indexOf` can land inside "screenlogin"). Built-in groups:
`[sign in | log in | login] · [email | e-mail] · [dropdown | drop-down] · [website | web site] · [checkbox | check box] · [filename | file name] · [setup | set-up] · [username | user name]` — **plus every STYLE.md glossary Use/Not row** (§10.2).

**`acronym-case`** · advisory
If a known acronym (the §11.2 allowlist) appears UPPERCASE in the doc, flag lowercase occurrences of the same token (`ddl` when `DDL` is present), once per token. Stoplist of allowlist entries that are also English words/SQL keywords/callout labels (never flagged):
`note tip info warning important caution danger attention hint example see warn error debug trace idea and or not null true false get put post head new all desc asc ok us jar war zip tar bin pr ram`.

**`acronym-plural`** · advisory — `([A-Z]{2,5})'s` → "use `…s` for the plural; keep `'s` only for the possessive".

**`inconsistent-capitalization`** · advisory
Multi-word Title-Case phrases (`[A-Z][a-z]+( [A-Z][a-z]+)+`) that also appear fully lowercase elsewhere. Leading sentence-initial stopwords are shed first (full stoplist: `the a an this that these those it he she they we you i if when while for and but or not as at by in on to of is are was were be note tip see use run add get set so such each any all`); requires ≥2 remaining words (single capitalized words carry a real proper-vs-generic distinction and are too noisy); skips headings and table lines; one finding per phrase.

**`fenced-code-language`** · advisory — an *opening* fence line ```` ``` ```` with no language token (fences alternate open/close; only openers flag). Runs on raw text (fences are masked).

**`duplicate-heading`** · advisory — same heading text (case-insensitive) used more than once; flags the repeats.

**`markup-leak`** · advisory — `^#{1,6}` immediately followed by a non-space non-`#` char ("#Heading").

**`thematic-break-before-heading`** · advisory — a `---`/`***`/`___` break whose next non-blank line is a heading ("an AI scaffold; remove it").

**`bullet-overuse`** · advisory · whole-doc — fire once when list items ≥8 AND ≥50% of non-blank lines are list items.

**`double-space`** · advisory — two spaces between word characters (`([^\s.!?:;])(  )(\S)` — sentence-spacing after punctuation is allowed), skipping table lines.

**`redundant-acronym`** · warn · per hit, full list:
`ATM machine, PIN number, LCD display, HIV virus, RAM memory, PDF format, ISBN number, GPS system, CPU unit, UPC code, NIC card, please RSVP, HTTP protocol, IP protocol, SIN number, VIN number`.

**`indefinite-article`** · advisory
`(a|an) <word>` with sound-based exception lists. Skips: matches adjacent to `&`/`.` (abbreviations like D&A); masked-adjacency check (§11.0.2). Exception lists — words needing `an` despite a consonant letter: `hour, honest, honor, heir, honour`; words needing `a` despite a vowel letter: `university, unicorn, unique, unit, user, used, useful, european, one, once, ubiquitous, url, ui, utility, eulogy`. Four branches: `a`+vowel-sound → "an"; `an`+consonant-sound → "a"; `an`+vowel-letter-but-consonant-sound → "a"; `a`+consonant-letter-but-vowel-sound → "an".

**`placeholder-citation`** · warn — `[citation needed]` · `(Author, Year)` · `(Year)` · `[REF]` · `[TODO]` · `[TK]` · `[??]`.

**`tracking-param-in-citation`** · warn — a URL containing `?`/`&` + `utm_*`, `fbclid`, or `gclid`.

**`malformed-doi-isbn`** · advisory — `doi:<value>` not matching `10.NNNN/suffix`; `ISBN` whose digit count (after stripping separators, `X` allowed) is neither 10 nor 13.

**`unused-named-ref`** · advisory — a reference definition `[id]: url` never used by `][id]` or shortcut `[id]`.

### 11.4 Family C — Microsoft pack

Native rules:

- **`no-space-em-dash`** · advisory — spaced em-dashes ` — `; one finding per doc reporting the count (spaced em-dashes are a legitimate style; the convention is flagged once, not per use).
- **`no-internal-caps`** · advisory — `[a-z]+[A-Z]\w*` mid-word capitals, skipping the allowlist `JavaScript TypeScript GitHub GitLab GraphQL PostgreSQL MySQL iPhone iPad iOS macOS YouTube PayPal WordPress LinkedIn DevOps WiFi eBay OpenAI npm`, tokens with digits, tokens >16 chars, and multi-cap camelCase (clearly code).
- **`omit-you-can`** · advisory — every `you can` ("often cut it and use the imperative").
- **`avoid-we`** · advisory · ≥3 hits — `we|we're|our|us`; one finding at the first, reporting the count.
- **`spell-out-small-numbers`** · advisory — a standalone single digit in prose (guards: not adjacent to `\w . $ % / -`; not in a table).
- **`no-numeral-sentence-start`** · advisory — a sentence starting with a digit (ordered-list items exempt).
- **`large-number-grouping`** · advisory — ≥5 ungrouped digits in prose (not table); message shows the comma-grouped form.
- **`no-k-m-b`** · advisory — `$?\d+(.\d+)? [KMB]` → spell out million/billion.
- **`leading-zero`** · advisory — a bare `.5` → `0.5`.

Vale-parity ports (rule id · mechanic):

- **`microsoft-ampm`** · advisory — `12AM`, `12 am`, `12 a.m.` forms → "Use 'AM' or 'PM' (preceded by a space)". Table lines skipped.
- **`microsoft-accessibility`** · advisory (family D) — don't define people by disability. Full list: `a victim of, able-bodied, an epileptic, birth defect, crippled, differently abled, disabled, dumb, handicapped, handicaps, healthy person, hearing-impaired, lame, maimed, mentally handicapped, missing a limb, mute, non-verbal, normal person, sight-impaired, slow learner, stricken with, suffers from, vision-impaired`.
- **`microsoft-adverbs`** — specified in §11.2 (family B).
- **`microsoft-auto-hyphenation`** · advisory — any `auto-<word>` ("in general, don't hyphenate").
- **`microsoft-avoid-words`** · advisory — A–Z-list banned terms: `abortion` · `and so on` · `app(lication)s? (developer|program)` · `app(lication)? file` · `backbone` · `backend` · `contiguous selection`.
- **`microsoft-contractions`** · advisory — prefer the contraction: `how is→how's · it is→it's · that is→that's · they are→they're · we are→we're · we have→we've · what is→what's · when is→when's · where is→where's`.
- **`ms-date-format`** · advisory — `31 July 2016` style → "Use 'July 31, 2016' format".
- **`ms-date-numbers`** · advisory — month + spelled ordinal ("July first" … "thirty-first") → don't use ordinals for dates.
- **`ms-date-order`** · advisory — `MM/DD/YYYY` or `MM/DD/YY` → always spell out the month.
- **`ms-ellipses`** · advisory — `...` or `…` (not in tables).
- **`ms-first-person`** · warn · ≥2 — `I I'd I'll I'm I've me my mine` ("use first person sparingly").
- **`ms-foreign-abbrev`** · advisory — `e.g.→for example · i.e.→that is · viz.→namely · ergo→therefore · eg/ie` (bare `eg`/`ie`/`ergo` must be lowercase so "IE" the browser doesn't flag; dotted forms match either case; must be followed by space/comma).
- **`ms-gender-slash`** · warn (family D) — `he/she`, `s/he`.
- **`ms-gender-bias`** · warn (family D) — full pair list (pattern → replacement; `m[ae]n` covers man/men):
  `alumna|alumnus→graduate · alumnae|alumni→graduates · airman/airwoman→pilot(s) · anchorman/anchorwoman→anchor(s) · authoress→author · cameraman/camerawoman→camera operator(s) · doorman/doorwoman→concierge(s) · draftsman/draftswoman→drafter(s) · fireman/firewoman→firefighter(s) · fisherman/fisherwoman→fisher(s) · freshman/freshwoman→first-year student(s) · garbageman/garbagewoman→waste collector(s) · lady lawyer→lawyer · ladylike→courteous · mailman/mailwoman→mail carriers · man and wife→husband and wife · man enough→strong enough · mankind→human kind · manmade→manufactured · manpower→personnel · middleman/middlewoman→intermediary · newsman/newswoman→journalist(s) · ombudsman/ombudswoman→ombuds · oneupmanship→upstaging · poetess→poet · policeman/policewoman→police officer(s) · repairman/repairwoman→technician(s) · salesman/saleswoman→salesperson or sales people · serviceman/servicewoman→soldier(s) · steward(ess)→flight attendant · tribesman/tribeswoman→tribe member(s) · waitress→waiter · woman doctor→doctor · woman scientist(s)→scientist(s) · workman/workwoman→worker(s)`.
- **`microsoft-general-url`** · advisory — `URL(s)` → "for a general audience, use 'address'".
- **`microsoft-heading-acronyms`** · advisory — any `[A-Z]{2,4}` inside heading text.
- **`microsoft-heading-colons`** · advisory — `: <lowercase>` inside a heading → capitalize the first word after a colon.
- **`ms-adverb-hyphen`** · advisory — `<word>ly-<word>` needs no hyphen, excluding the shared non-adverb `-ly` exception set (also used by `google-ly-hyphen`):
  `family early only supply apply reply assembly friendly daily weekly monthly yearly hourly ally holy ugly lovely lonely lively costly deadly silly jelly belly italy curly burly surly wobbly bubbly gnarly melancholy anomaly monopoly panoply wholly homely timely orderly elderly likely unlikely`.
- **`ms-negative-number-endash`** · advisory — a space-preceded `-N` in prose → form negative numbers with an en dash.
- **`ms-ordinal-ly`** · advisory — `firstly, secondly, thirdly`.
- **`ms-percentages`** · advisory — spelled number (`zero…ninety`, `hundred`) + `percent` → use a numeral.
- **`ms-plurals-parenthetical`** · advisory — `(s)` or `(es)` appended to a noun → use the plural.
- **`microsoft-quotes-punctuation`** · warn — a curly-quoted span followed by `.`/`,` → punctuation inside the quotes (single-line only).
- **`microsoft-range-time`** · advisory — `AM–PM` dash ranges → use "to".
- **`microsoft-semicolon`** · advisory — every `;` (HTML entities and tables skipped) → "Try to simplify this sentence."
- **`ms-suspended-hyphen`** · advisory — `pre- and post-` suspended hyphenation.
- **`ms-term-swaps`** · advisory — full map:
  `adaptor→adapter · administrate→administer · alphanumerical→alphanumeric · an url→a URL · anti-aliasing→antialiasing · anti-malware→antimalware · anti-spyware→antispyware · anti-virus→antivirus · appendixes→appendices · afterwards→afterward · keypress→keystroke · conversation-as-a-platform→conversation as a platform · audio-book/audio book→audiobook · back-light→backlight · smart phone/smartphone/mobile phone→phone · 24/7→every day · web robot/internet bot→bot · machine language→assembly language · virtual assistant/intelligent personal assistant→personal digital assistant · chat bot/chat bots/chatbots→chatbot`.
- **`ms-url-of`** · advisory — `URL for` → `URL of`.
- **`ms-units-spelled-number`** · warn — spelled number (`zero…million`) + measurement unit (`(centi|milli)meters, (kilo)grams, (kilo)meters, (mega)pixels, cm, inches, lb, miles, pounds`) → numeral with the unit.
- **`ms-vocab-az-wordlist`** · advisory · ≥2 — verify against the Microsoft A–Z word list: `above, accessible, actionable, against, alarm, alert, alias, allow, allows, and/or, as well as, assure, author, avg, beta, ensure, he, insure, sample, she`.
- **`ms-wordiness`** · advisory — the large phrase→concise map, full contents:
  `sufficient number of→enough · sufficient number→enough · take away→remove · eliminate→remove · as a means to→to · as a means of→to · in an effort to→to · inform→tell · let me know→tell · previous to→before · prior to→before · utilize→use · make use of→use · a large majority of→most · a majority of→most · a large number of→many · a number of→many · a myriad of→myriad · adversely impact→hurt · all across→across · all of a sudden→suddenly · all of these→these · all of→all · all-time record→record · almost all→most · almost never→seldom · along the lines of→similar to · an adequate number of→enough · an appreciable number of→many · an estimated→about · any and all→all · are in agreement→agree · as a matter of fact→in fact · as a result of→because of · as of yet→yet · as per→per · at a later date→later · at all times→always · at the present time→now · at this point in time→at this point · based in large part on→based on · based on the fact that→because · basic necessity→necessity · because of the fact that→because · came to a realization→realized · came to an abrupt end→ended abruptly · carry out an evaluation of→evaluate · close down→close · closed down→closed · complete stranger→stranger · completely separate→separate · concerning the matter of→regarding · conduct a review of→review · conduct an investigation→investigate · conduct experiments→experiment · continue on→continue · despite the fact that→although · disappear from sight→disappear · doomed to fail→doomed · drag and drop→drag · drag-and-drop→drag · due to the fact that→because · during the period of→during · during the time that→while · emergency situation→emergency · establish connectivity→connect · except when→unless · excessive number→too many · extend an invitation→invite · fall down→fall · fell down→fell · for the duration of→during · gather together→gather · has the ability to→can · has the capacity to→can · has the opportunity to→could · hold a meeting→meet · if this is not the case→if not · in a careful manner→carefully · in a thoughtful manner→thoughtfully · in a timely manner→timely · in addition→also · in between→between · in lieu of→instead of · in many cases→often · in most cases→usually · in some cases→sometimes · in spite of the fact that→although · in spite of→despite · in the very near future→soon · in the near future→soon · in the event that→if · in the neighborhood of→roughly · in the vicinity of→close to · it would appear that→apparently · lift up→lift · made reference to→referred to · make reference to→refer to · mix together→mix · none at all→none · not in a position to→unable · not possible→impossible · of major importance→important · perform an assessment of→assess · pertaining to→about · place an order→order · plays a key role in→is essential to · present time→now · readily apparent→apparent · some of the→some · span across→span · subsequent to→after · successfully complete→complete · take action→act · take into account→consider · the question as to whether→whether · there is no doubt but that→doubtless · this day and age→this age · this is a subject that→this subject · time frame→time · time period→time · under the provisions of→under · until such time as→until · used for fuel purposes→used for fuel · whether or not→whether · with regard to→regarding · with the exception of→except for`.
  (`in order to` deliberately absent — the always-on `wordy-phrase` owns it.)

### 11.5 Family C — Google pack

Native rules:

- **`no-gerund-heading`** · warn — heading whose first word ends in `-ing` and is >4 chars.
- **`no-link-in-heading`** · warn — a markdown link inside heading text.
- **`latinism-abbreviation`** · warn · map — `e.g.→for example · i.e.→that is · etc./etc→and so on · via→through · vs.→versus`.
- **`minimizing-words`** · warn · per hit — `easy, easily, simple, simply, just, quick, quickly, obviously, of course, merely, trivial` ("it's not easy for everyone").
- **`no-abbreviation-as-verb`** · advisory — `(ssh|rsync|scp|ftp|chmod|grep) (into|to)` not preceded by "use "/"using " → "use SSH to …".
- **`no-periods-in-acronyms`** · advisory — `(X.)(Y.)…` dotted acronyms, exempting `e.g.`, `i.e.`, `etc.`.
- **`no-exclamation`** · warn — `!` after a word char (excluding `!=`).
- **`american-spelling`** · warn · map, full:
  `colour(s)→color(s) · favour→favor · behaviour→behavior · flavour→flavor · honour→honor · labour→labor · neighbour→neighbor · organise(d)→organize(d) · recognise→recognize · analyse→analyze · catalogue→catalog · dialogue→dialog · centre→center · metre→meter · licence→license · defence→defense · grey→gray · cancelled→canceled · travelling→traveling · modelling→modeling`.
- **`no-preannounce`** · advisory · per hit — `currently, presently, at this time, latest, newest, brand-new, soon, in the near future, upcoming` ("docs outlive it").
- **`no-directional`** · advisory · map — `above→preceding · below→following`.

Vale-parity ports:

- **`google-ampm`** · warn — number joined to am/pm forms → "'AM'/'PM' preceded by a space".
- **`google-contractions`** · advisory — same map as `microsoft-contractions`.
- **`google-date-format`** · advisory — `D.M.YYYY`, `D/M/YYYY`, or `31 July 2016` → "July 31, 2016".
- **`google-ellipses`** · advisory — `...`.
- **`google-dash-spacing`** · advisory — a spaced em/en dash ` — `/` – ` → no space around a dash.
- **`google-first-person`** · warn · ≥2 — same tokens as `ms-first-person`; "address the reader".
- **`google-gender-neutral-pronoun`** · warn (family D) — `he/she`, `s/he`, `(s)he` → "they".
- **`google-gender-bias`** · warn (family D) — the same 36-pair list as `ms-gender-bias` (replacement for `mankind` is "human kind or humanity").
- **`google-ly-hyphen`** · advisory — same mechanic + exception set as `ms-adverb-hyphen`.
- **`google-optional-plurals`** · advisory — `word(s)` → rewrite as plural or "one or more".
- **`google-ordinal`** · warn — `1st|2nd|3rd|4th…` numerals-with-suffix → spell out ordinals.
- **`google-quote-punctuation`** · advisory — a straight-quoted span followed by `.`/`,`/`?` → punctuation inside the quotes.
- **`google-number-range-words`** · advisory — `(from|between) N-M` → drop the words around a numeric range.
- **`google-semicolons`** · advisory — every `;` (tables skipped) → "use semicolons judiciously".
- **`google-slang`** · warn — `tl;dr, ymmv, rtfm, imo, fwiw`.
- **`google-units-nbsp`** · advisory — a number joined to `kB|MB|GB|TB|min|ns|ms` with no space → nonbreaking space between number and unit. (Ambiguous single-letter units d/s/h/B deliberately excluded: "the 60s", "3d rendering", "747s".)
- **`avoid-first-person-plural`** · advisory · ≥2 — `we, we've, we're, our(s), us, let's`.
- **`avoid-will-future-tense`** · advisory · ≥2 — every bare `will` (`\b` so "willing"/"goodwill" don't match) → prefer present tense.
- **`google-word-list`** · advisory · map, full:
  `dev key/developer key/api console key→API key · cellphone/cell phone/smartphone/smart phone→phone · dev console/developer console/apis console→API console · e-mail→email · filepath/file path/pathname/path name→path · oauth2→OAuth 2.0 · wifi→Wi-Fi · google i-o/google io→Google I/O · tap and hold/long press→touch & hold · uncheck/unselect→clear · account name→username · action bar→app bar · ajax→AJAX · authn→authentication · authz→authorization · autoupdate→automatically update · cellular data→mobile data · cellular network→mobile network · check box→checkbox · click on→click · container engine→Kubernetes Engine · content type→media type · curated roles→predefined roles · data are→data is · file name→filename · k8s→Kubernetes · network ip address→internal IP address · omnibox→address bar · sign into→sign in to · stylesheet→style sheet · tablename→table name · vs.→versus · world wide web→web · approx.→approximately`.
  (Case-only entries like `ajax→AJAX` skip when already the preferred form; `in order to` deliberately absent.)

### 11.6 Family C — AP pack

- **`ap-serial-comma`** · advisory — flags the Oxford comma's *presence* (`\w+, \w+, (and|or) \w+`, anchored at the comma before the conjunction). The shared `serial-comma` self-suppresses under AP, so the two never both fire.
- **`ap-number-style`** · advisory — spell out whole numbers zero through nine (same standalone-digit mechanic as `spell-out-small-numbers`).
- **`ap-percent`** · advisory — `N%` → spell out "percent".
- **`ap-time-format`** · advisory — `12 PM`/`12:30 AM` forms → lowercase with periods, "a.m."/"p.m.".
- **`ap-dollar-style`** · advisory — `5 million dollars` → "$5 million".
- **`ap-over-quantity`** · advisory — `over <number|$>` → "more than" with quantities.
- **`ap-toward`** · advisory · map — `towards→toward · backwards→backward · upwards→upward · downwards→downward · afterwards→afterward`.
- **`ap-ampersand`** · advisory — a freestanding ` & ` → "and" except in proper names.

### 11.7 Family C — Chicago pack

- **`chicago-number-style`** · advisory — spell out whole numbers ≤100 in prose (1–3-digit standalone numerals, value ≤ 100, tables skipped). Chicago also requires the Oxford comma — that's the always-on shared `serial-comma`.
- **`chicago-directional-s`** · advisory · map — `towards→toward · afterwards→afterward · backwards→backward · upwards→upward · downwards→downward · onwards→onward`.
- **`chicago-percent-symbol`** · advisory — digit + `%` → spell out "percent" in running prose.
- **`chicago-em-dash-spacing`** · advisory — spaced em dash ` — ` → close it up.
- **`chicago-ellipsis`** · advisory — the `…` glyph → three spaced periods ". . .".
- **`chicago-united-states-noun`** · advisory — `the U.S.`/`the US` used as a noun (followed by a verb `is/are/was/were/has/have/had/will/would` or terminal punctuation) → spell out "United States"; abbreviate only as an adjective.
- **`chicago-ibid`** · advisory — `ibid.`, `op. cit.`, `loc. cit.` → shortened citations (Chicago 17th ed.).

### 11.8 Family C — Plain pack

- **`plain-long-sentence`** · advisory — sentences of 21–30 words (the band the shared 30-word `long-sentence` misses, so the two never double-report). PLAIN wants <20.
- **`plain-hidden-verb`** · advisory · map, full:
  `make a determination→determine · provide an explanation→explain · conduct a review→review · perform a calculation→calculate · give authorization→authorize · make a recommendation→recommend · reach a decision→decide · make use of→use · make reference to→refer to · provide notification→notify · make an adjustment→adjust · is in violation of→violates`.
- **`plain-shall`** · advisory — every `shall` ("ambiguous in instructions — use 'must'").
- **`plain-required-to`** · advisory · map — `is required to→must · are required to→must · will be required to→must`.
- **`plain-legalese-phrase`** · advisory · map — `pursuant to→under · in accordance with→under · prior to→before`.
- **`plain-legalese-word`** · advisory — `herein, thereof, aforementioned, heretofore, notwithstanding, hereinafter`.
- **`plain-double-negative`** · advisory — `not (uncommon|unusual|unlikely|unreasonable|unimportant|insignificant|infrequent|inexpensive|unhelpful|impractical|unclear)` → state it positively.
- **`reading-grade`** — §11.2.

### 11.9 Family D — inclusive & accessible (always on)

**`gendered-language`** · warn · map, full:
`chairman→chair · chairmen→chairs · mankind→humanity · manpower→workforce · man-hours→person-hours · manned→staffed · salesman→salesperson · salesmen→salespeople · policeman→police officer · policemen→police officers · layman→layperson · laymen→laypeople · freshman→first-year student · fireman→firefighter · firemen→firefighters · stewardess→flight attendant · mailman→mail carrier · businessman→businessperson · man-made→artificial`.
Pack precedence: under Microsoft/Google, the terms their gender-bias pack rules also match are suppressed here (`mankind, manpower, salesman, salesmen, policeman, policemen, fireman, firemen, stewardess, mailman, freshman`) so one token never reports twice.

**`ableist-language`** · warn + advisory · two maps:
warn (metaphorical): `crazy→wild / baffling · insane→extreme · psycho→erratic · lame→weak · dumb→foolish · tone-deaf→insensitive · cripple/cripples/crippling→degrade(s)/degrading`. advisory (CS-idiomatic): `sanity check→consistency check · sane→reasonable · dummy value→placeholder value`.

**`vague-link-text`** · warn (WCAG) — link text (trimmed, lowercased) exactly one of: `click here, here, read more, this, this link, link, more`.

**`skipped-heading`** · warn / advisory — a heading more than one level below its predecessor (h2→h4); advisory for a second h1.

**`person-first-language`** · warn · map, full:
`suffers from→has · suffering from→living with · victim of→person affected by · wheelchair-bound→wheelchair user · confined to a wheelchair→uses a wheelchair · an epileptic→a person with epilepsy · the disabled→disabled people · the mentally ill→people with mental illness · normal people→people without disabilities`.

**`gendered-address`** · advisory — `guys, gentlemen, ladies` → "everyone / folks".

**`tech-historical-terms`** · warn + advisory
warn map (full): `blacklist(s)→blocklist(s) · blacklisted→blocked · whitelist(s)→allowlist(s) · whitelisted→allowed · master/slave→primary/replica · grandfathered/grandfather→legacy · blackhat→unethical · whitehat→ethical · first-class citizen→fully supported · sanity→confidence`.
advisory map (high-FP, context-dependent): `master→primary / main · slave→replica / worker · native→built-in · primitive→basic · tribe→team` — suppressed when the ±12-char context matches the exemption regex `master's|scrum master|master class|native speaker|primitive type|native to`.

**`violent-tech-metaphor`** · advisory · map, full:
`abort(s)→stop(s) · kill→end · killing→ending · hang(s)→stop(s) responding · blast radius→scope of impact · dmz→perimeter network`. Suppressed when followed by a number (`kill -9`). `hit` deliberately excluded — "cache hit", "hit the endpoint" are standard.

**`ageist-classist-cultural`** · advisory · map, full:
`ghetto→makeshift · gypsy→traveler · gypped→cheated · oriental→Asian · eskimo→Inuit · third-world/third world→developing · the elderly→older adults · illegal immigrant/illegal alien→undocumented immigrant · sketchy→questionable`.

**`missing-alt-text`** · warn — an image with empty alt text (explicit empty alt for decorative images is the documented escape).

**`all-caps-shouting`** · advisory — a run of ≥3 all-caps words of ≥2 letters ("screen readers spell it out").

**`bare-url`** · advisory — a raw `http(s)://` URL in prose (not a link target `](…)`, autolink `<…>`, attribute/quoted context, or reference definition `[id]: url`) → use descriptive link text.

### 11.10 Grounding rules (factcheck engine)

Emitted with family `grounding`. Rule ids: **`number-date-mismatch`** (error) · **`contradicts-fact`** (error) · **`unsupported-claim`** (warn under `--source`, else advisory) · **`ungrounded-span`** (advisory, attention tier).

**Typed-span extraction** (per sentence and per fact; later extractors skip offset ranges already covered):
1. **percent** — `(\d+(.\d+)?) ?%` → float.
2. **money** — `$ N[,N…][.N] (million|billion|thousand|k|m|b)?` → value scaled (k/thousand ×10³, m/million ×10⁶, b/billion ×10⁹).
3. **date** — three forms, all canonicalized to ISO (`YYYY-MM-DD`, or `YYYY-MM` for month-year): `YYYY-MM-DD`; `DD(st|nd|rd|th)? Month[,] YYYY`; `Month DD[,] YYYY` / `Month YYYY`. Two dates are *compatible* when equal or one is a coarser truncation of the other (`2024-03` vs `2024-03-15` — granularity, not contradiction).
4. **year** — standalone `(19|20)\d\d`.
5. **count** — any remaining `\d[\d,]*(.\d+)?` (commas stripped).

**Entities:** capitalized word sequences (connectors `of|the|and` allowed inside); a lone sentence-initial capitalized word is skipped unless it's an acronym or carries a digit; plus all-caps acronyms `[A-Z]{2,6}`. **Content tokens:** lowercase words ≥3 chars minus a ~90-word stopword list (articles, auxiliaries, prepositions, pronouns, question words).

**Fact parsing:** each non-heading, non-comment line of FACTS.md, list markers stripped; a trailing `(…)`/`[…]` containing a URL, `source:`, or a year is captured as the fact's source. With `--source <file>`, every sentence of the file is a fact.

**Retrieval:** for each claim, score every fact `shared content tokens + 2 × shared entities`; best fact is *relevant* when score ≥ 3 AND ≥1 shared token.

**Tier 0 verdicts:** a sentence is checkable when it has ≥1 typed span AND (≥2 content tokens OR ≥1 entity). If a relevant fact exists and shares a span *kind* with disjoint value sets → **error** (`number-date-mismatch` for date/year kinds, else `contradicts-fact`), citing both raw values and the fact line. If no relevant fact → `unsupported-claim` anchored at the highest-value span (money/percent/year/date preferred over count).

**NLI tier** (with `--models`; premise = fact, hypothesis = claim): typed-span mismatch stays the hard error; otherwise contradiction ≥ 0.60 and > entailment → `contradicts-fact` (error, with NLI %); entailment ≥ 0.55 → supported (no finding); else neutral → `unsupported-claim`. *Rust:* run the NLI cross-encoder via `ort` or `candle` + `tokenizers`.

**Decomposed tier** (`--decompose`/`--claims`): claim candidates are sentences ≥12 chars with (≥1 typed span OR ≥4 content tokens) — `--emit-claim-targets` prints exactly this list, and supplied claims align to it by index. Each atomic claim runs the same retrieve → typed-span → NLI pipeline; findings anchor to the *parent* sentence and carry the atomic claim in the message; identical sibling findings dedupe on `(ruleId, offset, message)`. Decomposition is done by the agent, never by the CLI.

**Attention tier** (`--deep`/`--ground=attention`): sentences ≥12 chars are scored for attention lookback against the source; a span below threshold (default **0.10**) emits `ungrounded-span` — "reads as ungrounded", never an assertion of falsehood. *Rust:* `llama-cpp-2` with a small GGUF model, eager attention capture (§17).

### 11.11 Grammar rules (opt-in)

The grammar pass is **Harper** (Automattic) — natively a Rust library (`harper-core`; the prototype used its WASM build `harper.js`). Fully offline, no network. Behavior contract:

- Opt-in only (`--grammar` / `detector.grammar` / `hook.grammar`); the default detector stays pure-deterministic and synchronous.
- Run Harper's markdown parser (skips fenced/inline code; offsets return in the original source).
- **Keep only high-precision lint kinds:** `Agreement` (subject-verb/article-noun), `Grammar` (structural — "allows to deliver"), `Miscellaneous` (includes wrong indefinite article), `Eggcorn` ("for all intensive purposes"), `Malapropism`, `Nonstandard`, `BoundaryError` (run-ons), `Redundancy` ("and also").
- **Dropped kinds** (heavy false positives on technical markdown, or overlap with Mari's own rules): Spelling, Typo, Capitalization, Formatting, Punctuation, WordChoice, Style, Regionalism, Readability.
- **Disabled individual rules** within kept kinds: `MassNouns` (mislabels ordinary count nouns), `MissingPreposition` (fires vaguely on bare nouns).
- Findings emit as `grammar-<kind lowercased>`, family `grammar`, severity warn, with Harper's message plus its **top 3 suggestions** (an empty replacement renders as "(remove)"), sorted by offset, capped at **30 per file**.
- Grammar must never break detection: any failure (missing dependency, engine error) returns zero findings, with at most one stderr notice.
- (Rust-specific simplification: Harper's scalar-index vs UTF-16 offset conversion in the JS prototype is unnecessary — `harper-core` and Rust strings share UTF-8 byte offsets.)

### 11.12 Readability internals

Used by `reading-grade` (and the `--score` word stats). Syllable counting is heuristic (~3–8% per-word error; fine for aggregate scoring):

1. Lowercase, strip non-letters. Exceptions table first: `every 2 · business 2 · different 3 · comfortable 3 · vegetable 3 · february 4 · area 3 · idea 3 · science 2 · being 2 · create 2 · people 2 · simile 3 · queue 1 · the 1 · average 3 · naive 2 · real 1 · cereal 3`.
2. Strip silent endings (`-es` after non-l vowel-consonant, `-ed`, silent `-e`); strip leading `y`.
3. Count vowel groups `[aeiouy]{1,2}`.
4. +1 for consonant+`le` endings; +1 for hiatus (`ia|io|ua|eo`). Minimum 1.

Grade formulas (W words, S sentences, syl syllables, L letters):
`FKGL = 0.39·(W/S) + 11.8·(syl/W) − 15.59` · `CLI = 0.0588·(L/W·100) − 0.296·(S/W·100) − 15.8` · reported grade = `(FKGL + CLI) / 2`.

### 11.13 Fixture discipline

Every rule ships a bad→good fixture pair; the test suite asserts each rule fires on its bad fixture and stays silent on its good one (~180 assertions). Regression checks cover table-aware number rules, masking (front matter, comments, shortcodes), CJK/generated/vendored skipping, and large-repo false-positive budgets. A deliberate-slop self-test fixture must produce a known finding set.

---

## 12. Slop score

`mari detect --score` computes a 0–100 score (higher = sloppier). Exact mechanics — the breakdown is always returned so the number is explainable (Mari never asserts "this is AI-written"; it shows why a passage reads machine-made):

1. **Weighted finding mass:** each finding contributes `SEV × FAM` where `SEV` = error 3 / warn 2 / advisory 1, and `FAM` = ai-slop 1.0 / grounding 1.0 / inclusive 0.5 / clarity 0.4 / style 0.3 (unknown family 0.3). `per1k = Σ / words × 1000`.
2. **Saturating base:** `base = 100 × (1 − e^(−per1k/35))` — heavy slop approaches 100 without exceeding it.
3. **Human-signal discount:** count contractions (`\w+['’](t|s|re|ve|ll|d|m)`) plus first-person tokens (`I, I'm, I've, I'll, I'd, we/We (+'re 've 'll 'd), my/My, our/Our, me/Me, us/Us` — case-sensitive only for bare `I`, so list markers and math `i` don't count). `discount = min(15, (contractions + firstPerson)/words × 1000 × 1.5)`.
4. **Deterministic score:** `max(0, base − discount)`.
5. **Model blend** (only when a machine-likelihood `m ∈ [0,1]` is available via `--models`): `score = 0.8 × deterministic + 0.2 × (m × 100)` — the model term never dominates.
6. Round and clamp to 0–100. **Bands:** `clean` < 12 · `light` 12–29 · `moderate` 30–59 · `heavy` ≥ 60.

The reported breakdown includes: word count, finding count, weighted density per 1k, findings by family, human signals (contraction count, first-person count, discount), and machine likelihood when present.

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
4. **Nudges** — for any edited file matching a nudge's `when` (and, if `when.symbol` is set, an edit intersecting that resolved span), emit a directive per nudge: `✎ nudge <name>: <when-target> changed — edit <target>[, <target>…]` plus its `message`. This tells the agent to make those edits now; the hook itself still never modifies files. A symbol that fails to resolve degrades to whole-file matching with a warning.
5. **Lineage impact** — if a confirmed lineage edge's endpoint drifted, emit a semantic-lineage notice (`⛓ …`) telling the agent which spans to reconcile. Suppressed for a span pair a nudge already fired on.
6. **Association notice** — derived-assoc "related files" note (suppressed when a nudge or lineage notice already fired).
7. **Knowledge pending-impact** — note when scanned knowledge affecting this file changed.
8. **Tag advisories** — editing a `stale`/`deprecated`-tagged file, or referencing `internal` content from a `customer-facing` file (§10.1).

Invariants: always exit 0; emit nothing on internal failure; respect `hook.*` toggles; never modify files.

### 15.2 Commit association (git hook, optional)
An opt-in `post-commit` hook associates new commits with relevant knowledge (issues, conversations, docs) via the edge graph and embedding neighbors. It also flags commits that touched code covered by an edit-notify rule or a nudge's `when` without a matching change to the notify target / nudge `edit` targets — "context is never lost."

---

## 16. Command router & skill routing

Mari's slash surface has two layers: a set of **standalone commands** for the high-frequency actions (so `/search why did we change pricing tiers` works without a `/mari` prefix), and the **`/mari` general router** that covers everything else — subcommand dispatch, natural-language questions, and intent phrases. Every standalone command is a thin skill wrapper over the same flow the router would run; behavior is identical whichever entry point is used.

### 16.1 Standalone commands (ship by default)

| Command | Flow | Notes |
|---|---|---|
| `/search <question>` | Knowledge flow (§16.3) | Accepts natural language ("theres an outage in #incidents, what is causing it"), not just keyword queries. Flags pass through to `mari search`. |
| `/sync [source]` | `mari sync` | The one command **never** run unprompted; `/sync` is the explicit user prompt. |
| `/tag <path-or-ref> <status>` | `mari tag` | Also `/tag list`, `/tag remove`. |
| `/factcheck <file> [--source F]` | `mari factcheck` | Agent adds `--decompose` claim decomposition when depth is asked for. |
| `/audit [path]` | `mari audit` / `mari audit kb` | Bare path → detector report; "audit the knowledge base" phrasing → `audit kb`. |
| `/deslop <target>` | deslop verb (§13) | |
| `/tighten <target>` | tighten verb | |
| `/clarify <target>` | clarify verb | |
| `/sharpen <target>` | sharpen verb | |
| `/understate <target>` | understate verb | |
| `/critique <target>` | critique verb | Review only; never rewrites. |
| `/polish <target>` | polish verb | |
| `/draft <brief>` | draft verb | |

`<target>` may be a path, a natural-language reference ("the changelog", "the error copy"), or omitted — then the command applies to the file(s) just edited in the session, else asks.

**Pinning.** Teams can pin any other router-reachable action as a standalone command (e.g. `/docsite`, `/glossary`, `/outline`, `/soften`) or unpin defaults; the standalone set is a projection of the router, so pinning changes discovery, never behavior. Everything remains reachable as `/mari <verb|subcommand>` regardless of what is pinned.

### 16.2 The `/mari` general router

- **Bare `/mari <file>` or no-arg** → run detector, surface the top 2–3 recommended verbs; never auto-edit.
- **`/mari <known-subcommand> …`** → route to the command (init, sync, status, search, tag, config, features, docsite, glossary, facts, extract, nudge, rules, audit, localize, …). Any standalone command's verb also works here (`/mari deslop README.md` ≡ `/deslop README.md`).
- **Natural-language question** → knowledge flow (§16.3).
- **Editing intent phrases** map to verbs: "make it punchier"→sharpen, "cut it down"→tighten, "make it less salesy"→soften, "sounds like AI"→deslop, "prepare for launch"→polish, etc.
- **Coupling intent phrases** map to `nudge add`: "whenever X changes, update Y", "keep this section in sync with that function" → compose the `--when`/`--edit` pair (with `#symbol` when the user names a function or heading), confirm, and run it.
- **Connector setup** → the relevant `connect-<source>` skill: scope question (with per-source default), method choice, click-by-click credential walkthrough, the three credential-handling paths, `mari auth` + `mari track add` + first `mari sync`, confirmation.
- **Ambiguity rule:** when input could be either a question or an edit request, prefer the knowledge flow for interrogatives and the detector-first flow for file references; ask only when both readings are plausible and consequential.

### 16.3 Knowledge flow (shared by `/search` and `/mari <question>`)

Compose a toolbox, not one search — `search` with agent-generated `--variant`s, then `doc`/`thread`/`related`/`recent`/`neighbors`/`sql` as needed. Extract identifiers from early hits and feed them back as variants. **Never conclude from a truncated preview** — use `--full`. Answer from the current index even when stale; suggest `/sync` but never run it unprompted.

### 16.4 Guardrails

Setup is assistant-guided end-to-end; the user never has to run anything (but always may). Sync is the one command never run unprompted — `/sync` (or an explicit ask) is the only trigger. Standalone editorial commands follow the same verb contract as the router: preserve meaning and voice, rewrite-not-delete, re-run the detector after.

Connector-setup skills ship per source: `connect-slack connect-github connect-gdocs connect-confluence connect-jira connect-zendesk connect-salesforce connect-hubspot connect-microsoft connect-discord connect-linear`.

---

## 17. ML capability tiers

Detection and grounding are layered by model size, never "rules vs AI":

1. **Tier 0 — deterministic (always on):** the full rule registry, typed-span factcheck, structural checks. Instant, offline, dependency-free.
2. **Tier 1 — local small models (default-on once provisioned, `--no-models` to skip):** machine-likelihood (perplexity), NLI entailment/contradiction (factcheck + audit contradictions), zero-shot slop-span extraction (labels: marketing buzzword, hype phrase, vague corporate jargon, empty filler phrase, overused cliché), embeddings (search/explore/assoc). Models load lazily into a resident sidecar; only structured output crosses the boundary. *Rust:* `ort` (ONNX Runtime) or `candle` for the NLI cross-encoder and embedding models, `tokenizers` for tokenization, `gline-rs` for GLiNER slop spans, `fastembed` (fastembed-rs) as a batteries-included embedding/rerank alternative — all in-process, which removes the prototype's Python sidecar entirely.
3. **Tier 2 — local attention/generative (opt-in via configured model):** attention grounding with three modes — **coverage** (context the query ignores: dropped translation content, stale docs↔code), **grounding** (query sentences that ignore context: fabricated/unsupported), **focus** (where attention mass lands). Powers every `--deep` flag and `lineage refine`. ~seconds per document. *Rust:* `llama-cpp-2` (llama.cpp bindings) loads the GGUF model (qwen3.6 0.8b only), computes perplexity, and exposes attention capture for the mid-layer band — replacing the prototype's custom C++ binary.
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

- No SaaS requirement; no server in the core product (a hosted sync layer may exist later as an optional backend).
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
- **nudge** — a hand-declared edit obligation: when a file (or symbol span) changes, the agent is directed to edit named target files/spans.

