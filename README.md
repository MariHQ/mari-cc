# Mari

[![Claude Code plugin](https://img.shields.io/badge/Claude_Code-plugin-D97757)](https://github.com/MariHQ/mari-cc)
[![Latest release](https://img.shields.io/github/v/release/MariHQ/mari-cc?display_name=tag&sort=semver)](https://github.com/MariHQ/mari-cc/releases)
[![173 prose rules](https://img.shields.io/badge/prose_rules-170+-2E8B57)](#rules-not-vibes)
[![49 word lists](https://img.shields.io/badge/word_lists-49-1E6FA8)](#rules-not-vibes)
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

## Localization

Mari recognizes common documentation layouts, including `README.es.md`,
language directories such as `docs/{en,fr}/`, Hugo's `content.zh`, and
Docusaurus `i18n/<lang>/...` trees.

Ask Claude "Are the translations in sync?" to run the conformance workflow.
For a repository-wide check, use `/mari i18n conform docs`.

## Local by default

Mari runs on deterministic rules and repository-local files. Project
configuration lives in `.mari/config.json` at the repository root.

## License

MIT. See `LICENSE`.
