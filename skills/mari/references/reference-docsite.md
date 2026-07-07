# docsite — document an entire codebase, end to end

One command that takes a repo from "no docs" to a full, professional documentation website:
platform chosen and scaffolded, information architecture derived from the code, every page
written, community-health files in place, and everything validated. The deterministic halves
live in the CLI (`mari platform`, `mari asset`, `mari check`); the survey, architecture, and
writing are yours. Work phase by phase and report progress between phases — this is a long
task, and the user should see the site take shape.

Load `reference-register-docs.md` before writing any page, and `PRODUCT.md` if it
exists (run `init` first if the user wants voice context; `docsite` works without it, using
the docs register defaults).

## Phase 1 — survey the codebase

Read before you plan. Build an inventory the architecture will hang off:

- **What it is:** README, package manifest (`package.json`, `pyproject.toml`, `Cargo.toml`,
  `go.mod`, …), the top-level directory layout.
- **Public surface:** run `mari surface` — it extracts every exported/public
  symbol (JS/TS, Python, Go, Rust) with its signature and file:line. Add to it what the
  extractor can't see: CLI commands and flags (bin entry points, usage/help text), config
  files and their options, environment variables.
- **How it runs:** install steps, build/serve commands, test commands, required runtimes.
- **What already exists:** every `*.md` in the repo (README, CHANGELOG, design docs, todo
  notes) — existing prose is source material and voice reference, never overwrite it blindly.

Write the inventory down (a scratch list is fine). Every docs page you plan must trace back
to something in it — pages invented without a source in the code become fiction.

## Phase 2 — choose the platform

Follow `reference-platform.md`:

1. `mari platform detect` — if a site generator already exists, use it; don't
   scaffold a second one.
2. If none: recommend a platform from the repo's ecosystem (Node → Docusaurus, Python →
   MkDocs/Sphinx, Rust → mdBook, Go → Hugo; MkDocs Material is the safe default) and **ask
   the user to pick**. Confirm the site title.
3. `mari platform scaffold <id> --name "<title>"`.

## Phase 3 — information architecture

Design the site on the Diátaxis frame (https://diataxis.fr) — four kinds of pages, kept
separate because they serve different reader modes:

| Section | Reader is… | Pages you derive from |
|---------|------------|----------------------|
| **Tutorial / Getting started** | learning by doing | install steps, quick-start path, first success in ≤10 minutes |
| **How-to guides** | solving a task | each real workflow the tool supports (one guide per task) |
| **Reference** | looking something up | CLI commands/flags, API surface, config options, file formats — exhaustive, from the code |
| **Explanation** | understanding why | architecture, key concepts, design decisions |

Scale to the project: a small CLI might need one page per section; a platform needs trees.
Then:

1. Write the nav (in `mkdocs.yml`, `SUMMARY.md`, `_sidebar.md`, the toctree, …) for the full
   plan — the whole skeleton, in reading order.
2. Create every page as a **stub with a brief**: the H1, a one-line purpose, and an HTML
   comment listing audience, outline, and the exact source files/commands the page draws
   from. The briefs are what make the fill phase grounded instead of improvised.
3. Show the user the proposed tree before filling it. This is the cheap moment to reshape
   the site; after Phase 4 it's expensive.

## Phase 4 — fill every page

For each stub, in nav order:

1. Read the source files named in the brief — code is the ground truth for every claim,
   command, and flag on the page. Copy-pasteable commands must be real ones.
2. Write the page fully in the docs register (second person, imperative, present tense, one
   idea per paragraph). Respect the Diátaxis mode: no explanation digressions inside
   tutorials, no task steps inside reference tables.
3. Delete the brief comment, then run `mari detect <page>` and fix what it
   flags before moving on — don't batch the cleanup to the end.

Report progress every few pages ("6/14 pages written"). If the codebase contradicts the
README or itself, flag it to the user rather than guessing.

## Phase 5 — community-health files

The repo-level files a project should carry (GitHub's community standards). For each one
missing, scaffold and fill it from the Phase-1 inventory:

- `mari asset scaffold contributing "<project>"` → real setup/test/PR commands, not placeholders.
- `mari asset scaffold code-of-conduct "<project>"` → needs a real reporting contact — ask the user.
- `mari asset scaffold security` → needs a real private reporting channel — ask the user.
- `LICENSE` — if missing, ask which license; never pick one for the user.
- `CHANGELOG.md` — offer; seed from git tags/releases if the user wants it.

Run `mari asset check <file>` on each until its structure passes.

## Phase 6 — validate until clean

`mari check --strict` is the gate. It validates the whole project in one
pass: every internal link and anchor resolves, the nav agrees with the files on disk (no
missing targets, no orphan pages), community files exist, and each one's structure is
complete. Fix and re-run until warn-free; advisories are judgment calls — resolve or
consciously leave them.

Two focused checks scoped to the docs site sit alongside the whole-project gate:

- `mari docsite check` — the links-only validator: internal links + anchors resolve and
  nav↔files agree, plus in-page `#anchor`→`id` links in HTML/JSX (so code-based sites are
  covered). Faster than `mari check` and the right thing to run after moving or deleting pages.
- `mari docsite sync` — reports drift between the documented surface and the real one: CLI
  commands and config keys the docs miss, and stale "N commands" count phrases in prose. Run it
  whenever the code's command surface changes; it exits non-zero on hard drift so it can gate CI.

### Removing a page

There is **no** `docsite remove` command. Delete the page like any file and drop it from the nav.
The post-edit hook fires on the deletion and reruns `mari docsite check`, surfacing every
now-dangling link and orphaned nav entry for you to repair — finding the breakage is
deterministic; healing the surrounding prose is yours.

Then:

- **Completeness + staleness (attention):** `mari check --deep --limit 0`.
  Coverage flags public symbols the docs never engage — each one is either a page to write
  or a deliberate omission to note. Grounding flags doc sentences that engage none of the
  surface — re-verify each against the code (renamed flag, removed command, invented
  behavior). Treat both as leads, not verdicts: conceptual prose legitimately floats above
  the surface. Opt-in cost: ~3s per doc/chunk; needs the native attention binary + a GGUF
  model, and degrades gracefully to a skip message without them.
- Build the site with the platform's own command (the scaffold printed it, e.g.
  `mkdocs build --strict`) so the generator verifies what Mari can't.
- If translations exist: `mari i18n conform docs`.
- Optionally `factcheck` the top-level claims against `FACTS.md` if the project keeps one.

## Phase 7 — keep it alive

Wire the maintenance loop so the site doesn't rot:

- `mari install` — post-edit hook lints every future docs edit.
- `mari rules discover` — propose code↔docs rules so code changes nudge the
  agent to update the affected pages.
- Offer a CI/pre-commit step: `mari check --strict` (plus the platform's `build --strict`)
  as the docs gate.

Close by telling the user: where the site lives, how to serve it locally, what `mari check`
gates, and any decisions still open (license, security contact).

## Always

- The user picks the platform and the license; you recommend, they decide.
- Never overwrite existing prose without reading it — fold it into the new structure.
- Every page traces to code. A page you couldn't ground in the inventory is a page to cut.
