# init — one-time project setup (search + style)

`init` has two flows. `mari init search` sets up knowledge connectors; `mari init style` writes
the editorial context files; bare `init` (or `mari init all`) runs both, search first. Setup is
a conversation — the CLI never prompts; **you** ask the questions and run the commands.

## Routing

- User said "connect my tools", "set up search", `/mari init search` → **search flow**.
- User asked for a prose-editing command with no `PRODUCT.md` (`mari context` printed
  `NO_PRODUCT_MD`), or said "set up the style system" → **style flow**.
- Bare `/mari init`, "set up mari" → ask which they want first, or run both.

## Search flow (`mari init search`)

Run `mari init search` for a detailed human listing — one entry per source with its connection
status, scope, credential file path + fields, config file path + tracked-ref list names (e.g.
`slack.[channels]`, `github.[repos]`), whether it indexes-everything-when-connected, and (for
Slack/Discord/Google Drive) the first-sync lookback. Mari ships **12 core connectors** — Git
history (the current repo, no auth, on by default), Slack, Google Drive, GitHub, Linear,
Confluence, Jira, Zendesk, Salesforce, HubSpot, Microsoft 365, Discord — plus local files,
always on. Read the output and act on it; don't memorize the list.

**Each connector has a guided `/connect-<name>` setup skill** (`connect-slack`,
`connect-github`, `connect-linear`, …) that walks scope and every auth option for that source.
When setting a source up, invoke the matching one rather than improvising the steps.

Walk the user through what's missing, one source at a time:

1. **Scope — ask for every connector.** Global (indexed once, searchable from every repo —
   Slack, personal Google Drive) or local (scoped to this repo — a GitHub project)? Set it with
   `mari scope <source> global|local`. Changing scope purges the old index — suggest `sync`
   afterward.
2. **Credential — three ways, matching the user's comfort:**
   1. **You run it** — `mari auth <provider> <fields>` (a failed `auth` names the missing fields
      and the exact page to create the token). Simplest; the token passes through you.
   2. **The user runs it** (privacy) — hand them the same `mari auth …` line so the token never
      reaches you; continue once they confirm.
   3. **Write files directly** — write the credential JSON to the path `mari init search` shows
      (keys mirror the listed fields, e.g. `{"token": "…"}`).
   Interactive sources (Google, Microsoft) open a browser/device prompt instead of taking a token.
3. **Track refs** — `mari track add <source> <ref>` (URL, `#channel`, `owner/repo`, `PROJ`,
   path). It asks: personal, or team-shared committed `.mari/config.json`? Whole-collection
   sources (Slack, Zendesk, Salesforce, HubSpot) index everything once connected; tracked lists
   only *narrow* — the `init search` output flags which.
4. **Lookback — ask when a source has one** (Slack, Discord, Google Drive). How many days of
   history on the first sync (0 = all)? Non-default →
   `mari config set <source>.lookback_days <days>` before the first sync. It only bounds the
   initial backfill; later syncs are incremental regardless.
5. **Finish** — suggest the user run `/mari sync` (never run it unprompted), then verify with a
   test `mari search`.

Team sharing and storage backends (git/S3 index, one-writer rule) are the router's "Team
sharing" section — offer them when the user says "my team should share this".

## Style flow (`mari init style`)

Write the context files every editorial command reads. This is the blocker `mari context`
routes to on `NO_PRODUCT_MD`.

1. **Ask the register** (pick one): docs / marketing / editorial / microcopy. This sets the bar
   for ceilings and tone.
2. **Ask the base style guide** (default **microsoft**; or google / ap / chicago / plain).
3. **Sample existing writing** — read 1–3 representative files (README, a docs page, UI strings)
   and infer the current voice in three adjectives. Don't impose a generic voice.
4. **Write `PRODUCT.md`** with: audience, register, voice (3-word personality), anti-references
   (what NOT to sound like), banned words, reading-grade target (if plain).
5. **Offer `STYLE.md`** — base style guide, terminology glossary (preferred term + forbidden
   variants; `glossary harvest` can seed it from the repo + knowledge base), formatting rules,
   approved/forbidden phrasings.
6. **Offer `FACTS.md`** — seed it with a few checkable product facts (`mari facts add`), or run
   `mari extract facts` against connected sources if search is already set up.
7. **Offer the hook** — run `mari install` (post-edit detector + notices).
8. **Discover rules** — run `mari rules discover --json`. It scans for code↔docs couplings (API
   surface ↔ API docs, schema/migrations ↔ data-model docs, CLI ↔ usage docs, config/env ↔
   config reference, monorepo packages ↔ per-package README). Also read the repo structure
   yourself and infer couplings the scan misses (a `proto/` dir paired with generated client
   docs, a public SDK entrypoint, a feature-flags file). For each candidate, show the user the
   paths + proposed notify message; let them keep/edit/drop it; add accepted ones with
   `mari rules add <name> --paths "…" --notify "…" [--exclude "…"]`. Don't add a rule the user
   hasn't confirmed; skip if the repo has no clear code↔docs structure.
9. **Recommend next commands** — usually `audit` then `deslop`.

### PRODUCT.md skeleton
```markdown
# PRODUCT
- Audience: <who reads this>
- Register: docs | marketing | editorial | microcopy
- Voice: <three adjectives>
- Anti-references: <brands/styles to avoid sounding like>
- Banned words: <project-specific>
- Reading-grade target: <n, or "n/a">
```

Write the file; don't lecture. Keep it short and specific to this project.
