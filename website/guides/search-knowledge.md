# Search and explore knowledge

Once a source is synced, Mari gives you several ways to read it back. Search is the everyday entry point. The other primitives pull full documents, walk the neighborhood of a hit, or follow the graph between documents.

## Search

`mari search` runs hybrid retrieval, combining vector similarity with keyword scoring and fusing the two:

```sh
mari search "why did we change pricing tiers"
```

Useful flags:

- `--full [N]` prints full bodies instead of the default five-line preview. Bare `--full` caps at 4000 characters per hit, `--full 0` is uncapped.
- `--k N` sets how many results to return.
- `--source <key>` restricts to one source, `--doc <substr>` to documents whose title or id matches.
- `--author`, `--since`, and `--before` filter by metadata.
- `--tag canonical` and `--no-tag deprecated` filter by [curation tag](curate-knowledge.md).

Mari routes the query automatically. Identifier-like or quoted queries lean on keyword matching, and natural-language questions lean on vectors. Canonical documents rank higher, stale and deprecated ones rank lower.

## Explore

`mari explore` is the skill-facing explorer. Pass a question and it delegates to the same search surface with stable output. Pass a file path and it uses that file's path, title, and symbols as the query:

```sh
mari explore "how does the hook decide what to lint"
mari explore src/hook.rs
```

The `--deep` and `--focus` flags add an attention pass that pinpoints the exact passage. They need the optional attention model and degrade loudly when it is missing.

## Pull and navigate

When you know roughly what you want, these primitives are faster than search:

| Command | Returns |
|---------|---------|
| `mari recent` | Most recently changed documents, newest first |
| `mari doc <ref>` | The full body of the best-matching document |
| `mari thread <ref>` | A whole conversation as one block |
| `mari neighbors <chunk-id>` | The chunks surrounding a hit, in document order |
| `mari related <ref>` | Documents one hop away in the graph, each with a reason |

For anything the primitives don't cover, `mari sql "SELECT ..."` runs read-only SQL over the catalog.

## Browse in the console

When you would rather click than type, the console is a local web dashboard over the same knowledge base:

```sh
mari console --open
```

It starts a server at `http://127.0.0.1:4319/console` and opens your browser. Pass `--port` to change the port. The console serves entirely from the local binary, with no external service. It covers more than search. You get an overview of the index, sources to track and sync, documents, tags, the glossary and facts ledger, lineage, edit-notify rules and nudges, detector settings, and localization. It reads the same catalog as the CLI, so anything you sync from the command line shows up here, and the reverse.

## When results are empty

An empty result prints a nudge to run `mari sync`. If you expected hits, check that the source is synced (`mari status`) and that your filters are not too narrow.
