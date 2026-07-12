# Connect your team's sources

Mari indexes the knowledge your team already produces. A source becomes searchable in three steps: authenticate, track what you want, and sync. This guide covers the shared pattern. For the exact credential each service needs, see the [Connectors reference](../reference/connectors.md).

## 1. Authenticate

Most sources need a credential. You supply it once with `mari auth`:

```sh
mari auth github --token ghp_xxx
mari auth slack --token xoxp-xxx
```

Two providers, Google Drive and Microsoft 365, run an interactive browser or device-code flow instead of taking a token. Run them with no flags:

```sh
mari auth google
```

Credentials are stored under `~/.mari/credentials/` with restrictive permissions (`0600`). They never enter the repo. A few sources need no credential at all: git history, Granola, local files, and public mailing-list archives.

## 2. Track what to index

Authenticating connects the account. Tracking tells Mari which repos, channels, or projects to pull:

```sh
mari track github add owner/repo
mari track slack add '#incidents'
mari track localfiles add ./docs
```

`mari track <source> list` shows what's tracked. Some sources auto-index once connected (Slack, Google Drive, Zendesk) and only need tracking to narrow the scope. Others (GitHub, Jira, Confluence, Linear, Discord) index nothing until you track at least one item.

## 3. Choose a scope

Every source is scoped `global` or `local`:

- **Global** sources share one index across all your repos. They live in `~/.mari/_global/`. Chat and support tools default here (Slack, Google Drive, Zendesk).
- **Local** sources index per repo. Code-adjacent tools default here (GitHub, git history, Jira).

Check or change a scope with `mari scope`:

```sh
mari scope             # list every source and its scope
mari scope github local
```

## 4. Sync

Pull everything you tracked into the index:

```sh
mari sync              # all sources
mari sync github       # one source
```

The first sync of a chat source backfills a lookback window (14 days for Slack and Discord, 30 for Google Drive). Later syncs are incremental. One source failing never stops the others. Re-run `mari sync` whenever you want fresh results, or wire it into your own cron or continuous-integration job. Mari runs no background daemon.

## Per-connector setup

Each service has its own credential type, scopes, and quirks. The [Connectors reference](../reference/connectors.md) has a row for every source. Inside Claude Code, the `connect-*` skills walk you through a single service step by step.
