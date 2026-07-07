# 07 — Security & Trust

Mari is local-first (a stated invariant: no external LLM calls from the CLI,
credentials never in the repo). That posture is mostly honored in code, but
several items must be hardened before external use.

---

## 7.1 — Model-download integrity (P0, S–M)

**Current.** Both GGUFs download from unpinned Hugging Face `main` revisions
with **no checksum verification**:
- `src/index/vector.rs` `MODEL_URL` → `…/resolve/main/…gguf`
- `src/attn.rs` `DEFAULT_MODEL_URL` → `…/resolve/main/…gguf`

Risks: a repointed `main` silently changes the model; a corrupted/truncated
download is accepted (the truncation guard only checks a minimum byte count);
a MITM on the download substitutes weights.

**Fix.**
- Pin a specific HF revision (commit SHA) in each URL, not `main`.
- Record an expected SHA-256 per model; verify after download; refuse and
  delete on mismatch.
- Prefer HTTPS with cert validation (ureq does this) and consider an
  owner-hosted mirror (`02` #4) for reproducibility and rate-limit avoidance.

**Acceptance.** A tampered/truncated download is rejected with a clear error;
the pinned revision is reproducible; `mari model pull` reports the verified
hash.

**Effort.** S–M.

---

## 7.2 — `trust_remote_code` in the OCR runner (P0 disclosure, M to sandbox)

**Current.** The optional Unlimited-OCR Python runner
(`src/ocr.rs::RUNNER_PY`) loads the model with `trust_remote_code=True` —
by design of that model, this executes arbitrary Python from the HF repo. It
is opt-in (only the `auto`/`ocr-model` backends, and only after
`ocr.auto_install`), and the default backend is pure-Rust `pdf-extract`. But
the risk is real and currently undisclosed.

**Fix.**
- Document the risk prominently wherever `ocr.backend = auto|ocr-model` is
  configured or first triggered; require an explicit acknowledgement (a
  config flag like `ocr.accept_remote_code = true`, defaulting false, that
  gates provisioning).
- Pin the OCR model revision + verify (as 7.1).
- Consider running the Python toolchain in a constrained environment
  (dedicated venv is already used; document that it is not a security
  sandbox).
- Make crystal clear in docs that the **default OCR path is pure Rust** and
  never runs remote code.

**Acceptance.** A user cannot reach the `trust_remote_code` path without an
explicit opt-in they were warned about; the default path is unaffected.

**Effort.** M.

---

## 7.3 — Credential handling audit (P0, S)

**Current.** Credentials live under `~/.mari/credentials/<provider>.json`
(global) or per-workspace, written via `workspace::write_credential` at
mode 0600, dirs 0700 (`src/workspace.rs`). Team-shared `.mari/config.json`
holds tracked refs and tags but should never hold secrets.

**Audit checklist.**
- [ ] Confirm every credential write path uses `write_credential` (0600) —
  grep for direct `fs::write` into credential locations.
- [ ] Confirm no token is ever logged (grep connector error paths for
  interpolating `token`/`cred` into eprintln).
- [ ] Confirm `.mari/config.json` (committed) never receives a secret — only
  refs, tags, rules, nudges. Microsoft refresh-token write-back must target
  the *credentials* dir, not config.
- [ ] Confirm `mari status` / `mari config list` never print secrets.
- [ ] `.gitignore` covers `.mari/config.local.json` and any local state that
  could carry tokens; the committed `.mari/` never carries credentials.
- [ ] Windows equivalent of 0600 (ACLs) once Windows is supported (`08`).

**Acceptance.** A written audit note confirming each checkbox, plus a test
that a synced credential file is 0600 and that config never contains a token.

**Effort.** S.

---

## 7.4 — License compatibility audit (P0, S)

**Current.** Crate declares `license = "MIT"`. Bundled: Qwen models
(Apache-2.0, OK for MIT crate), Unlimited-OCR (MIT), Harper (check its
license), duckdb/lance/llama-cpp/harper and their transitive deps (mixed
MIT/Apache/BSD, generally OK). The **jina embedding model was CC-BY-NC** and
has been swapped out — verify no jina reference or download URL remains after
`01-known-issues.md` #1 is fixed (the current stub still names jina).

**Fix.**
- Run `cargo license` / `cargo deny` to enumerate all dependency licenses and
  flag anything copyleft or NC.
- Confirm the two shipped model licenses permit redistribution/commercial use
  under Mari's chosen license.
- Add a `NOTICE`/`THIRD_PARTY_LICENSES` file if any dependency requires
  attribution.

**Acceptance.** `cargo deny check licenses` passes with an allowlist the
owner has approved; no NC-licensed asset ships or downloads.

**Effort.** S.

---

## 7.5 — Input-handling safety (P2, M)

**Current.** Mari parses untrusted content: connector payloads, Office/PDF
files, markdown. Consider:
- **Zip-bomb / path-traversal** in Office extraction (`src/office.rs` uses
  `zip::by_name` for known entries and `file_names()` filtering — verify no
  entry name escapes; the code reads specific paths, which is safer than
  extracting to disk).
- **PDF parser** (`pdf-extract`) on malformed/malicious PDFs — it runs
  in-process; a panic is caught where wired, but verify a malformed PDF can't
  hang or OOM.
- **XML entity expansion** (billion-laughs) in `quick-xml` — confirm entity
  expansion is bounded (quick-xml does not expand external entities by
  default; document it).
- **SQL surface** — `mari sql` is `SELECT`/`WITH`-only (verified by a test);
  confirm the allowlist can't be bypassed via CTEs calling functions with
  side effects in DuckDB.

**Acceptance.** Fuzz or fixture the extractors against malformed inputs; none
hang, OOM, escape the sandbox, or execute code.

**Effort.** M.

---

## 7.6 — PII / sensitive-content posture (P2 → documented non-goal in v1)

**Current.** SPEC §20 lists "No PII redaction of indexed content in v1
(credentials protection only) — flagged as future work." Mari indexes
whatever the connectors return, including potentially sensitive Slack DMs,
tickets, and mail, into a local catalog.

**Fix (v1: document; later: implement).**
- Document that the index may contain sensitive content and lives at
  `~/.mari/<workspace>/` (recommend disk encryption for laptops).
- Document that team-shared cloud catalogs replicate that content to whatever
  storage the team controls (Git LFS / S3) — a data-governance decision the
  team must make consciously.
- Future: opt-in PII detection/redaction at ingest.

**Acceptance.** The data-handling posture is documented; teams can make an
informed choice about what to index and share.

**Effort.** S (docs) now; L (redaction) later.

---

## 7.7 — Supply-chain / dependency hygiene (P2, S)

**Current.** Heavy dependency tree (llama.cpp, duckdb, lance, datafusion,
harper, ONNX later). Pin and audit.

**Fix.** `cargo deny` (advisories + licenses + bans), `cargo audit` in CI
(`09`); pin `Cargo.lock` for the binary; review the harper pin (=2.0.0 is
deliberate because 2.4/2.5 don't compile — track upstream for a fixed
release).

**Acceptance.** CI fails on a new advisory or a disallowed license; the
harper pin is documented with the upstream issue link.

**Effort.** S.
