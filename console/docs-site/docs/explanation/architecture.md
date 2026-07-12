---
title: Architecture
sidebar_position: 1
---

# Architecture

The console is a single-page app served by the `mari` binary. It has no server of its own, no build step at runtime, and no external dependency. This page explains how the pieces fit.

## Frontend

The frontend is a Vite and React 18 app with Tailwind, under `console/src/saas`. It has one entry point and no authentication, because it only ever runs locally against your own machine. All data goes through a typed `fetch` client in `src/saas/lib/client.ts`, which calls the local API.

The UI is composed of panel components, one per feature area (`OverviewGroup`, `SourcesGroup`, `TagsGroup`, and so on). Each panel reads from and writes to one part of the API. A command palette and a project switcher sit above the panels for navigation.

## Backend

The backend is `src/console/` in the Rust crate. It is a synchronous `tiny_http` server. `mod.rs` serves the embedded frontend bundle and routes requests. `api.rs` holds the JSON handlers, each backed by an existing Mari module: `curation`, `lineage`, `config`, `connectors`, and `search`. There is no separate service and no database of its own. The console reads and writes the same catalog and configuration the command-line tool uses.

## One binary, no Node at runtime

The built frontend (`console/dist`) is baked into the `mari` binary at compile time with `include_dir!`, and the bundle is committed to the repository. That means `cargo build` and `cargo install` produce a working console with no Node.js present. When you run `mari console`, the binary serves the embedded bundle directly.

## Why it stays consistent with the CLI

Every console mutation writes through the same code path as the matching `mari` command. Tagging a document in the Tags panel and running `mari tag` do the identical thing, because both call the `curation` module. This is the design invariant that lets you move between the browser and the terminal without the two drifting apart.

## Requests that reach the app

Static asset paths map straight into the embedded bundle. Any other path, including the panel routes under `/console`, serves `index.html` so the single-page app can handle client-side routing. Unknown API routes return a JSON error rather than the app shell.
