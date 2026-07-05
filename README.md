# Mari

Mari is a local-first Claude Code plugin that lets teams curate, search, and share their
product knowledge layer, and enforces prose quality on everything Claude writes. This repo is
the complete "what" specification — enough to one-shot the plugin. A companion "how" document
(language, libraries, cloud choices) comes next.

## Repo layout

```
PRODUCT.md              High-level product overview: what Mari is and why.
SPEC.md                 The master behavioral specification: every command, subcommand,
                        switch, config key, connector, retrieval mechanic, and the full
                        detector rule registry (all ~170 rules with complete word lists,
                        gates, and Rust library references).
README.md               This file.

mari-skill.md           The router skill: knowledge retrieval + editorial routing,
                        setup phase, guardrails. Merged from both prototypes' routers.

reference-*.md          44 authoritative flow docs the router loads per command
                        (the flat form of the skill's reference/ folder):
  reference-init.md       unified setup (init search — connectors; init style — PRODUCT.md/
                          STYLE.md/FACTS.md/hook/rules discover)
  reference-{deslop,tighten,understate,clarify,sharpen,soften,critique,polish,voice,
  cadence,format,delight,harden,adapt,localize,draft,outline,document,live,humanize,
  audit,narrative}.md   — the editorial verbs (register-aware; detector-driven)
  reference-register-{docs,marketing,editorial,microcopy}.md
                        — the four writing registers and their bars
  reference-asset-{runbook,adr,postmortem,rfc,contributing,code-of-conduct,governance,
  security}.md          — archetype requirements, tone norms, review rubrics
  reference-{factcheck,docsite,platform,community,lineage,scan,glossary}.md
                        — the deterministic-engine + interactive-goal flows
  reference-{tag,extract}.md
                        — the curation flows (new in Mari: trust tags, fact extraction)

connect-<source>-skill.md
                        One guided setup skill per connector (slack, github, gdocs,
                        confluence, jira, linear, zendesk, salesforce, hubspot, microsoft,
                        discord): scope question (with the per-source default),
                        click-by-click credential creation, the three credential-handling
                        paths, `mari track add`, first sync, confirm. connect-linear is
                        new (SPEC §6.13); the rest are faithful ports.

template-readme.md      Template index + rules: team override at .mari/templates/<type>.md;
                        LICENSE fetched verbatim (GitHub Licenses API), never authored;
                        core community files vs recommended extras.
template-{runbook,adr,postmortem,rfc,contributing,code-of-conduct,governance,security}.md
                        The exact scaffolds `mari asset scaffold <type>` emits, each headed
                        by an HTML comment specifying required sections, detection heuristic
                        (scoring model + threshold), and structure-check rules.
```

At package time these flat names map back to the plugin's on-disk layout: `mari-skill.md` →
`skills/mari/SKILL.md`, `reference-<name>.md` → `skills/mari/reference/<name>.md`,
`connect-<source>-skill.md` → `skills/connect-<source>/SKILL.md`, `template-<type>.md` →
`templates/<type>.md`.

## Provenance

Everything is harvested and unified from two prototypes — **bean** (knowledge connectors,
hybrid search, team sync) and **mari-cli** (deterministic prose detector, editorial skill,
hooks, grounding) — under the single `mari` CLI defined in [`SPEC.md`](SPEC.md). The skill and
template docs are the behavioral contract for the plugin's assistant-facing surface: an
implementation packages them verbatim (adjusting only install-time paths). SPEC §22 lists the
decisions made where the prototypes disagreed.

## Adaptation rules applied to the ports

- CLI: `bean <cmd>` / `python3 …/bean.py <cmd>` / `node cli/bin/cli.js <cmd>` → `mari <cmd>`.
- Paths: `~/.bean` → `~/.mari`; `.bean/` → `.mari/`; skill-internal absolute paths → relative
  `reference-<file>.md`.
- The prototypes' phantom `add` command and "write refs into the config file directly"
  convention → the real `mari track add <source> <ref>` (asks personal vs team-shared).
- `bean init` → `mari init search`; everything else (steps, credential fields, scope defaults,
  lookback windows, gotchas, guardrails) preserved verbatim.

## Invariants these docs encode (do not weaken when editing)

- Setup is assistant-guided; the user never has to run anything — with a privacy path so tokens
  need never reach the assistant.
- `sync` is never run unprompted; staleness is flagged, the answer still comes from the current
  index.
- The detector reports, never autofixes; findings are leads, not verdicts; Mari never claims
  text "is AI-written".
- Never conclude from a truncated preview; cite retrieval answers by title and URL.
- Licenses are copied verbatim, never generated; scaffolds never overwrite.
