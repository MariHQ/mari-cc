# platform — set up a docs-as-code site

Stand up a documentation-site generator (MkDocs, Docusaurus, Sphinx, Hugo, Jekyll, mdBook,
Antora, Docsify) in a repo that doesn't already have one. Deterministic scanning and file-writing
happen in the CLI (`mari platform …`); the *choice* of platform is a conversation you run with the
user. Never scaffold a platform the user hasn't picked.

## Flow

1. **Detect first.** Run `mari platform detect`. If it reports a platform is already
   set up, stop — the repo is already docs-as-code. Tell the user what's there (e.g. "This repo
   already uses MkDocs — `mkdocs.yml`") and offer editorial help instead (`audit`, `deslop`). Do
   **not** scaffold over an existing setup unless the user explicitly asks.

2. **If none is detected, ask the user which platform they want.** Present the choices from
   `mari platform list` and let them pick one. Match the recommendation to the repo:

   | Repo signal | Lean toward |
   |-------------|-------------|
   | Node.js / React project (`package.json`) | **Docusaurus** (or **VitePress** / **Starlight**, which you'd set up by hand) |
   | Python project (`pyproject.toml`, `setup.py`) | **MkDocs (Material)** or **Sphinx** |
   | Rust project (`Cargo.toml`) | **mdBook** |
   | Go project (`go.mod`) | **Hugo** |
   | Wants GitHub Pages with zero tooling | **Jekyll** |
   | Wants no build step at all | **Docsify** |
   | AsciiDoc shop | **Antora** |

   Surface a recommendation, but the user decides. If they're unsure, MkDocs (Material) is the
   safest general default. Confirm the site title too (defaults to "Docs").

3. **Scaffold the chosen platform.** Run
   `mari platform scaffold <id> --name "<site title>"`. It writes a minimal, valid
   site (config + a landing page) and prints each file plus the command to serve it locally. It
   **refuses to overwrite** existing files and re-checks detection first; pass `--force` only if the
   user has confirmed they want to replace what's there.

4. **Report the next step.** Relay the `serve`/build command the CLI printed (e.g.
   `pip install mkdocs-material && mkdocs serve`) so the user can preview the site. Note any runtime
   they'll need (Python, Node, Ruby, Rust, Go).

5. **Offer to wire docs↔code rules.** A fresh docs site pairs naturally with Mari's edit-notify
   rules — offer to run `init`'s discover step (`mari rules discover`) so code
   changes nudge the agent to update the new docs.

## Notes

- The CLI is non-interactive by design — it never prompts. All the "which one?" logic is yours.
- `platform detect` recognizes more platforms than it scaffolds (VitePress, Astro Starlight,
  GitBook, Read the Docs), so it won't tell a user to add a second site next to one of those. If a
  user wants one of those specifically, set it up by hand — Mari only auto-scaffolds the eight in
  `platform list`.
- This is a deterministic-ish command: it needs no `PRODUCT.md`. Skip the editorial setup phase and
  run it directly.
