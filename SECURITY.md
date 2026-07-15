# Security policy

## Report a vulnerability

Do not open a public issue for a suspected vulnerability. Email the repository maintainers with:

- the affected version or commit;
- steps to reproduce;
- the expected and observed behavior;
- any suggested mitigation.

We will acknowledge the report, assess severity, and coordinate a fix and disclosure timeline.

## Runtime boundary

Mari reads repository files selected by its commands and stores project settings in `.mari/config.json` or `.mari/config.local.json`. The console binds to `127.0.0.1` and should not be exposed through a public proxy.

The post-edit hook receives file paths from Claude Code and reports detector findings. Treat hook and configuration changes like source changes: review them before merging.

## Dependencies

Rust dependencies are locked in `Cargo.lock`. Report a dependency advisory with the package name, affected range, and advisory identifier.
