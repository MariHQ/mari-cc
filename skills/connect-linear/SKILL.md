---
name: connect-linear
description: Complete click-by-click setup for connecting Linear to Mari — pick scope, create a personal API key, authenticate, track teams, and sync issues. Use when the user wants to connect or add Linear as a Mari source.
version: 0.1.0
user-invocable: true
argument-hint: (guided Linear setup)
allowed-tools: Bash
---

# Connect Linear to Mari

Guide the user through connecting **Linear** as a knowledge source. Mari indexes the **issues** (title, description, and comments) of the teams you track, so you can search them locally.

You (the assistant) run every Mari command via Bash:

```
mari <cmd>
```

Work the four steps in order. This is a walkthrough — be thorough, use exact button names, and confirm each step before moving on.

---

## 1. Scope — global or local?

Decide where the Linear index lives before authenticating.

- **local** (default for Linear) — the index belongs to the current repo/directory only. A Linear team is usually tied to one project, so its issues are most relevant while you're working in that project.
- **global** — one shared index, searchable from any directory on this machine. Choose this if you want to search a team's issues from anywhere (e.g. a company-wide triage team, or a team you reference across many projects).

Ask the user: **"Track this Linear team just for the current project (local), or make it searchable everywhere (global)?"** Default to **local** unless they say otherwise. You'll set the scope in step 4.

---

## 2. Connection method

Mari authenticates with a Linear **personal API key**. There is one method — the key is created in Linear's settings and passed to `mari auth linear --token <key>`. The key acts as *you*: whatever teams and issues your Linear account can see, Mari can index. **Read access suffices** — Mari never writes to Linear, so if your workspace lets you scope the key, read-only is the right choice.

Note: Linear has **no auto-indexing** — Mari only indexes teams you explicitly track. You must add at least one team (step 4) or a sync does nothing. There is no first-sync lookback window; the first sync pulls each tracked team's issues in full, then incremental syncs pull only what changed (Mari keeps a per-team `updatedAt` cursor).

---

## 3. Get the credential

Walk the user through creating the key in their browser:

1. Go to **https://linear.app** and sign in to the workspace whose issues you want indexed.
2. Open **Settings** (click the **workspace name** in the top-left corner, then choose **Settings** — or press **G** then **S**).
3. In the left sidebar, under your account section, click **Security & access**.
4. Scroll to the **Personal API keys** section.
5. Click **New API key**.
6. Enter a **Label** you'll recognize later, e.g. `mari`.
7. If the dialog offers permission or team restrictions, **read access suffices** — pick read-only and (optionally) limit it to the teams you plan to track. Broader keys also work; Mari only ever reads.
8. Click **Create** (the confirmation button in the dialog).
9. **Copy** the key that appears — it is **shown only once**. If it's lost, revoke it and create a new one.

Ask the user to paste the key.

---

## 4. Connect, scope, track, sync

1. **Authenticate.**
   ```
   mari auth linear --token <api-key>
   ```
   On success it prints `✓ Linear connected as <name>.` (it calls the Linear API to resolve the key's user).

   If the user is privacy-minded and doesn't want to paste the key into the chat, offer either:
   - hand them the exact line above to run in their own terminal, or
   - run `mari init search` to get the credential file path, then write the credential JSON there directly:
     ```json
     {"token": "<api-key>"}
     ```

2. **Set the scope** from step 1:
   ```
   mari scope linear global
   ```
   or
   ```
   mari scope linear local
   ```

3. **Track at least one team.** Run `mari track add linear <ref>` once per team — it adds the team to the linear **`teams`** list, asking whether the ref goes in your personal config or the team-shared committed config. Each ref is a **`linear:TEAM`** key (the uppercase prefix on issue ids — the `ENG` in `ENG-123`) **or** a full **linear.app issue or project URL** (paste straight from the browser; Mari pulls the team out of it — project URLs land in the `projects` list). Add every team the user wants indexed. Removing a team from this list later (`mari track remove linear <ref>`) prunes everything indexed under it on the next sync.

4. **Build the index.**
   ```
   mari sync linear
   ```
   This fetches every issue for each tracked team — title, description, and comments — and embeds them, one document per issue. Mari keeps a per-team `updatedAt` cursor, so later syncs only pull issues that changed (nothing is re-fetched or re-embedded unnecessarily).

5. **Confirm.**
   ```
   mari status
   ```
   then a test query:
   ```
   mari search "<topic>" --source linear
   ```
   Pick a `<topic>` you know appears in one of the tracked teams' issues to verify results come back.
