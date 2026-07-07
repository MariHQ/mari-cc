# 08 — Portability

Mari is verified only on macOS/arm64 (Metal). This doc enumerates what it
takes to support the other platforms teams actually run.

---

## 8.1 — Windows support (P2, L)

**Current.** Several paths are Unix-only:
- `src/ocr.rs` — venv `bin/python` (Windows is `Scripts/python.exe`).
- `src/rulescmd.rs` / hook install — a `sh` `post-commit` script.
- `src/workspace.rs` — `PermissionsExt` 0600/0700 (no Windows equivalent
  applied; needs ACL-based restriction).
- Path handling is mostly `PathBuf` (portable) but shell-outs (`git`,
  `gcloud`, `aws`) assume Unix availability.

**Design.**
- `#[cfg(windows)]` branches for venv path, hook script (`.bat`/PowerShell or
  a native git hook), and credential-file ACLs (restrict to the current
  user).
- Verify llama.cpp/duckdb/lance build on Windows (they do upstream; the CI
  matrix must prove it).
- Decide the GPU story on Windows (CUDA/DirectML or CPU-only).

**Acceptance.** `mari` builds and passes the suite on Windows CI; credentials
are user-restricted; hooks install and fire.

**Effort.** L.

---

## 8.2 — Linux verification + CUDA (P1 Linux, P2 CUDA)

**Current.** Linux is a build target in the plan (`05` #1) but unverified;
CUDA is entirely untested (only Metal exercised).

**Design.**
- Linux x86_64/arm64 CI jobs: build + test + a real-inference smoke where the
  model is cached.
- CUDA: a separate build variant of llama.cpp (`GGML_CUDA`), a larger binary,
  and its own CI runner with a GPU (or accept CPU-only on Linux for v1 and
  document CUDA as a from-source build option).
- Confirm `n_gpu_layers = 99` degrades cleanly to CPU when no GPU is present
  (llama.cpp handles this, but verify the embedding/attention paths don't
  assume a device).

**Acceptance.** Linux CPU-only inference works out of the box; a documented
path exists for CUDA; the binary auto-detects and falls back to CPU.

**Effort.** M (Linux CPU) / L (CUDA).

---

## 8.3 — Model-runtime portability (P2, M)

**Current.** Embedding + attention run through llama.cpp with
`n_gpu_layers = 99` (offload everything). On a machine with a small/absent
GPU this can OOM or fail.

**Where.** `src/index/vector.rs` (`LlamaModelParams`),
`src/attn.rs` (`mparams.n_gpu_layers = 99`).

**Design.** Make GPU-layer offload configurable
(`embedding.gpu_layers` / `attention.gpu_layers`, default auto), and
auto-reduce on OOM (the mari-cli prototype's guidance: retry with fewer
layers / smaller ubatch). Surface a clear "reduce `gpu_layers`" message on
Metal/CUDA OOM.

**Acceptance.** On a low-VRAM machine, inference either succeeds with
partial offload or fails with an actionable message, never a silent hang.

**Effort.** M.

---

## 8.4 — External tool dependencies (P2, S)

**Current.** Mari shells out to `git` (always), and optionally `gcloud`
(gdocs), `aws` (S3 cloud backend), `python3` (OCR model tiers). These are
assumed present with no preflight.

**Design.** A `mari doctor` (or extend `mari status`) that reports which
external tools are present and which optional features they gate, with the
install hint for each. Fail connector/feature paths with a clear "install X"
message rather than a raw spawn error (mostly done for gcloud/aws/python;
audit for completeness).

**Acceptance.** `mari doctor` lists tool availability and feature impact; any
missing-tool failure names the tool and the fix.

**Effort.** S.
