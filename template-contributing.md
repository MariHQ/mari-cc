<!--
  mari template: contributing — Contributing guide
  Grounding: GitHub's community-standards checklist

  Required sections (in order):
    1. How to Contribute    (aliases: contributing, ways to contribute, getting started)
    2. Development Setup    (aliases: development, building, local development, setup,
                             getting started)
    3. Pull Request Process (aliases: pull requests, pull request, submitting changes,
                             submitting a pull request, opening a pull request, making changes)
    4. Reporting Bugs       (aliases: reporting issues, bug reports, issues, filing issues)
  Recommended sections:
    - Code of Conduct
    - Style Guide (aliases: coding style, code style, coding conventions)
    - Testing     (aliases: tests, running tests)

  Tone norms:
    - Open with the fastest path to a first contribution.
    - Link the Code of Conduct rather than restating it.
    - Give exact commands for setup, tests, and submitting a PR.

  Detection heuristic (mari asset detect):
    - Canonical filename (+4, qualifying): CONTRIBUTING.md (also .mdx/.rst/.txt, optional
      language suffix like CONTRIBUTING.zh-cn.md), case-insensitive, anywhere in the tree.
    - Directory match (+3, qualifying): .github/ or docs/contributing/
    - Heading markers (+2 each, capped at 3 hits): how to contribute, ways to contribute,
      getting started, development, development setup, local development, building,
      pull request, pull requests, pull request process, submitting changes,
      submitting a pull request, reporting bugs, reporting issues, issues,
      code of conduct, style guide, coding style, coding conventions, testing,
      running tests, commit messages. Strong (content-only qualifying) markers:
      pull request process, submitting changes, submitting a pull request,
      development setup, how to contribute, ways to contribute.
    - Best-scoring type wins; total score must be >= 4.

  Structure checks (mari asset check):
    - asset-missing-section (warn): each required section absent from headings.
    - asset-missing-section (advisory): each recommended section absent.

  Community-file role: required core artifact; `mari community` scaffolds it at
  CONTRIBUTING.md via this same asset template (one source of truth).
-->
# Contributing to <project>

Thanks for contributing! This guide gets you from clone to merged PR.

## How to Contribute

The quickest ways to help: fix a bug, improve docs, or pick up a `good first issue`.
By participating you agree to our [Code of Conduct](CODE_OF_CONDUCT.md).

## Development Setup

```bash
git clone <repo> && cd <project>
<install deps>
<run the app / tests>
```

## Pull Request Process

1. Fork and branch from `main`.
2. Make your change, with tests and updated docs.
3. Ensure the test suite and linters pass.
4. Open a PR describing the what and the why; link any related issue.

## Reporting Bugs

Open an issue with steps to reproduce, expected vs. actual behavior, and your environment.
Search existing issues first to avoid duplicates.
