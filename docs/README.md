# Mari — Remaining Work Plan

This folder is the exhaustive plan for everything left to take Mari from its
current state (a feature-complete-against-`SPEC.md` implementation with a few
regressions and production gaps) to a shippable v1 and beyond.

It is deliberately split by concern so each area can be picked up
independently. Read `00-current-state.md` first for the snapshot, then
`01-known-issues.md` for the things that are currently broken, then the
numbered area docs in whatever order matches your priorities.

## Documents

| Doc | Scope |
|---|---|
| [`00-current-state.md`](00-current-state.md) | What is built, metrics, what works today, what the code actually is |
| [`01-known-issues.md`](01-known-issues.md) | Regressions and bugs in the current working tree — fix these first |
| [`02-production-blockers.md`](02-production-blockers.md) | Must-clear items before any external release |
| [`03-ml-tier1.md`](03-ml-tier1.md) | The last deferred model tier: NLI, machine-likelihood, slop-spans |
| [`04-scale-and-robustness.md`](04-scale-and-robustness.md) | Search scaling, concurrency, locking, migrations, resilience |
| [`05-distribution.md`](05-distribution.md) | Binary distribution, install flow, plugin packaging, versioning |
| [`06-connectors.md`](06-connectors.md) | Live connector shakedown, HTTP fixtures, per-connector risk notes |
| [`07-security.md`](07-security.md) | Model-download integrity, `trust_remote_code`, credentials, secrets, PII |
| [`08-portability.md`](08-portability.md) | Windows, Linux, CUDA, CPU-only, model-runtime portability |
| [`09-testing-ci.md`](09-testing-ci.md) | §19 quality bars, false-positive budget, CI matrix, real-inference tests |
| [`10-community-and-user-docs.md`](10-community-and-user-docs.md) | README/LICENSE/etc, humanizer URL, end-user documentation |
| [`11-deferred-and-nice-to-have.md`](11-deferred-and-nice-to-have.md) | Explicitly out-of-scope items, future features, polish |

## How items are described

Each work item uses a consistent shape:

- **What** — the concrete change.
- **Why** — the user- or production-facing reason.
- **Where** — file/function pointers into the codebase.
- **Acceptance** — how you know it's done (test or observable behavior).
- **Effort** — rough size: **S** (< half a day), **M** (1–3 days), **L** (a week+), **XL** (multi-week).
- **Priority** — **P0** (blocks release), **P1** (should ship in v1), **P2** (fast-follow), **P3** (later).

## Priority summary (the short version)

**P0 — blocks a working build**
1. Reconcile the embedding regression: the committed tree writes 0 vectors (`01-known-issues.md` #1).
2. Commit the uncommitted working tree cleanly and re-establish a green `main` (`01-known-issues.md` #2).

**P0 — blocks external release**
3. LICENSE + README + community-health files; Mari currently fails its own `mari check` (`02`, `10`).
4. Prebuilt cross-platform binaries + an install path the plugin can point at (`05`).
5. Real humanizer upstream URL (currently a placeholder guess) (`10`).
6. Model-download integrity (pinned revisions + checksums) and `trust_remote_code` disclosure (`07`).

**P1 — should be in v1**
7. Live shakedown of the 11 cloud connectors against real accounts + replayable fixtures (`06`).
8. Workspace locking + schema migrations (`04`).
9. CI matrix with real-inference job and a hard false-positive budget (`09`).

**P2/P3 — fast-follow and later**
10. ML tier 1 (NLI/machine-likelihood/slop-spans) (`03`).
11. Search ANN + inverted-index scaling past ~100k chunks (`04`).
12. Windows support (`08`).
13. Resident model sidecar to amortize load latency (`04`).

The rest is enumerated in the area docs.
