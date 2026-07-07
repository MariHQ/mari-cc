# Security Policy

## Reporting a Vulnerability

Please report security issues privately to the maintainers rather than opening
a public issue. Include a description, reproduction steps, and the affected
version (`mari --version`). We aim to acknowledge reports promptly and will
coordinate a fix and disclosure timeline with you.

## Response Process

On receiving a report we acknowledge it, reproduce and assess severity, prepare
a fix on a private branch, and coordinate a disclosure date with the reporter.
Fixes ship in the next release with a `CHANGELOG.md` security note; credit is
given to reporters who want it.

## Supported Versions

Mari is pre-1.0; security fixes land on the latest release. Pin a specific
version for reproducibility and update when advisories are published.

## Security posture

Mari is **local-first**: indexing, embeddings, search, and the prose detector
all run on your machine. The CLI makes no external LLM calls. The only network
activity is (a) syncing from the sources you explicitly connect, using
credentials you provide, and (b) one-time model downloads (below).

### Credentials

- Credentials are stored under `~/.mari/credentials/<provider>.json` (global)
  or the per-workspace credentials directory, at mode `0600` (dir `0700`).
- Credentials never enter the repository. The committed `.mari/config.json`
  holds only tracked refs, tags, rules, and nudges — never secrets.
- Run `mari doctor` to see which connectors are configured.

### Model downloads

On first use, Mari downloads two small models into `~/.mari/models` from
Hugging Face. Downloads are resumable and, once a checksum is pinned in the
release, verified against a known SHA-256 (a mismatch is rejected). To
provision explicitly (e.g. before going offline): `mari model pull all`. For
air-gapped installs, place the GGUFs yourself and point `embedding.model` /
`attention.model` at them with `embedding.auto_download=false`.

### OCR remote-code disclosure (important)

The **default** PDF path (`ocr.backend = "text"`) is pure Rust and executes no
external code.

The optional OCR *model* tiers (`ocr.backend = "auto"` or `"ocr-model"`) run
`baidu/Unlimited-OCR` through a local Python toolchain that loads the model
with `trust_remote_code=True` — meaning it **executes code shipped in the model
repository**. Because of this, the model tiers are gated behind an explicit
acknowledgement:

```sh
mari config set ocr.accept_remote_code true
```

Without that setting, Mari refuses to provision or run the model tiers. Only
enable it if you trust the `baidu/Unlimited-OCR` model repository, and prefer
pinning a specific model revision. If you only need embedded-text PDFs, leave
the default `text` backend and this never applies.

### Indexed content

Mari indexes whatever your connected sources return — which may include
sensitive Slack DMs, tickets, or mail — into a local catalog under
`~/.mari/`. Consider full-disk encryption on laptops. If you enable cloud
sharing, that content replicates to the storage your team controls (Git LFS or
S3); treat that as a data-governance decision. PII redaction is not performed
in this version.

### Dependency and supply-chain hygiene

The dependency tree is checked with `cargo deny` (licenses + advisories; see
`deny.toml`) and `cargo audit` in CI. Report any concern via the process
above.
