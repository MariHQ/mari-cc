---
title: Knowledge panels
sidebar_position: 1
---

# Knowledge panels

These panels cover the core loop: see what is indexed, connect sources, and read the knowledge back.

## Overview

Your knowledge base at a glance. The Overview panel summarizes the index: how many documents and chunks are stored, which sources are active, and recent activity. It is the first thing you see when the console opens. It maps to `mari overview` and the summary half of `mari status`.

## Status

Workspace, embedding model, catalog, and cloud. The Status panel shows the current workspace directory, the embedding model, the catalog location, and the cloud role when sharing is enabled. It warns when the index was built with a different embedding model than the one now configured. It maps to `mari status`.

## Sources

Connectors, what they track, and their sync status. The Sources panel lists every connector, whether it is connected, what it tracks, and how many documents it has indexed. From here you can track a new reference and trigger a sync without leaving the browser. It maps to `mari track` and `mari sync`.

## Search

Hybrid semantic and keyword search across everything. The Search panel runs the same hybrid retrieval as `mari search`. It fuses vector similarity with keyword scoring, and curation tags shape the ranking. Filter by source, tag, author, or date, and open any hit to read the full document. It maps to `mari search`.

## Documents

Every indexed document in your knowledge base. The Documents panel is a browsable table of everything in the index, with its source, title, and tags. Open a row to read the full body, the same content `mari doc` returns. It maps to `mari documents` and `mari doc`.

## See also

- The `mari` command-line tool has equivalents for every panel here: `mari search`, `mari sync`, `mari track`, and `mari doc`.
- [Curation panels](curation.md) to tag what you find here.
