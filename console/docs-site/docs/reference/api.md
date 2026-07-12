---
title: HTTP API
sidebar_position: 1
---

# HTTP API

The console frontend talks to a small JSON API served by the `mari` binary. The API is local-only, bound to `127.0.0.1`, and backed by the same `curation`, `lineage`, `config`, `connectors`, and `search` modules the command-line tool uses. Every route lives in `src/console/api.rs`.

You will not normally call these directly. They are documented so you can understand what each panel does and script against a running console if you want to.

## Conventions

- Base path is the console origin, for example `http://127.0.0.1:4319`.
- Reads are `GET`. Mutations are `POST`, `PUT`, or `DELETE`, and they write through the same paths as the equivalent `mari` command.
- Responses are JSON. Errors return a JSON body with an `error` field.

## Routes

| Method | Path | Backs the panel |
|--------|------|-----------------|
| GET | `/status` | Status |
| GET | `/overview` | Overview |
| GET | `/sources` | Sources |
| POST | `/sources/track` | Sources |
| POST | `/sources/sync` | Sources |
| GET | `/documents` | Documents |
| GET | `/documents/{id}` | Documents |
| GET | `/search` | Search |
| GET | `/tags` | Tags |
| POST | `/tags` | Tags |
| DELETE | `/tags` | Tags |
| POST | `/tags/statuses` | Tags |
| GET | `/lineage` | Lineage |
| POST | `/lineage` | Lineage |
| POST | `/lineage/{id}/confirm` | Lineage |
| POST | `/lineage/{id}/reject` | Lineage |
| GET | `/facts` | Facts |
| GET | `/glossary` | Glossary |
| GET | `/config` | Config |
| PUT | `/config` | Config |
| GET | `/projects` | Project switcher |
| POST | `/projects/switch` | Project switcher |
| POST | `/projects/register` | Project switcher |
| GET | `/nudges` | Nudges |
| POST | `/nudges` | Nudges |
| DELETE | `/nudges` | Nudges |
| GET | `/rules` | Rules |
| POST | `/rules` | Rules |
| POST | `/rules/discover` | Rules |
| DELETE | `/rules` | Rules |
| GET | `/detector` | Detector |
| POST | `/detector/zero` | Detector |
| POST | `/detector/ignore` | Detector |
| POST | `/detect` | Detector |
| GET | `/templates` | Templates |
| POST | `/templates/scaffold` | Templates |
| GET | `/localization` | Localization |
| GET | `/localization/coverage` | Localization |
| GET | `/localization/file` | Localization |
| GET | `/docsite` | Docsite |

Any path outside this list falls through to the single-page app, which serves `index.html` so client-side routing works.
