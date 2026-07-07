# community — set up the GitHub community standards

Walk a repo through GitHub's community-standards set — license, code of conduct, security policy,
contributing guide, issue and PR templates, support, funding, code owners, governance, changelog —
in one interactive session. This is a **goal**, not a one-shot generator: you ask the user about
each file and hand them a starting point they control. You do **not** invent policy, contacts,
URLs, or license text.

Two hard rules, both from how these documents actually work:

- **The license is COPIED, never generated.** License text is legal boilerplate that must be
  byte-for-byte correct. `mari community license <key>` fetches the verbatim body from the GitHub
  Licenses API (the source behind choosealicense.com) and fills only the copyright line. Never write,
  paraphrase, summarize, or "clean up" a license yourself, and never let the model author one.
- **Everything else is a template the user fills.** Scaffolds carry `<angle-bracket>` placeholders
  marking what the author must supply — a real security contact, funding links, CODEOWNERS handles.
  Present each one for the user to fill; don't fabricate the values.

The CLI (`mari community …`) is deterministic and never prompts. Every "which one?" and "what should
this say?" is a conversation you run — use `AskUserQuestion` for the choices below.

## Flow

1. **Survey what's already there.** Run `mari community status`. It lists all
   thirteen artifacts with present/missing and the required ones. Don't touch anything that already
   exists unless the user asks — offer editorial help on it instead (`audit`, `deslop`).

2. **Ask which artifacts to set up.** Show the missing ones (`community list` describes each) and let
   the user pick with `AskUserQuestion` (multi-select). Recommend the required core — license, code
   of conduct, security, contributing — but the user decides the scope. Don't set up all thirteen by
   default; a two-person project rarely wants GOVERNANCE.

3. **License — the copy path.** This is the one the user asked to control most, so slow down here:
   1. Show the options: `mari community licenses` (the set GitHub offers). If the user
      is unsure, describe the common trade-off in plain terms — permissive (MIT, Apache-2.0, BSD) vs.
      copyleft (GPL, AGPL, MPL) vs. public-domain (Unlicense, CC0) — and point them at
      choosealicense.com. **The user picks. Never pick for them.**
   2. Ask the **copyright year** (default the current year) and the **copyright holder** (person,
      company, or "The <Project> Authors"). These are the only fields you fill.
   3. Write it: `mari community license <key> --year <yyyy> --holder "<name>"`. The CLI
      prints which SPDX license it copied and any placeholder it left for the user (e.g. Apache's
      appendix). Confirm the pick with the user **before** writing — this is a legal choice.
   4. If the user names a license GitHub's API doesn't carry, don't improvise the text. Point them to
      choosealicense.com / the SPDX list and have them drop the file in, or pick a carried one.

4. **Code of conduct.** Scaffold the Contributor Covenant:
   `mari community scaffold code-of-conduct --project "<name>"`. Then **ask the user
   for the reporting contact** (an email or a link) and fill the `<contact>` placeholder — a code of
   conduct with a placeholder contact is not enforceable. Keep the Contributor Covenant attribution.

5. **Security policy.** Scaffold: `community scaffold security`. Ask, and fill in:
   - the **private reporting channel** (a security email, or GitHub's "Report a vulnerability"
     advisory — never "open an issue"),
   - which **versions are supported**,
   - a **response-time** the user can actually commit to.

6. **Contributing guide.** Scaffold: `community scaffold contributing --project "<name>"`. Fill the
   setup, test, and PR commands from the repo (read `package.json` / `Makefile` / CI). Link the code
   of conduct rather than restating it.

7. **Issue and PR templates.** Scaffold the ones the user wants:
   - `community scaffold issue-bug`, `community scaffold issue-feature` → `.github/ISSUE_TEMPLATE/`.
   - `community scaffold issue-config` → the chooser config. Ask for a **discussions/support URL** to
     route questions away from the issue tracker, and whether to disable blank issues.
   - `community scaffold pull-request` → `.github/PULL_REQUEST_TEMPLATE.md`. Trim the checklist to
     what this project actually enforces.

8. **Support, funding, code owners** (ask first — these are optional and repo-specific):
   - `community scaffold support --project "<name>"` — fill the help channels.
   - `community scaffold funding` — ask for the sponsor handles (GitHub Sponsors, Open Collective,
     custom URLs). Leave it out entirely if the project takes no funding.
   - `community scaffold codeowners` — ask who owns what; fill real `@user`/`@org/team` handles. An
     unfilled CODEOWNERS silently requests review from nobody.

9. **Governance and changelog** (usually only for larger projects):
   - `community scaffold governance --project "<name>"` — roles, decision-making, the maintainer bar.
   - `community scaffold changelog --project "<name>"` — a Keep a Changelog skeleton.

10. **Validate.** Run `mari community status` again to confirm coverage, then
    `mari check` — it link-checks and structure-checks the community docs (missing
    required sections, broken links). Fix what it flags. Finally, run the detector on the prose files
    you filled (`detect CONTRIBUTING.md SECURITY.md …`) so the copy you added isn't slop.

## Notes

- **Never overwrite.** Both `license` and `scaffold` refuse to clobber an existing file; pass
  `--force` only after the user has said to replace it. On a repo that already has some of these,
  work only on the gaps.
- **Placeholders are the contract.** After filling, grep for leftover `<…>` and `[…]` and either fill
  them with the user or call them out — a shipped placeholder is worse than a missing file.
- **Paths follow GitHub's conventions.** Issue/PR/support/funding/CODEOWNERS land in `.github/`;
  LICENSE, CODE_OF_CONDUCT, CONTRIBUTING, SECURITY, GOVERNANCE, CHANGELOG at the repo root. GitHub
  finds all of them either place; the scaffolds use the conventional home.
- **This is deterministic setup — no `PRODUCT.md` needed.** Skip the editorial setup phase; the only
  editorial work is filling the placeholders in the user's voice afterward.
- `community license` and `community licenses` need network (they hit the GitHub API). Everything
  else is offline.
