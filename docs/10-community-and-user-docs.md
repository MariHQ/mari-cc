# 10 — Community Files & User Documentation

Mari has an extensive *internal* spec (`SPEC.md`) and skill/reference
markdown, but no *user-facing* documentation and none of the community-health
files it checks other projects for. It fails its own `mari check`.

---

## 10.1 — Community-health files (P0, S)

**Current.** Missing `README.md`, `LICENSE`, `CONTRIBUTING.md`,
`CODE_OF_CONDUCT.md`, `SECURITY.md`, `CHANGELOG.md`. `mari check` reports the
first three as errors.

**Fix.** Mari can scaffold most of these itself (dogfooding):
- `LICENSE` — the actual license text for the chosen license (`02` #1). Not
  scaffolded; copy verbatim.
- `README.md` — see 10.2.
- `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md` —
  `mari asset scaffold contributing|code-of-conduct|security` produces
  templated starting points; fill the placeholders.
- `CHANGELOG.md` — see `05` #5.

**Acceptance.** `mari check` on the repo returns `check: ok` (or only
advisories for the recommended files); every file is real, not a template
placeholder.

**Effort.** S.

---

## 10.2 — README (P0, S–M)

**Current.** None.

**Design.** A README that covers: what Mari is (one paragraph), install (the
chosen channel from `05`), quickstart (`mari init`, `mari sync`,
`mari search`, a `/deslop` example), the first-run model-download expectation,
the connector list, and where the full behavioral spec lives (`SPEC.md`).
Mari's own editorial tooling (`mari deslop`, `mari detect`) should be run over
it — dogfood the prose quality.

**Acceptance.** A newcomer can install and run their first search from the
README alone; `mari detect README.md` is clean.

**Effort.** S–M.

---

## 10.3 — User guide / docs site (P1, M)

**Current.** The `docsite` flow and `mari platform` scaffolding exist to
*generate* docs for other projects, but Mari has no user guide of its own.

**Design.** Use Mari's own `docsite` flow (dogfooding) to produce a user
guide covering: the command reference (§5), the connector setup walkthroughs
(already in `skills/connect-*`), the editorial vocabulary (`deslop`/`tighten`/
etc.), configuration (§4 keys), the hook, cloud sharing, and the model/OCR/
attention tiers with their costs. Publish to the chosen platform.

**Acceptance.** A hosted (or in-repo) user guide exists; `mari check --deep`
passes on it.

**Effort.** M.

---

## 10.4 — Humanizer upstream (P1, S)

**Current.** `src/curation.rs::HUMANIZER_REPO =
"https://github.com/blader/humanizer"` — a placeholder guess. See
`01-known-issues.md` #4.

**Fix.** Owner supplies the real upstream, or the `humanize` command and its
skill reference are removed from the shipped surface.

**Acceptance.** `mari humanize ensure/update/status` operate on the real
vendored skill, or the command is cleanly removed (no dangling skill
reference, no `humanize` in `mari features`).

**Effort.** S.

---

## 10.5 — SPEC ↔ implementation reconciliation (P2, S)

**Current.** SPEC.md §22 records implementation decisions and has been kept
current through this session (Qwen embedding, attention tiers, Office,
grammar, CI). After `01-known-issues.md` #1 is fixed, re-verify §22's
embedding bullet matches reality (the working tree currently contradicts it —
§22 says Qwen 1024-dim, the code says jina 768-dim stub).

**Fix.** One pass reconciling §22 against the reconciled code; add a note that
§22 is the source of truth for "what the build actually does" vs SPEC §1–21
(what the product *should* do).

**Acceptance.** §22 accurately describes every implemented-vs-deferred
decision in the shipped build.

**Effort.** S.

---

## 10.6 — Inline API docs (P2, S)

**Current.** Modules have good `//!` headers and function doc comments.
There's no generated `cargo doc` published for contributors.

**Design.** Ensure public items are documented; publish `cargo doc` for the
crate (internal contributors) if the crate is ever consumed as a library.
Low priority while Mari is a binary-only product.

**Acceptance.** `cargo doc` builds without missing-docs warnings on public
items (if that lint is enabled).

**Effort.** S.
