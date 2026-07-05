# Mari document scaffold templates

Built-in templates emitted by `mari asset scaffold <type>` and used by
`mari asset detect` / `mari asset check`. Each file below carries a header
comment (the spec: required sections, tone norms, detection heuristic,
structure-check rules) followed by the exact scaffold body, with its
`<placeholders>` intact.

| Template | `mari asset scaffold <type>` emits |
|----------|------------------------------------|
| [template-adr.md](template-adr.md) | Nygard/MADR-style ADR: Status/Date/Deciders metadata, Context, Options Considered, Decision, Consequences. |
| [template-postmortem.md](template-postmortem.md) | Blameless incident retrospective: Summary, Impact, Detection, Resolution, Timeline, Root Cause, Action Items (owner table), Lessons Learned. |
| [template-runbook.md](template-runbook.md) | Operational procedure: Overview, Triggers, Prerequisites, numbered Steps with expected outcomes, Validation, Rollback, Escalation. |
| [template-rfc.md](template-rfc.md) | Design proposal: Summary, Motivation, Non-goals, Design, Alternatives, Drawbacks, Unresolved Questions. |
| [template-contributing.md](template-contributing.md) | Contributing guide: How to Contribute, Development Setup, Pull Request Process, Reporting Bugs. |
| [template-code-of-conduct.md](template-code-of-conduct.md) | Contributor Covenant scaffold: Our Pledge, Our Standards, Enforcement, Scope, Attribution. |
| [template-governance.md](template-governance.md) | Project governance: Roles, Decision Making, Maintainers, Meetings, Changing This Document. |
| [template-security.md](template-security.md) | Security policy: Supported Versions table, Reporting a Vulnerability (private channel), Disclosure Policy. |

Scaffold titles: each template takes an optional title (`mari asset scaffold adr
"Use Postgres"`); when omitted, the H1 keeps the placeholder shown in the file
(e.g. `NNNN. Short decision title`, `<incident name>`, `<project>`). The
security template's H1 is `# Security Policy` bare, or `# Security Policy —
<title>` when a title is given.

## Override rule

A team file at `.mari/templates/<type>.md` replaces the built-in template for
that type — for **both** `mari asset scaffold` (the emitted body) **and**
`mari asset check` (the structure the checker validates against). Absent an
override, the built-ins in this folder apply.

## Community files (`mari community`)

The community-standards registry looks for each file in the repo root,
`.github/`, or `docs/` (case-insensitive, common extension variants accepted).

Required (core):

- **LICENSE** — never scaffolded and never authored by Mari. The text is
  fetched **verbatim** from the GitHub Licenses API
  (`https://api.github.com/licenses`, the machine-readable form of
  choosealicense.com) via `mari community license <key>`. Mari fills only the
  copyright placeholders GitHub's own picker fills (`[year]`/`[yyyy]` and
  `[fullname]`/`[name of copyright owner]`/`[name of copyright holder]`/
  `[author]`/`[name]`), leaves every other character untouched, and reports
  any bracketed placeholder it did not recognize.
- **CODE_OF_CONDUCT.md** — scaffolded from the code-of-conduct asset template.
- **CONTRIBUTING.md** — scaffolded from the contributing asset template.
- **SECURITY.md** — scaffolded from the security asset template.

Recommended:

- Issue/PR templates: `.github/ISSUE_TEMPLATE/bug_report.md`,
  `.github/ISSUE_TEMPLATE/feature_request.md`,
  `.github/ISSUE_TEMPLATE/config.yml`, `.github/PULL_REQUEST_TEMPLATE.md`.
- Special files: `.github/SUPPORT.md`, `.github/FUNDING.yml`,
  `.github/CODEOWNERS`.
- Extras: `GOVERNANCE.md` (from the governance asset template) and
  `CHANGELOG.md` (Keep a Changelog / SemVer skeleton with an Unreleased
  section).

Community scaffolds carry deliberate `<angle-bracket>` placeholders marking
exactly what the author must supply; the asset-backed ones (contributing,
code-of-conduct, security, governance) reuse the templates in this folder so
there is one source of truth per document.
