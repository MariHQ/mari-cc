---
name: connect-granola
description: Complete setup for connecting Granola to Mari — reads Granola's on-device meeting-notes cache (no login, no token), optionally narrows to folders, enables transcripts, and syncs. Use when the user wants to connect or add Granola as a Mari source.
version: 0.1.0
user-invocable: true
argument-hint: (guided Granola setup)
allowed-tools: Bash
---

# Connect Granola to Mari

Guide the user through connecting **Granola** as a knowledge source. Mari indexes your Granola **meeting notes** — the AI-enhanced notes plus your own raw notes, one document per meeting — so you can search them locally alongside your other sources.

Granola is different from every other Mari connector: **there is no login and no token.** Mari reads Granola's on-device cache file directly. If Granola is installed and you've opened it, the cache is already there. That means:

- **No `mari auth granola`** — the command doesn't exist; Granola isn't an auth provider.
- **"Connected" simply means the cache file is present.**
- Granola is **macOS-only** in practice (that's where the desktop app writes its cache).

You (the assistant) run every Mari command via Bash:

```
mari <cmd>
```

Work the steps in order. Be thorough and confirm each step before moving on.

---

## 1. Confirm Granola is present

Granola writes its cache to:

```
~/Library/Application Support/Granola/cache-v3.json
```

Check it exists:

```
ls -la ~/Library/"Application Support"/Granola/cache-v3.json
```

- **File exists** → you're ready; Granola is "connected."
- **File missing** → ask the user to install the Granola desktop app (**https://www.granola.ai**), sign in, and open at least one meeting note so the app writes its cache. Then re-check.
- **Cache lives elsewhere** (custom location, or you copied it off another machine) → point Mari at it:
  ```
  mari config set granola.cache_path /absolute/path/to/cache-v3.json
  ```

Nothing leaves the machine — Mari only ever **reads** this file.

---

## 2. Scope — global or local?

Decide where the Granola index lives.

- **local** (default for Granola) — the index belongs to the current repo/directory only. Best when these notes are about the project you're working in (e.g. OSS project meetings tracked in that repo).
- **global** — one shared index, searchable from any directory on this machine. Choose this if you take meeting notes across many projects and want them searchable everywhere.

Ask: **"Search these notes just from the current project (local), or everywhere on this machine (global)?"** Default to **local** unless they say otherwise.

```
mari scope granola local
```
or
```
mari scope granola global
```

---

## 3. Choose what to index

**By default Mari indexes every note in the cache** — Granola is always-when-connected, so you don't have to track anything. Two optional narrowing/opt-in choices:

- **Folders.** To index only specific Granola folders (workspaces), track them. Each ref is a folder name, optionally prefixed `granola:` — matched case-insensitively:
  ```
  mari track granola add OSS
  mari track granola add "Customer calls"
  ```
  Once you track ≥1 folder, only notes in those folders are indexed; notes outside them are pruned on the next sync. Remove a folder later with `mari track granola remove <name>`. Leave the folder list empty to index everything.

- **Transcripts.** By default only the notes (AI-enhanced + raw) are indexed — the higher-signal content. To also index the full raw meeting transcript, turn it on:
  ```
  mari config set granola.transcripts true
  ```
  Leave it `false` to keep transcripts out of search.

Ask the user which they want; the defaults (all folders, no transcripts) are a fine starting point.

---

## 4. Build the index

```
mari sync granola
```

This reads the cache and embeds one document per note — the title, the AI-enhanced notes, your raw notes, and (if enabled) the transcript. Mari uses each note's content hash as the re-embed authority, so later syncs only re-embed notes that actually changed, and notes deleted from Granola are pruned.

Re-run `mari sync granola` (or plain `mari sync`) whenever you want to pull in new meetings.

---

## 5. Confirm

```
mari status
```

Look for the **Granola** line — it shows scope, `local` (no credential needed), tracked-folder count, and how many notes are indexed.

Then a test query:

```
mari search "<topic>" --source granola
```

Pick a `<topic>` you know came up in a recent meeting to verify results come back. The results are ordinary Mari documents — you can tag them (`mari tag`), factcheck against them, and open them with `mari doc <ref>` (refs look like `granola:<id>`).
