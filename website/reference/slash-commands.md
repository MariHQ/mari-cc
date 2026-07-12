# Claude Code slash commands

The plugin registers standalone slash commands so the most common actions are one keystroke away in Claude Code. Each maps to a `mari` command or an editorial verb. For anything not pinned as its own command, route through `/mari`.

## Knowledge

| Command | Maps to | Use it to |
|---------|---------|-----------|
| `/search` | `mari search` | Ask a question your team's knowledge would answer |
| `/sync` | `mari sync` | Re-index tracked sources |
| `/tag` | `mari tag` | Mark a document canonical, stale, deprecated, and so on |

## Grounding

| Command | Maps to | Use it to |
|---------|---------|-----------|
| `/factcheck` | `mari factcheck` | Check a document's claims against a source of truth |

## Prose

| Command | Backed by | Use it to |
|---------|-----------|-----------|
| `/audit` | `mari detect` | Get a report of prose problems with a fix each |
| `/deslop` | detector + verb | Strip AI tells, clichés, and generic phrasing |
| `/tighten` | detector + verb | Cut wordiness and filler |
| `/clarify` | detector + verb | Fix jargon, acronyms, passive voice, error copy |
| `/sharpen` | detector + verb | Cut hedges and commit to claims |
| `/understate` | detector + verb | Cut over-explanation and restated takeaways |
| `/critique` | detector + verb | Review argument, clarity, voice, reader experience |
| `/polish` | detector + verb | Final pass before publishing |
| `/draft` | detector + verb | Outline then write a new piece |

## Routing with `/mari`

`/mari` is the front door. Give it a command name (`/mari deslop README.md`) and it routes to that flow. Give it no argument and it runs the detector over the changed files, then suggests the highest-value next commands. Ask it "what can Mari do?" and it prints the capability catalog.

Every editorial verb, connector setup, and knowledge command is reachable through `/mari`, whether or not it has its own pinned slash command.
