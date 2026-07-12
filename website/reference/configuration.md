# Configuration

Mari is configured through files, never environment variables. The one exception is a small set of capability toggles for optional machine-learning features. This page covers where config lives and the keys you are most likely to change. Read the whole resolved config any time with `mari config list`.

## Where config lives

Effective config is a deep merge, where later layers win:

```text
defaults  ->  ~/.mari/config.json  ->  <repo>/.mari/config.json  ->  <repo>/.mari/config.local.json
```

| File | Purpose |
|------|---------|
| `~/.mari/config.json` | Global, per-user config. `mari config set` writes here. |
| `<repo>/.mari/config.json` | Committed, team-shared config: tracked refs, detector settings, tags, rules. |
| `<repo>/.mari/config.local.json` | Personal, gitignored overrides. A `null` value deletes a key. |

List-valued keys such as tracked refs union across layers. Scalars from the more personal layer win.

## Read and write config

```sh
mari config              # print the whole resolved config, annotated
mari config get search.k
mari config set search.k 12
```

`set` coerces the value to the type of the default at that path. Booleans accept `1`, `true`, `yes`, or `on`. An unknown path prints every known dotted path and exits `2`. Changing any `embedding.*` or `*.chunking.*` key prints a reminder to run `mari sync --rebuild`, because those keys change how documents are indexed.

## Key groups

The full registry is in the specification. These are the groups you will touch most often.

- **`embedding.*`**: batch size, GPU layers, auto-download, and a model path override for air-gapped installs.
- **`chunking.*`**: `lines` per window (40), `overlap` (8), and `min_chars` (40). Chat sources ship smaller defaults.
- **`search.*`**: `hybrid` (on), `k` (8), fusion and weighting constants, `recency_decay`, `merge_sections`, and `tag_boosts`.
- **`<source>.*`**: per-source blocks such as `slack.lookback_days`, `github.include`, and `granola.transcripts`.
- **`detector.*`**: `styleGuide` (microsoft), `ignoreRules`, `ignoreFiles`, `ignoreValues`, `zeroTolerance`, and the opt-in `grammar` pass.
- **`hook.*`**: `maxFindings` per file and the hook `grammar` toggle.
- **`rules` and `nudges`**: edit-notify rules and directed edit obligations.
- **`tags.*`**: the status list and stored tag entries.
- **`cloud.*`**: `enabled`, `backend` (`s3` or `git`), and the bucket settings.

## Waivers

Detector waivers live only in config JSON. There are no inline in-file disable comments. Silence a rule with `mari ignores add-rule <id>`, skip files with `add-file <glob>`, or allow a specific term with `add-value <rule> <value>`.
