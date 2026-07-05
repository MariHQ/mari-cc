---
name: connect-microsoft
description: Complete click-by-click setup for connecting Microsoft 365 to Mari — pick scope, sign in with device code (nothing to install) or reuse an existing az CLI session, track drives/mail/teams, and sync. No Azure app registration or admin consent needed. Use when the user wants to connect or add Microsoft 365 as a Mari source.
version: 0.1.0
user-invocable: true
argument-hint: (guided Microsoft 365 setup)
allowed-tools: Bash
---

# Connect Microsoft 365 to Mari

This walks the user through connecting **Microsoft 365** end to end: OneDrive/SharePoint files, Outlook mail, and Teams channel messages. Run every Mari command yourself via Bash:

```
mari <cmd>
```

Work the four steps in order. Steps 2 and 3 are a walkthrough — read them, pick a method with the user, then drive the commands in step 4.

## 1. Scope — global or local?

Ask: **"Search Microsoft 365 from every repo (global) or just this one (local)?"**

- **global** (default) — one shared index, searchable from any repo. Microsoft 365 is your personal work/school or personal account, so you almost always want it everywhere.
- **local** — scoped to the current repo only. Pick this if these files/mail/Teams belong to one project.

You set this in step 4 with `mari scope microsoft global|local`.

## 2. Connection methods

Mari signs in as a **delegated** user — it can only read what your account can already see. It uses the **public Azure CLI client id** (`04b07795-8ddb-461a-bbee-02f9e1bf7b46`), the same well-known client the official Azure CLI uses, so:

- There is **no Azure app registration** to create.
- There is **no client secret** to manage.
- There is normally **no admin consent** step — you consent for yourself at sign-in. (Only exception: if your tenant admin has locked down the Azure CLI client or requires admin approval for delegated permissions, ask them to allow it. Nothing you can change from Mari's side.)

The delegated read scopes Mari requests:

| Scope | Grants read of |
|---|---|
| `Files.Read.All` | OneDrive + SharePoint files |
| `Mail.Read` | Outlook mail |
| `Chat.Read` | Teams chat |
| `Sites.Read.All` | SharePoint sites |
| `User.Read` | your basic profile |

Two ways to sign in — both interactive, neither puts a token on the command line. Ask which the user prefers:

- **(A) Device code** (default, **recommended**) — nothing to install. Mari prints a URL and a short code; you enter the code in a browser and sign in. Mari stores a refresh token and quietly renews access from then on.
- **(B) Azure CLI (`az`)** — if you already use the Azure CLI, Mari rides your existing `az login` session and mints Graph tokens through `az` on each sync. Nothing is stored by Mari beyond "use az".

## 3. Get the credential

### Method A — Device code

There is no token to fetch: the browser sign-in *is* the credential. The flow (you run the command in step 4, the user does the browser part):

1. Mari prints a line like: `To connect Microsoft, open https://microsoft.com/devicelogin and enter code: ABCD-EFGH`.
2. In a browser, open **https://microsoft.com/devicelogin**.
3. Type the **code** Mari printed and click **Next**.
4. Sign in with the **work/school or personal account** that has the files, mail, and Teams you want indexed.
5. On the **permissions** screen, review the read scopes and click **Accept**.
6. When the browser says you're signed in, Mari finishes automatically (it's polling) and stores a refresh token.

Because the user must open the URL and type the code, either you run `mari auth microsoft` and relay the printed URL + code to them, or a privacy-minded user runs that command themselves in their own terminal. The device-code window is short (about 15 minutes) — if it expires, just run the command again.

### Method B — Azure CLI (`az`)

1. Install the **Azure CLI** if it isn't already: see **https://learn.microsoft.com/cli/azure/install-azure-cli**. On macOS: `brew install azure-cli`.
2. Sign in once: run `az login` in a terminal. This opens a browser; sign in with the account that has the content you want. Confirm it worked with `az account show`.
3. That's it — Mari will call `az account get-access-token --resource https://graph.microsoft.com` on each sync to get a Graph token. No token is copied or stored by Mari.

## 4. Connect, scope, sync

1. **Authenticate.**
   - Device code: `mari auth microsoft` — then follow the printed URL + code from step 3A.
   - az CLI: `mari auth microsoft --method az` — must already be logged in via `az login`.
   No token passes through you; the sign-in happens in the browser (device code) or in the existing az session.

2. **Set scope** from step 1: `mari scope microsoft global|local`.

3. **Track what to index.** Run `mari track add microsoft <ref>` for each item — it adds the ref to the `microsoft.[drives]` / `[mail]` / `[teams]` lists, asking whether the ref goes in your personal config or the team-shared committed config. Formats:
   - **Files** — add a drive/folder root to `[drives]`. Use `me` for your own OneDrive root, or a drive id for a SharePoint document library. Track a specific item as `ms:file:<itemId>`.
   - **Mail** — add a mail folder to `[mail]`, e.g. `ms:mail:inbox` (or just `inbox`).
   - **Teams** — add a channel to `[teams]` as `ms:teams:<teamId>/<channelId>`.
   Add at least one source or the sync has nothing to fetch.

4. **Sync.** `mari sync microsoft` — builds the index. What's indexed, one document each:
   - **OneDrive/SharePoint files** — body text extracted (Office, PDF, HTML, text). Files prune when deleted upstream.
   - **Outlook mail** — one document per conversation/thread. Threads never prune.
   - **Teams messages** — one document per channel message. Messages never prune.
   (There is no first-sync lookback window to set for Microsoft.)

5. **Confirm.** `mari status`, then a test query: `mari search "<topic>" --source microsoft`.
