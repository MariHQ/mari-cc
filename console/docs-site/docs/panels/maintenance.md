---
title: Maintenance panels
sidebar_position: 3
---

# Maintenance panels

These panels keep docs and code in step and keep prose clean. They surface the same maintenance loop the post-edit hook runs, in a form you can inspect and edit.

## Lineage

Span-to-span maintenance edges. The Lineage panel shows the curated graph of links between code and doc spans. Search a node or filter to navigate the graph, and confirm or reject proposed edges. Once an edge is confirmed, editing one side reminds you to update the other. It maps to `mari lineage`.

## Nudges

When this changes, remember to update that. The Nudges panel lists hand-declared maintenance couplings: when a file or symbol matching the trigger is edited, the console records that the target should be updated too. Unlike lineage, nudges are stated up front by name rather than curated from proposals. It maps to `mari nudge`.

## Rules

Edit-notify rules. The Rules panel manages the couplings that remind Claude to update related docs when matching code changes. You can add a rule by hand or run discovery, which scans the repo for code-to-docs couplings and proposes rules. It maps to `mari rules`.

## Detector

Run the deterministic prose detector on text or a repo file. The Detector panel runs `mari detect` in the browser: paste text or point at a file, and see findings, the slop score, and one-click waivers. Waiving a rule, adding it to the zero-tolerance list, or allowing a value writes straight to the detector configuration. It maps to `mari detect`, `mari zero`, and `mari ignores`.

## See also

- [Docs panels](docs.md) for docsite readiness and localization drift.
- The `mari` command-line tool has equivalents: `mari lineage`, `mari nudge`, `mari rules`, and `mari detect`.
