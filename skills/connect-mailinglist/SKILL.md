---
name: connect-mailinglist
description: Complete click-by-click setup for connecting mailing-list archives to Mari — pick scope, point at an Apache Pony Mail archive (default lists.apache.org), track the lists you care about, and sync. No credential needed for public archives. Use when the user wants to connect or add a mailing list (e.g. dev@flink.apache.org) as a Mari source.
version: 0.1.0
user-invocable: true
argument-hint: (guided mailing-list setup)
allowed-tools: Bash
---

# Connect mailing lists to Mari

> **Status:** the `lists` connector is **specified but not yet implemented**
> (SPEC.md §6.15). Until it ships, `mari track lists …` / `mari sync lists`
> will not exist. This skill documents the intended flow so it is ready the
> moment the connector lands; if the commands below error with "unknown
> source", tell the user the connector isn't built yet and stop.

Guide the user through connecting a **mailing-list archive** as a knowledge
source. Mari indexes public [Apache Pony Mail](https://lists.apache.org)
archives — one document per **thread** (the root message plus every reply),
so `[DISCUSS]`/`[VOTE]`/`[ANNOUNCE]` threads, FLIP design debates, and release
announcements become searchable locally.

You (the assistant) run every Mari command via Bash:

```
mari <cmd>
```

Public archives need **no credential** — there is no auth step. Work the three
steps in order; keep it a back-and-forth.

What gets indexed: for each list you track, Mari pulls every thread and stores
**one document per thread** — the root subject, plus each message's sender,
date, and plaintext body (HTML flattened, attachments dropped). You must track
at least one list or a sync indexes nothing.

## 1. Scope — global or local?

Ask: **"Search these lists from every repo (global) or just this one (local)?"**

- **local** (default) — the index lives with the current repo, searchable only
  from here. Pick this when the lists map to the project you're working in
  (e.g. `dev@flink.apache.org` alongside the Flink repo).
- **global** — one shared index searchable from any repo on this machine.

Default to **local**. You set it with `mari scope lists …` in step 3.

## 2. Which archive and which lists?

**Archive.** The default backend is `https://lists.apache.org` (covers every
Apache project — Flink, Kafka, Spark, …). Only if the user is on a *different*
Pony Mail deployment do you set a custom base:

```
mari config set lists.archive_url https://lists.example.org
```

**Lists.** Each list is a full address. For Apache Flink the useful ones are:

- `dev@flink.apache.org` — design discussion, FLIP `[DISCUSS]`/`[VOTE]`
  threads, release management. **The primary source.**
- `user@flink.apache.org` — user Q&A, real-world usage and gotchas.
- `user-zh@flink.apache.org` — Chinese-language user list.
- `community@flink.apache.org` — community/organizational threads.

Ask the user which lists they want. Recommend at least `dev@` and `user@`.

## 3. Track, scope, sync

### 3a. Track at least one list

Run once per list. Each ref accepts any of:

- a bare address — `dev@flink.apache.org`
- `lists:dev@flink.apache.org`
- a `lists.apache.org` list or thread URL (Mari extracts the list address)

```
mari track lists add dev@flink.apache.org
mari track lists add user@flink.apache.org
```

Removing a list later and re-syncing prunes that list's threads from the index.

### 3b. Set the scope

```
mari scope lists local
```
(or `global`, per step 1.)

### 3c. Choose how far back to reach (optional)

The first sync backfills `lists.lookback_days` — **0 means the entire
archive** (Flink's `dev@` reaches back to 2014, which is what you usually want
for full product history). To limit the first pull instead:

```
mari config set lists.lookback_days 365   # only the last year on first sync
```

Later syncs are always incremental (per-list newest-message cursor plus a
14-day trailing re-scan for late replies).

### 3d. Sync

```
mari sync lists
```

First sync of a busy list can take a while — it walks the archive month by
month. On success, `status` shows the list under **Mailing lists** with a
non-zero `indexed` count. Confirm with a search, e.g.:

```
mari search "FLIP-27 source interface vote"
```
