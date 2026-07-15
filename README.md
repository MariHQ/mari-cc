# Mari

[![Claude Code plugin](https://img.shields.io/badge/Claude_Code-plugin-D97757)](https://github.com/MariHQ/mari-cc)
[![Latest release](https://img.shields.io/github/v/release/MariHQ/mari-cc?display_name=tag&sort=semver)](https://github.com/MariHQ/mari-cc/releases)
[![173 prose rules](https://img.shields.io/badge/prose_rules-170+-2E8B57)](#rules)
[![49 word lists](https://img.shields.io/badge/word_lists-49-1E6FA8)](#rules)
[![Local first](https://img.shields.io/badge/local-first-0A7B83)](#local-by-default)
[![MIT license](https://img.shields.io/github/license/MariHQ/mari-cc)](LICENSE)


```sh
/plugin marketplace add MariHQ/mari-cc
/plugin install mari@mari
```

Mari is an AI prose manager for Claude Code. It catches weak writing as Claude
edits, rewrites it in your project's voice, and enforces your house style.

The detector is deterministic and runs locally. It flags concrete passages and
named rules instead of guessing whether text "sounds AI-written." Then Claude handles
the rewrite.

## Manage AI-written prose

- **Catch problems after every edit.** The Claude Code hook checks new prose for
  AI slop, unclear language, grammar, inclusive language, and house-style
  violations while the writing is still in context.
- **Rewrite with editorial intent.** Use `/deslop`, `/tighten`, `/clarify`,
  `/sharpen`, `/understate`, `/critique`, and `/polish` instead of asking for a
  vague "make this better" pass.
- **Enforce your voice.** Choose Microsoft, Google, AP, Chicago, or plain style.
  Add project terminology and forbidden phrasing, then configure waivers and
  zero-tolerance rules.
- **Keep documentation current.** Edit-notify rules, localization checks, and
  nudges tell Claude what else must change with an edit.

## Rules

Mari's 170+ deterministic rules and 49 configurable word lists identify the
passage, the problem, and the applicable style guidance. The Rules console
shows the complete catalog, project waivers, zero-tolerance rules, and
edit-notify rules in one place.

![The Mari Console Rules view showing 49 word lists, the streamlined navigation, detector families, and rule controls](assets/mari-console-rules.png)

Run `/mari console --open` in Claude Code to open the web console.

## Glossary

Keep preferred terms and discouraged variants in the `Terminology` table in
`STYLE.md`. For example, this repository uses `dataset`, not `data set`:

```markdown
| Use | Not |
|---|---|
| dataset | data set |
```

Add an approved term from the CLI with
`mari glossary add dataset --not "data set"`. `mari glossary list` prints the
active glossary, and the detector flags discouraged variants in new prose.

## Templates

Mari includes templates for runbooks, architecture decision records,
postmortems, requests for comments, contributing guides, codes of conduct,
governance documents, and security policies. Scaffold a document, fill in its
placeholders, then check that its required sections are present:

```sh
mari asset scaffold runbook "Restore the API"
mari asset check RUNBOOK.md --strict
```

Use `mari asset detect <file>` when you are unsure which template matches an
existing document. To enforce a team-specific structure, add
`.mari/templates/<type>.md`. Mari uses that file for both scaffolding and
structural checks. The console's Templates panel lists every available type,
its output file, required sections, and source standard.

## Localization

Mari recognizes common documentation layouts, including `README.es.md`,
language directories such as `docs/{en,fr}/`, Hugo's `content.zh`, and
Docusaurus `i18n/<lang>/...` trees.

Ask Claude "Are the translations in sync?" to run the conformance workflow.
For a repository-wide check, use `/mari i18n conform docs`.

### Remind Claude to update related docs

Add a nudge when a source change should prompt a specific documentation edit:

```sh
mari nudge add cli-docs \
  --when "src/main.rs" \
  --edit "website/reference/cli.md" \
  --message "Update the CLI reference for this change."
```

The post-edit hook shows the nudge whenever a matching file changes. Run
`mari nudge list` to review configured nudges and `mari nudge check` to verify
their file and symbol targets.

## Local by default

Mari runs on deterministic rules and repository-local files. Project
configuration lives in `.mari/config.json` at the repository root.

## License

MIT. See `LICENSE`.
