# Models

Mari runs small models locally. Nothing leaves your machine, and the CLI never calls an external service for inference. This page lists the models, when each one loads, and how to manage them.

## The two bundled models

| Model | Identity | Size | Needed for |
|-------|----------|------|------------|
| Embeddings | `Qwen3-Embedding-0.6B` (Apache-2.0) | ~640 MB | `sync`, `search`, `explore` |
| Attention | `Qwen3.5-0.8B` (Apache-2.0) | ~520 MB | The opt-in `--deep` and `--focus` passes |

Both download on first use into `~/.mari/models`. The embedding model is the only permitted embedding identity. If it is unavailable, vector embedding fails loudly rather than falling back silently, and keyword-only search can still run.

## Manage models

```sh
mari model status        # what is present, and any index mismatch
mari model pull all      # fetch both ahead of time
```

`mari status` warns when the index was built with a different embedding model and suggests `mari sync --rebuild`.

## Capability tiers

Mari layers detection and grounding by model size, never framing it as "rules versus AI":

- **Tier 0, deterministic (always on).** The full rule registry, typed-span factcheck, and structural checks. Instant, offline, no dependencies.
- **Tier 1, small local models (default once provisioned).** Machine-likelihood, entailment and contradiction checking, zero-shot slop-span extraction, and embeddings. Skip them with `--no-models`.
- **Tier 2, attention (opt-in).** On-device grounding, coverage, and focus. Powers every `--deep` flag. Roughly seconds per document.
- **Agent tier.** Claude does anything that generates text in-session, such as rewriting, query expansion, or page drafting. The CLI only prints candidate spans, scores, and evidence.

## Optional OCR tier

Scanned PDFs can use an optical character recognition tier backed by `baidu/Unlimited-OCR`. It is off by default. The default PDF path is pure-Rust text extraction. OCR needs an explicit opt-in because it runs code from the model repository. Enable it in config under `ocr.backend` and acknowledge `ocr.accept_remote_code`. See `SECURITY.md` before turning it on.
