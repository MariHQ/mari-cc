# Connectors

Every source Mari can index, with the credential it needs, its default scope, and its first-sync behavior. For the step-by-step flow, see [Connect your sources](../guides/connect-sources.md). Inside Claude Code, the `connect-*` skills walk through a single service.

## Source matrix

| Source | Key | Auth | Default scope | First sync |
|--------|-----|------|---------------|------------|
| Slack | `slack` | User token `xoxp-` or bot token `xoxb-`, or a browser session | global | 14-day backfill |
| Google Drive | `gdocs` | gcloud browser session, no OAuth app | global | 30-day backfill |
| GitHub | `github` | Personal access token (PAT), fine-grained or classic | local | Full, no lookback |
| Git history | `git` | None (local `git log`) | local | Full history |
| Confluence | `confluence` | Cloud email + token, DC PAT, or anonymous | local | Full |
| Jira | `jira` | Cloud email + token, DC PAT, or anonymous | local | Full |
| Zendesk | `zendesk` | Subdomain + email + API token | global | Full |
| Salesforce | `salesforce` | OAuth token + instance URL | global | Full |
| HubSpot | `hubspot` | Private-app token `pat-` | global | Full |
| Microsoft 365 | `microsoft` | Device-code flow, no app registration | global | Full |
| Discord | `discord` | Bot token with Message Content intent | global | 14-day backfill |
| Linear | `linear` | Personal API key | local | Full |
| Granola | `granola` | None (on-device cache) | local | Full |
| Mailing lists | `lists` | None (public Pony Mail archives) | local | Whole archive |
| Local files | `localfiles` | None | local | Full |

## Notes that catch people out

- **Scope defaults.** Chat and support sources default to `global`, so they are shared across all your repos. Code-adjacent sources default to `local`. Change either with `mari scope`.
- **Auto-index versus tracking.** Slack, Google Drive, Zendesk, Salesforce, HubSpot, git, Granola, and local files index once connected, and tracking only narrows the scope. GitHub, Jira, Confluence, Linear, Discord, Microsoft 365, and mailing lists index nothing until you track at least one item.
- **Anonymous mode.** Jira and Confluence support an `--anonymous` credential for public Server or Data Center instances, such as the Apache trackers, whose REST endpoints are world-readable.
- **Slack session mode.** For workspaces that require admin approval to install apps, authenticate with your own browser-session token (`xoxc-`) and cookie (`xoxd-`) instead of an app token. Session credentials expire on logout.
- **What never prunes.** Zendesk, Salesforce, HubSpot, Microsoft mail and Teams, and public mailing-list threads use whole-collection semantics and never delete indexed documents.

## Local files

`localfiles` accepts folders or single files and recurses, skipping dotfiles and dot-directories. It indexes Markdown, text, HTML, Office documents (`.docx`, `.odt`, `.rtf`, `.pptx`, `.xlsx`), and PDFs. It deliberately excludes logs and source code. Legacy binary Office formats (`.doc`, `.ppt`) are unsupported.
