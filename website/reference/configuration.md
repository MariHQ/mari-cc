# Configuration

Mari resolves configuration in this order:

```text
defaults → <repo>/.mari/config.json → <repo>/.mari/config.local.json
```

The committed file holds shared project settings. The local override is useful for repository-specific personal preferences.

```sh
mari config list
mari config get detector.styleGuide
mari config set detector.grammar true
```

`config set` writes `.mari/config.json`. Supported settings cover the detector style guide, grammar checking, ignored rules/files/values/reasons/spans, zero-tolerance rules, word-list overrides, hook output, edit-notify rules, nudges, and the glossary file.
