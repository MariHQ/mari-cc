# 06 — Connectors

All 13 sources are implemented against `SPEC.md` §6 and share the §6.0
contract (retry/backoff honoring `Retry-After`, single 401 refresh, 60s
timeouts, per-source cursors, content-hash re-embed authority, per-source
prune rules). The 11 cloud connectors are **unit-tested against recorded
payload shapes but have never touched their live services.** That is the
gap.

---

## 6.1 — Live shakedown per connector (P1, L overall — M each)

**What.** One supervised sync of each cloud connector against a real account,
fixing whatever auth quirk, pagination edge, rate-limit behavior, or API
drift surfaces. Capture the real HTTP exchanges into replayable fixtures
(e.g. `wiremock`-style) so CI keeps covering them without live credentials.

**Where.** `src/connectors/cloud/<source>.rs`; the shared HTTP client in
`src/connectors/cloud.rs`; auth in `src/authcmd.rs`.

**Per-connector risk notes (what to watch during the shakedown):**

| Connector | Highest-risk areas to verify live |
|---|---|
| **slack** | `xoxp` vs `xoxb` scope differences; `groups:read` degradation; 7-day re-scan window; thread reply folding; user-directory cache; permalink shape |
| **gdocs** | gcloud session token refresh (~50 min); Docs→Markdown export fidelity; PDF via OCR path (`ocr.rs`); comments as separate docs; folder recursion; PDF-in-Drive with the new native-text default |
| **github** | fine-grained vs classic PAT scopes; `since` cursor correctness; issues-vs-pulls `include`; comment pagination; untracked-repo prune |
| **confluence** | Cloud (email+token) vs Server/DC (PAT) auth inference; storage-HTML→text fidelity; version-based lazy body fetch; space vs page refs; prune-on-unseen only for full-space listings |
| **jira** | Cloud vs DC auth; JQL `updated >` cursor time format (`to_jql_time`); comment extraction; project prune |
| **zendesk** | incremental-export epoch cursor; help-center-disabled tolerance; brand filter; ticket comment public/internal split; never-prune semantics |
| **salesforce** | short-lived token re-auth on 401 (no refresh); SOQL for Knowledge (may be absent) + Cases; `nextRecordsUrl` pagination; never-prune |
| **hubspot** | private-app token scopes; KB tolerated-if-absent; notes HTML→text; `updatedAt` revision; cursor pagination |
| **microsoft** | device-code refresh-token rotation + write-back; Graph delta feed deletions (prune); mail per-conversation folding; Teams no-revision handling; PDF + Office extraction via the new native paths |
| **discord** | bot Message Content intent; snowflake backward pagination; text-channel-type filter; guild channel expansion; 14-day lookback floor |
| **linear** | personal API key; GraphQL cursor pagination; team key filter; comment extraction; team prune |

**Acceptance.** Each connector: a real sync produces correct docs/edges/
cursors; a re-sync is incremental (no re-fetch of unchanged); a killed sync
resumes; a recorded fixture reproduces the mapping in CI.

**Effort.** L overall; ~M per connector, parallelizable across whoever has
the accounts.

---

## 6.2 — HTTP fixture harness (P1, M)

**What.** A test harness that replays recorded connector responses so the
mapping logic (already unit-tested on hand-written JSON) is also covered
against *real* payload shapes, and so regressions in pagination/cursor logic
are caught without live accounts.

**Design.** Record real exchanges (sanitized of secrets) once during the 6.1
shakedown; store as fixtures; a `wiremock` or file-backed mock server replays
them in `#[test]`s. Keep the existing hand-written unit tests (they're fast
and cover the mapping); add the replay tests for end-to-end sync flow.

**Acceptance.** `cargo test connectors` exercises each connector's full
sync loop against recorded real responses.

**Effort.** M.

---

## 6.3 — Auth flow robustness (P1, M)

**What.** The interactive auth flows (`google` gcloud, `microsoft`
device-code) and token-refresh paths need live validation and better failure
messages.

**Where.** `src/authcmd.rs`, `src/connectors/cloud/microsoft.rs::refresh_token`,
`src/connectors/cloud/gdocs.rs::gcloud_token`.

**Notes.**
- Microsoft device-code uses the public Azure CLI client id; verify the
  refresh-token rotation write-back survives a token expiry across sync runs.
- gcloud path assumes `gcloud auth login --enable-gdrive-access`; verify the
  scope is sufficient and the ~50-min token cache behaves.
- Every `mari auth <provider>` should validate the credential against the
  service before saving (most do) and give an actionable error otherwise.

**Acceptance.** Each provider's auth completes end-to-end against a real
account; an expired token triggers exactly one refresh then a clear
re-auth prompt; credentials save at 0600.

**Effort.** M.

---

## 6.4 — Rate-limit and quota behavior (P2, M)

**What.** The shared HTTP client retries 429/≥500 up to 4 times honoring
`Retry-After`. Under a real large-workspace first sync (e.g. Slack backfill,
a big GitHub org), verify this doesn't hammer the API, respects per-service
quotas, and degrades gracefully (see `04-scale-and-robustness.md` #4).

**Acceptance.** A large first sync completes without tripping bans; sustained
429s pause-and-resume rather than fail.

**Effort.** M.

---

## 6.5 — Connector documentation accuracy (P2, S)

**What.** The 11 `skills/connect-<source>/SKILL.md` walkthroughs describe
click-by-click credential setup. After the live shakedown, reconcile any
step that differs from the real provider UI (provider UIs drift).

**Acceptance.** Each connect skill's steps match the current provider UI and
produce a credential Mari accepts.

**Effort.** S (per connector, after 6.1).
