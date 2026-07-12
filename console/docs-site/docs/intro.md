---
slug: /
title: Introduction
sidebar_position: 1
---

# Mari Console

The Mari Console is a local web dashboard over your knowledge base. It is a browser interface to the same catalog and configuration the `mari` command-line tool reads and writes, so anything you do in one shows up in the other.

Everything is local. The `mari` binary serves the console itself, and it talks only to a local API and makes no external calls. Your credentials never leave your machine.

## Open the console

From any tracked project, run:

```sh
mari console --open
```

This starts a server at `http://127.0.0.1:4319/console` and opens your browser. Pass `--port` to choose a different port. Stop the server with Ctrl-C.

If `mari console` reports an unknown command, your installed binary predates the console. It landed in Mari 0.2.0. Build from source or update to a 0.2.0 or newer binary.

## What you can do here

The console is organized into panels, grouped by task:

- [Knowledge](panels/knowledge.md): your index at a glance, sources, search, and documents.
- [Curation](panels/curation.md): apply tags, keep a glossary, and manage the facts ledger.
- [Maintenance](panels/maintenance.md): lineage, nudges, edit-notify rules, and the prose detector.
- [Docs](panels/docs.md): docsite readiness, localization coverage, and document templates.
- [Team](panels/team.md): cloud sharing and configuration.

Every panel mirrors a `mari` command. The [API reference](reference/api.md) lists the endpoints behind them, and [Architecture](explanation/architecture.md) explains how the dashboard fits together.

## A note on scope

The console reads and writes the knowledge base, but it does not generate prose. Rewriting, drafting, and other generative work still run through Claude and the `mari` editorial verbs. The console surfaces the detector, tags, and structure so you can steer that work.
