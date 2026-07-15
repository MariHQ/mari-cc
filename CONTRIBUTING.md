# Contributing

Mari is a Rust CLI with a Vite/React console.

## Build and test

```sh
cargo build
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
npm --prefix console install
npm --prefix console run build
```

Detector rules live under `src/detector/`. Add focused positive and negative fixtures for every rule change. Keep rule IDs stable because repository configuration may refer to them.

The console source lives under `console/src/`; the production bundle in `console/dist/` is embedded in the binary.

## Pull requests

Keep each pull request scoped to one concern. Explain the user-visible behavior, list the checks you ran, and call out configuration or rule-ID changes.

For bugs, include the smallest reproduction, your operating system, `mari --version`, the command you ran, and the observed output. Follow `SECURITY.md` for vulnerabilities.
