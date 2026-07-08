---
name: mari
description: Curate, search, and improve your team's product knowledge. Searches connected sources — git commit history, Slack, Google Drive, GitHub, Linear, Confluence, Jira, Zendesk, Salesforce, HubSpot, Microsoft 365, Discord, and local files (including PDFs) — with local hybrid (semantic + keyword) retrieval. Also the design system for text — write, rewrite, edit, critique, audit, polish, understate, tighten, clarify, or de-slop any prose; tag what Claude should trust; factcheck claims; maintain docs, translations, and doc↔code lineage. Use when the user asks a question their work docs/messages/commits would answer, wants to connect or sync a source, wants prose written or improved, or types /mari, /search, /deslop, /tag, /factcheck, or /sync.
version: 0.1.0
user-invocable: true
argument-hint: "[command] [target] | <question>"
allowed-tools: Bash(mari *), Read, Edit
---

# Mari — curated project memory + a design system for text

You are driving **Mari**, which does two jobs from one CLI (`mari <subcommand>`):

1. **Knowledge** — a local hybrid search index over the repo's git history and the user's
   connected sources (12 cloud connectors + git + local files). Runs entirely on this machine
   (their credentials, index under `~/.mari/<repo>-<hash>/`); a repo may carry a committed
   `.mari/` folder with team-shared sources — and, under the git storage backend, the index
   itself.
2. **Editorial** — a deterministic detector (AI-slop, clarity, house style, inclusive language)
   plus editorial commands. The detector is the source of ground truth — run it first so every
   edit is grounded in concrete findings, not vibes.

**You run every mari command yourself via Bash.** Setup is assistant-guided; the user never has
to run anything. By default, ask the user only for the token string and set the source up
yourself. Two exceptions: interactive sign-ins (Google via gcloud, Microsoft device-code) open a
browser/prompt the user completes; and a **privacy-minded user may prefer their token never
reach you** — hand them the exact `mari auth …` command (or the credential-file path) to run
themselves, and continue once they say it's done.

## Setup (run before any prose-editing command)

> **Deterministic commands skip this setup.** `detect`, `audit`, `asset`, `i18n`, `platform`,
> `community`, `check`, and all knowledge commands are mechanical — they need no `PRODUCT.md`,
> register, or voice context. Setup applies to the prose-*editing* commands (`deslop`,
> `tighten`, `clarify`, …).

1. **Load context.** Run `mari context`. It prints `PRODUCT.md` (+ `STYLE.md`, `FACTS.md`) or
   `NO_PRODUCT_MD`. If `NO_PRODUCT_MD` and the user asked for a prose-editing command, run
   **`init`** (style flow) first, then resume.
2. **Load the command reference.** If a sub-command was named, read `reference-<command>.md` —
   that's the authoritative flow.
3. **Read the existing writing.** Sample at least one representative file so edits match the
   project's real voice — never impose a generic voice on a project that already has one.
4. **Load the register reference** (non-optional). Pick by first match: task cue → surface in
   focus → `register` in `PRODUCT.md`: `reference-register-{docs,marketing,editorial,microcopy}.md`.
5. **Run the detector** on the target (`mari detect <target>`) and let its findings drive the edit.
6. **Check for a developer asset.** Run `mari asset detect <target>`. If it reports a type
   (runbook / ADR / postmortem / RFC, or a community doc: contributing / code-of-conduct /
   governance / security), load `reference-asset-<type>.md` and run `mari asset check <target>`.
   Apply that type's structure requirements, tone norms, and rubric on top of the register. To
   create one, scaffold from best practice with `mari asset scaffold <type> "<title>"`.

## Routing

Route on what follows `/mari` (or, if the skill auto-triggered, on the user's message):

- **Empty or a plain question** → the retrieval flow below. If the target is clearly a file and
  the ask is editorial, run the detector and surface the 2–3 highest-value commands (many
  buzzword/cliché hits → `deslop`; long-sentence hits → `tighten`; passive/jargon → `clarify`;
  inclusive/heading/link hits → `audit`). Never auto-edit.
- **`init`** → `references/reference-init.md` — routes to the **search** flow (connectors), the **style**
  flow (PRODUCT.md/STYLE.md/hook), or both.
- **`sync` / `status` / `config` / `features` / `plugins`** → run the CLI directly (see below).
- **`tag` / `facts` / `extract` / `glossary`** → the curation flows (`references/reference-tag.md`,
  `references/reference-extract.md`, `references/reference-glossary.md`). Tagging is a team decision — confirm the
  status meaning before writing it.
- **A deterministic docs command** (`detect`, `audit`, `asset`, `i18n`, `platform`, `community`,
  `check`, `surface`, `explore`, `lineage`, `scan`, `factcheck`) → run the CLI directly, loading
  `reference-<command>.md` where one exists (factcheck, lineage, scan, platform, community,
  docsite have interactive flows). `check --strict` is the CI gate; add `--deep` only when asked,
  `--anchors` to validate in-page `#anchor`→`id` links in HTML/JSX on code-based sites. To lint
  copy that lives in code, `detect --strings <dir>` extracts and checks user-facing strings; for
  a list of nav/menu labels, `detect --labels` treats each line as its own unit.
- **`docsite`** (or "document the whole codebase") → the end-to-end flow in
  `references/reference-docsite.md`: survey the code, choose + scaffold a platform, design the information
  architecture (Diátaxis), fill every page from the code, add community-health files, validate
  with `check --strict`. `docsite check` is the focused links-only validator; `docsite sync`
  reports command/config drift between the docs and the real surface. Removing a page is a plain
  delete — the hook reruns `docsite check` to surface what broke.
- **An editing command** (`init`, `document`, `draft`, `outline`, `glossary`, `critique`,
  `deslop`, `humanize`, `understate`, `tighten`, `clarify`, `polish`, `sharpen`, `soften`,
  `harden`, `voice`, `cadence`, `format`, `delight`, `adapt`, `localize`, `live`) → run the
  setup phase, load `reference-<command>.md`, and run it; the rest of the line is the target.
- **Connector setup** ("connect Slack", "add my Jira") → invoke the matching guided
  `/connect-<source>` skill (`connect-slack`, `connect-github`, `connect-gdocs`,
  `connect-confluence`, `connect-jira`, `connect-linear`, `connect-zendesk`,
  `connect-salesforce`, `connect-hubspot`, `connect-microsoft`, `connect-discord`) rather than
  improvising the steps.
- **Intent maps to a command** — "make this punchier" → `sharpen`, "cut this down" → `tighten`,
  "cut the explanation/restatement" → `understate`, "fix the error copy" → `clarify`, "tone down
  the hype" → `soften`, "sounds like AI" → `deslop`, "prepare for launch" → `polish`, "are
  translations in sync?" → `i18n conform`, "what should Claude trust here?" → `tag`.
- **Curation intent** — "tag the knowledge base", "which docs are stale/out of date?", "what
  should Claude trust?", "mark the deprecated stuff" → the auto-tagging flow `tag analyze` (scoped
  to the named status when one is implied). Proposals only; tags are written after user
  confirmation.
- **No clear match** → a general editing pass using setup context + detector findings, or the
  retrieval flow if it reads as a question.

## A question — retrieve intelligently, then answer

Mari gives you a **toolbox of retrieval commands**. Don't just run one search — decide what
context the question needs, then compose calls. Every command prints human-readable text — read
its output. All accept `--source <connector>`: one of `git`, `slack`, `gdocs`, `github`,
`linear`, `confluence`, `jira`, `zendesk`, `salesforce`, `hubspot`, `microsoft`, `discord`,
`localfiles`. `git` is the repo's commit history (no auth, indexed by default) — `search
--source git`, `recent --source git --author ada`, and `--since` answer "when/why did we
change X".

**Previews are truncated by default** (5 lines × ~110 chars per hit) so a list stays scannable —
**don't conclude from a preview alone**. `search`/`recent`/`related` take **`--full [N]`** to
print whole bodies: bare `--full` caps at 4000 chars/hit, `--full 8000` sets your own cap,
`--full 0` is uncapped. `thread`/`doc`/`neighbors` already print full bodies; pass `--full N`
there to *cap* them. Reach for `--full` the moment a preview looks cut off.

- **`mari search "<q>"`** — hybrid semantic + keyword, fused with weighted RRF. Keyword fusion
  means exact tokens (identifiers, error strings, ticket numbers, `#channels`) are found even
  when not semantically close. Flags:
  - `--variant "<q2>"` (repeatable) — **fuse extra query variants** with the main one. This is
    your lever: pass a paraphrase *and* the raw identifiers you spotted (e.g. main
    `"how billing works"` plus `--variant "ZQ-9001"`). You are the query-expansion step — Mari
    doesn't call an LLM for it, you do.
  - `--author <substr>` `--since YYYY-MM-DD` `--before YYYY-MM-DD` — narrow by who/when.
  - `--doc <substr>` (id/title contains), `--expand N`, `--k N`.
  - `--tag <status>` / `--no-tag <status>` — trust filters (`--tag canonical`,
    `--no-tag deprecated`). Hits show tag badges; a `deprecated` hit prints its replacement when
    a lineage edge exists.
- **`mari recent [--source S] [--doc <substr>] [--author <substr>] [--since …] [--before …]`** —
  most recently changed docs/messages. For "lately", "this week", "what did Ada change".
  `--doc` matches title *and* id, so `recent --author eric --doc "<doc title>"` answers "show me
  eric's most recent comment on my doc" (Google Drive indexes each comment as its own
  author-attributed, timestamped entry).
- **`mari related <ref>`** — documents one hop away in the graph: same repo/project/channel or
  same author, and directly linked docs. Each hit says *why* (`reason`).
- **`mari thread <ref>` / `mari doc <ref>`** — a whole Slack thread / document as one block
  (full body by default), matched by id or title substring.
- **`mari neighbors <chunk-id>`** — the chunks surrounding a specific hit (each hit has an `id`).
- **`mari sql "<SELECT …>"`** — read-only SQL (SELECT/WITH only) over the workspace store:
  tables `documents`, `edges`, `tags`, `state`, and the `_chunks` dataset. For structured
  questions retrieval can't phrase — counts by author, comments by a person, dates. `mari sql`
  with **no query prints the schema**. `--global` targets the shared cross-repo store.
- **`mari explore "<q>" | <file>`** — RAG search over the *current repo's own content* (distinct
  from `search`, which queries connected sources). First run embeds the repo (minutes on a big
  repo — warn the user); after that queries are fast and the index self-maintains from git.

Ranking is config-driven (`mari config set search.*`): `recency_decay`, `merge_sections` (on by
default), `auto_weight` (identifier queries lean keyword, questions lean vector), `tag_boosts`
(canonical up, stale/deprecated down), and an optional local `rerank.enabled` cross-encoder.
Index-shape knobs (`chunking.*`, the reranker) take effect after `mari sync --rebuild`.

**Worked example** — "I had a convo in the product channel, what's the impact on my docs?":
1. Pull the conversation: `mari recent --source slack --doc product`
2. Read it; extract the concrete topics/decisions/identifiers.
3. Find affected docs: `mari search "<topics>" --source gdocs --expand 1`
4. If a hit looks central, pull the whole thing (`mari doc <title>`), then answer, **citing each
   source by title and URL**. If nothing relevant comes back, say so rather than inventing; if
   the index is empty, point the user at `/mari init search`.

## `sync`

`mari sync [source] [--rebuild] [--since N]` — fetches changes and re-embeds **only what
changed**. `--rebuild` ignores cursors and re-fetches back `--since` days, re-embedding every
doc — run it to apply a chunking or embedding-model change. Sync is resumable: the embed phase
checkpoints per document.

**Never run `sync` on your own.** It is the one command you do not run unprompted — it hits the
user's live services and can take minutes. When a read command prints a staleness warning (or
`status` reports stale), **tell the user their index looks stale and suggest `/mari sync`** —
then wait for them to ask. Still answer their question from the current index; just flag that it
may be behind.

## `status` / `config` / `features`

- `mari status` — connections, tracked sources, index counts, embedding model (warns if the
  index was built with a different model), detector style guide + hook state, tag counts.
- `mari config list` — resolved settings; `config get <path>` / `config set <path> <value>`.
  Chunking is **per-source** (`mari config set slack.chunking.lines 15`). Changing the embedding
  model or chunking prints a `sync --rebuild` reminder.
- `mari features` — the grouped capability list (detect, rewrite, clarity, polish, authoring,
  verify, knowledge, curation, setup, configure). Use it to answer "what can Mari do?"; route to
  the specific command once the user picks one.

## Tracking refs

`mari track add <source> <ref>` is the routing command — it parses the ref (URL, `#channel`,
`owner/repo`, `PROJ`, path), normalizes it, and asks whether it's personal or team-shared. Refs
for the **whole team** go into the repo's committed `.mari/config.json` — everyone who clones
inherits them, and each person authenticates with their own credentials. `mari track list`
shows what's tracked and from which config layer; `mari track remove` untracks (next sync
prunes). Then suggest `/mari sync`.

## Curation — what should Claude trust?

- **`mari tag <path-or-ref> <status>`** — statuses: `canonical`, `stale`, `deprecated`, `draft`,
  `internal`, `customer-facing`, `needs-review`. Tags live in the catalog `tags` table and ride
  the shared warehouse; they boost or bury search results, gate factcheck evidence, and drive hook
  advisories. Load `references/reference-tag.md`. When tagging `deprecated`, add
  `--superseded-by <ref>` to record the successor (a confirmed `replaces` lineage edge) so the
  replacement pointer shows on search hits.
- **`mari tag analyze [path…]`** — bulk auto-tagging (§10.4): the CLI extracts deterministic
  context cards, you judge from them, ask the user grouped questions, and apply the agreed tags.
  Default posture is *untagged = current* — hunt only for docs needing special treatment. Load
  `references/reference-tag-analyze.md`. User-triggered only; never runs from a hook or sync.
- **`mari facts add "<fact>" [--source "<ref>"]`** / `mari facts list` — the FACTS.md ledger that
  `factcheck` grounds against.
- **`mari extract facts [--source S] [--doc D] [--since N]`** — mine candidate facts from recent
  knowledge (e.g. `#product` Slack messages); review each with the user before writing. Load
  `references/reference-extract.md`.
- **`glossary`** — harvest approved terms + variants into STYLE.md (feeds the
  `terminology-consistency` rule). Load `references/reference-glossary.md`.
- **`mari audit kb [path…]`** — audit the knowledge base: stale pages, contradiction candidates,
  duplicates, unsupported claims, inconsistent terminology, the `needs-review` backlog.

## Team sharing: `.mari/` + storage backends

- A committed `.mari/config.json` declares team sources + settings. A clone that hasn't authed a
  declared source is skipped with a nudge at sync — not an error. Credentials never enter the repo.
- **Storage backends** for the index: local (default, `~/.mari`), **git** (`mari cloud init
  --backend git` — catalog committed at `.mari/catalog`, data files on Git LFS; teammates get the
  index by `git clone`), or **s3** (`mari cloud init --bucket …` / `mari cloud connect`).
- Shared backends have **one writer** (clones default to read-only consumer; `mari cloud role
  writer` takes over). After a git-backend sync, remind the user to commit `.mari/` — committing
  is the user's move, like sync itself.

## Editorial commands

The most-used verbs (each has a `reference-<command>.md` with its full flow):
**`deslop`** (signature — strip AI tells; `--narrative` adds the discourse tier), **`audit`**
(report grouped by family, no edits), **`tighten`**, **`understate`**, **`clarify`**,
**`critique`**, **`polish`**, **`init`**, **`document`**. More, grouped by intent: build —
`docsite draft outline glossary`; refine — `sharpen soften harden`; external — `humanize`;
enhance — `voice cadence format delight`; channel — `adapt localize`; iterate — `live`;
verify — `factcheck` (the deep `--decompose` tier has *you* split sentences into atomic claims
in-session — see `references/reference-factcheck.md`).

## Management

- `mari install [--providers=…]` — wire the post-edit hook + skill for this project.
- `mari hooks status | on | off` — hook state; `mari ignores add-rule|add-file|add-value …` —
  detector waivers (config JSON only; no inline in-file comments). For finer control than an
  all-or-nothing file waiver, `detector.ignoreSpans` (`{path: [[startLine,endLine], …]}`) waives
  findings within a line range — so a file that deliberately demonstrates slop can waive just
  those spans while genuine violations elsewhere stay visible.
- `mari rules add <name> --paths "<globs>" --notify "<msg>" [--exclude …]` — edit-notify rules;
  `rules discover` proposes code↔docs couplings.
- `mari nudge add <name> --when "<glob>[#symbol]" --edit "<file>[#symbol]" [--edit …] [--message …]`
  — directed edit obligations: when the `--when` file (or symbol span — a code function/class or
  a markdown heading) is edited, the hook tells you to edit every `--edit` target now. "Whenever
  X changes, update Y" → compose this. `nudge check` verifies endpoints still resolve (CI).
- `mari zero add|remove|list <rule-id>` — per-rule zero tolerance (fires on first occurrence,
  bypassing density gates; e.g. `zero add em-dash-overuse` bans em-dashes outright).
- `mari pin <command>` / `mari unpin <command>` — expose a verb as a standalone slash command
  (`/search`, `/deslop`, `/tag`, `/sync`, …).
- **Need a source that isn't core?** Author a connector plugin from the bundled template into
  `~/.mari/plugins/`; `mari plugins` shows what's loaded.

## Always

- The detector is deterministic and never claims a document "is AI-written." Findings are leads,
  not verdicts — `advisory` especially. Preserve the author's meaning and voice; de-slopping is
  rewriting, not deletion.
- Cite retrieval answers by source title and URL. Never conclude from a truncated preview.
- Never run `sync` unprompted. Setup is assistant-guided end-to-end, with the privacy path
  always available.
