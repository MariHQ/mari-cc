# Design decisions

## Concrete findings

Every detector result names the passage and rule. Scores summarize findings but do not replace editorial review.

## Repository-owned configuration

Shared settings live beside the project they govern. Teams can review style changes in the same pull request as prose changes, while `.mari/config.local.json` provides a repository-specific personal override.

## Focused commands

Editorial verbs describe intent. `tighten` should reduce length, `clarify` should reduce ambiguity, and `understate` should remove explanation that the reader does not need. This is more predictable than one generic rewrite mode.

## Claude performs the edit

The CLI supplies deterministic evidence and structural checks. Claude uses the current conversation, repository context, and user direction to make the revision.
