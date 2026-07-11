# Mari Console

A local, single-user web dashboard over your Mari knowledge base. It is served
by the `mari` binary itself — there is no Node runtime, no cloud, and no auth.

```
mari console            # serve on http://127.0.0.1:4319/console
mari console --open     # …and open a browser
mari console --port N   # pick a port
```

## What it is

The console reads and **writes** the same DuckDB/Iceberg catalog and config the
CLI uses, so anything you change here (tags, lineage, config, tracked refs) is
identical to running the equivalent `mari` command. Sections:

- **Overview** — documents, connectors, freshness, tag distribution, recent syncs.
- **Sources** — every connector, what it tracks, sync status; add/remove tracked
  refs and trigger syncs.
- **Documents** — browse indexed documents; read the body, chunks, and lineage;
  tag inline.
- **Search** — hybrid semantic + keyword search.
- **Tags** — apply / remove curation tags (canonical, stale, deprecated, …).
- **Lineage** — a navigable React Flow graph of span↔span edges; confirm/reject
  proposals; add edges.
- **Glossary / Facts** — STYLE.md terms and the FACTS.md ledger (read-only).
- **Cloud** — team sharing over an S3 (or git) warehouse: connect/initialize,
  set this machine's role (writer/consumer), and pull / push-sync (with
  compaction).
- **Config** — effective configuration across layers; edit repo or global values.
- **Status** — workspace, embedding model, catalog, cloud.

## Architecture

- **Frontend** — Vite + React 18 + Tailwind (`src/saas`). One entry, no router
  auth. Data goes through `src/saas/lib/client.ts` (typed `fetch` to `/api/*`).
- **Backend** — `src/console/` in the Rust crate: a synchronous `tiny_http`
  server. `mod.rs` serves the embedded bundle + routes; `api.rs` holds the JSON
  handlers, each backed by the existing `curation` / `lineage` / `config` /
  `connectors` / `search` modules.
- **Embedding** — the built `dist/` is baked into the binary with `include_dir!`
  and is committed so `cargo build` / `cargo install` work with no Node present.

## Developing

```
make console          # install deps + build dist (run from repo root)
make build            # rebuild dist + release binary
# or, for a fast frontend loop with HMR:
cd console && npm run dev          # http://localhost:4318, proxies /api to :4319
mari console --port 4319           # run the API alongside it
```

After changing the frontend, run `make console` (or `npm run build`) and rebuild
the binary so the embedded bundle updates.
