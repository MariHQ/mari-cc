# Keep docs and code in sync

Documentation rots when code moves and the docs stay behind. Mari wires a maintenance loop that catches the drift. A post-edit hook lints every change. Edit-notify rules and nudges point Claude at the counterpart that needs updating, and lineage tracks the links between spans.

## The post-edit hook

The hook runs after every file edit in Claude Code. It lints the change and surfaces findings without ever modifying files or breaking the turn. Manage it with:

```sh
mari hooks status
mari hooks on
mari hooks off
```

You can waive noise at the hook level with `mari hooks ignore-rule <id>`, `ignore-file <glob>`, or `ignore-value <rule> <value>`.

## Edit-notify rules

A rule reminds Claude to update related docs when matching code changes. `mari rules discover` scans the repo for code-to-docs couplings and proposes rules:

```sh
mari rules discover
mari rules add api-docs --paths "src/api/**" --notify "Update the API reference." --exclude "**/*.test.*"
mari rules list
```

When an edited file matches a rule's paths, the hook reminds Claude to do what the notify message says.

## Nudges

A nudge is stronger than a reminder. It is a directed edit obligation: when a file matching `--when` is edited, Claude is told to edit each `--edit` target now:

```sh
mari nudge add rate-limits \
  --when "src/limits.rs#RATE_LIMIT" \
  --edit "docs/limits.md#Rate limits" \
  --message "Keep the documented limit in step with the constant."
```

A `#symbol` scopes either side to a code symbol's definition or a Markdown heading's section. Symbols re-resolve at hook time, so nudges survive rewrites that line-based spans would not. `mari nudge check` re-verifies every endpoint, which makes it a continuous-integration gate.

## Doc-to-code lineage

The lineage graph curates span-to-span links between code and docs. Once curated, the hook fires whenever a linked span's content changes, so you update the counterpart in the same session. This is the machine-proposed counterpart to a hand-written nudge.

## Keep translations in sync

When a source-language file has translation siblings, editing it raises an i18n staleness note. Check that translations still share the source's structure with:

```sh
mari i18n conform docs
```

Add `--deep` for the attention pass that flags source passages a translation barely covers.
