# Contributing

Thanks for your interest in Mari. This guide covers how to build, test, and
propose changes.

## Getting started

Mari is a single Rust crate. You need a Rust toolchain and **cmake** (llama.cpp
builds from source on first compile — the initial build takes a few minutes).

```sh
git clone <repo>
cd mari
cargo build            # ~3 min cold (llama.cpp + duckdb + lance)
cargo test             # unit + integration tests
```

`SPEC.md` is the authoritative behavioral specification — every command, rule,
and config key. §22 records the concrete implementation decisions for this
build (what is implemented vs deferred). Read the relevant section before
changing behavior. The remaining-work plan lives in `docs/`.

## Development

- **Layout.** `src/detector/` is the prose engine (rules, packs, grammar);
  `src/index/` is the catalog, chunking, hybrid search, and vector embeddings;
  `src/connectors/` holds the source integrations; `src/attn.rs` is the local
  attention engine; top-level modules are the individual commands.
- **Style.** Match the surrounding code. Detector rules ship their normative
  word/phrase lists as in-module consts and a bad→good fixture test each.
- **Prose.** Dogfood the tools: run `mari detect` (and `mari deslop` via the
  skill) over docs you touch; keep `mari check` green.

## Testing

- `cargo test` runs the full suite. Model-inference tests are gated behind the
  presence of the local GGUFs (they download on first real run).
- `cargo clippy -- -D warnings` and `cargo fmt --check` must be clean.
- `cargo deny check` must pass (licenses + advisories; see `deny.toml`).
- New detector rules must add a bad→good fixture pair (§19).
- Changes with a runtime surface should be verified end-to-end, not just by
  unit tests.

## Reporting bugs

Open an issue on GitHub with the smallest reproduction you can manage: the command you ran, what you expected, and what happened. Include your operating system, the output of `mari --version`, and `mari doctor` when the problem touches models or optional tools. Search the open issues first so you are not filing a duplicate. For a security vulnerability, do not open a public issue. Follow `SECURITY.md` instead.

## Pull requests

- Keep PRs focused; one logical change per PR.
- Update `SPEC.md` (including §22) when you add, remove, or change a feature —
  the spec is the contract.
- Ensure CI is green: build, tests, clippy, fmt, deny.
- Describe the behavior change and how you verified it.

## Code of conduct

This project follows the Contributor Covenant. See `CODE_OF_CONDUCT.md`.
