# scan — Google Docs and Slack scanners (local product knowledge)

`mari scan` mirrors external product knowledge — Google Docs and Slack channels — into plain
files under `.mari/knowledge/` (gitignored). There is no server: sync pulls directly with the
user's own credentials. Once mirrored, the snapshots behave like any other file in the repo:

- `mari explore "<question>"` searches them (add `--knowledge` to search *only* them).
- `mari lineage propose` links them to code and docs; a confirmed edge is a promise the two
  stay in sync.
- `mari scan sync` detects upstream changes and reports which linked code/docs the change
  impacts. Unaddressed impacts persist, and the post-edit hook keeps surfacing them until the
  counterpart is reconciled (`mari lineage stamp`).

All commands: `mari scan …`

## 1. Connect (once per user)

```
mari scan auth google
mari scan auth slack
```

Google is an interactive wizard, built for users with no technical support:

- **Default (gcloud)** — signs in through Google's own command-line tool, so nobody creates a
  Google Cloud project or consent screen; the user just clicks Allow in the browser. If gcloud
  is missing, the wizard offers to download a self-contained copy into `~/.mari/gcloud/`
  (~100 MB, one time, no installer).
- **Own OAuth client** — a user with a `credentials.json` (from their own Google Cloud
  project) can pass `--credentials <file>` or drop it at `~/.mari/google-oauth-client.json`;
  mari runs a browser loopback flow itself. Force a path with `--method gcloud|oauth`.

Slack takes a user token (`xoxp-…`): a workspace admin creates a minimal Slack app once (user
scopes `channels:history`, `channels:read`, `users:read`) and shares the install link; each
user installs it and pastes their token. Mari validates it and stores it.

Credentials live in `~/.mari/credentials/` (mode 0600) — never inside the repo. Auth failures
always resolve the same way: re-run `mari scan auth <source>`.

## 2. Track sources (per repo)

```
mari scan add https://docs.google.com/document/d/<id>/edit
mari scan add https://drive.google.com/drive/folders/<id>
mari scan add "#eng-payments"
mari scan remove <same item>
```

Doc/folder ids land in `.mari/config.json` under `scan.google`; channels under
`scan.slack.channels` (`lookbackDays` there tunes the edit-catching window, default 14).
The config is shareable; credentials are not in it.

## 3. Sync

```
mari scan sync [google|slack] [--full] [--since <days>] [--json]
```

What one sync does:

1. Fetches changes. Google compares each doc's `headRevisionId`; Slack re-fetches each channel
   back to the lookback window (first sync is bounded by `--since`, default 90 days). The
   content hash is the final authority — a revision bump whose exported text is identical
   (comment-only edits) rewrites nothing.
2. Rewrites changed snapshots atomically. Google Docs become one Markdown file each
   (`gdocs/<slug>--<id>.md`, path stable forever); Slack becomes one digest per channel per
   ISO week (`slack/<channel>/<week>.md`) with threads as their own sections.
3. Re-embeds changed snapshots into the assoc index (when one exists — run `mari assoc build`
   once so search covers the mirror; the git-driven refresh cannot see these gitignored files).
4. Runs lineage impact over the changed files and prints every confirmed edge the external
   change broke, with the doc's title and URL. Impacts persist in the lineage DB
   (`scan_pending`) until someone reconciles them.

`--full` ignores cursors and revision ids and re-fetches everything.

## 4. The lineage loop (why this exists)

After new knowledge lands, link it:

```
mari lineage propose      # assoc + symbol candidates, mirror files included
mari lineage review       # then confirm/reject each edge
```

From then on the promise is enforced in both directions:

- **External change** → `scan sync` reports the impacted code/docs, and the post-edit hook
  reminds every session until the counterpart is updated and `mari lineage stamp <file>` runs.
- **Local change** → the hook's lineage notice names the external doc a changed span mirrors
  ("mirrors the Google Doc "Product Vision" <url>"), so the agent can tell the user which doc
  is now stale. Mari never edits the external doc — surfacing the link is the contract.

## 5. Status

```
mari scan status
```

Shows auth health per source, tracked sources, mirrored counts, per-channel cursors, and
whether unaddressed external-change impacts are pending.

## Limits worth knowing

- Google's Markdown export drops images, drawings, and smart chips; tables can be lossy. Docs
  that refuse Markdown export fall back to plain text (weaker headings/anchors).
- Slack edits and deletions older than the lookback window are missed by design; `--full`
  re-fetches. New non-Marketplace Slack apps are rate-limited hard (~1 request/minute on
  history since 2025) — first syncs of busy channels are slow; mari honors Retry-After and
  prints progress.
- `hook.knowledge: false` in `.mari/config.json` silences the pending-impact notice;
  `hook.lineage: false` silences lineage notices entirely.
