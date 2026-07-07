# 03 — ML Tier 1 (NLI, Machine-Likelihood, Slop-Spans)

The one deferred model tier from SPEC §17. Everything here is **P2** — the
deterministic and Tier-2 attention layers already cover the primary flows;
this tier improves precision/recall on grounding and slop detection. Each
flag already parses and degrades loudly, so wiring is additive.

Runtime note: the crate already links llama.cpp (`llama-cpp-2`,
`llama-cpp-sys-2`) and has an ONNX-free precedent. Tier 1 can go two ways —
ONNX Runtime (`ort`) / `candle` for the cross-encoder + GLiNER, or llama.cpp
GGUF for everything to avoid a second runtime. Pick one and stay consistent
(see "Runtime decision" below).

---

## 3.1 — NLI entailment/contradiction (factcheck + audit) (P2, L)

**What.** SPEC §11.10 "NLI tier": with `--models`, run a natural-language
inference cross-encoder (premise = fact, hypothesis = claim). Typed-span
mismatch stays the hard error; otherwise contradiction ≥ 0.60 and >
entailment → `contradicts-fact` (error, with NLI %); entailment ≥ 0.55 →
supported (no finding); else neutral → `unsupported-claim`.

**Where.** `src/factcheck.rs` — the `if args.models { … "not available" … }`
note at the top; the tier-0 verdict logic in `check_sentences`. Also
`src/curation.rs::audit_kb` for contradiction candidates via NLI.

**Design.**
- Model: a small multilingual NLI cross-encoder (e.g. an mDeBERTa-v3 NLI or
  a distilled variant) as ONNX, or a GGUF-served equivalent.
- Add `src/index/nli.rs` (or `src/ml/nli.rs`): load once, score
  (premise, hypothesis) → {entail, neutral, contradict} probabilities.
- Wire into the retrieve → typed-span → NLI pipeline; keep typed-span
  mismatch as the hard error (NLI only refines the neutral/soft cases).
- Respect the resident-model idea (`04` #6) so factcheck doesn't reload per
  file.

**Acceptance.** `factcheck --models` on a fixture where a claim contradicts a
fact in meaning but shares no typed span (e.g. "the plan is free" vs
"the plan costs $12") produces `contradicts-fact` with an NLI %. Real
inference, gated behind the model-present CI job.

**Effort.** L (model selection + tokenizer + runtime + calibration + fixtures).

---

## 3.2 — Machine-likelihood / perplexity blend for the slop score (P2, M)

**What.** SPEC §12 step 5: when a machine-likelihood `m ∈ [0,1]` is available
via `--models`, `score = 0.8·deterministic + 0.2·(m·100)`. The model term
never dominates; the breakdown reports it.

**Where.** `src/detector/score.rs::compute` already accepts an
`Option<f64>` machine term and blends it — the plumbing exists. What's
missing is the *producer*: a perplexity/machine-likelihood estimate.

**Design.**
- Reuse the attention model (`Qwen3.5-0.8B`) already loaded for Tier 2:
  compute per-token perplexity over the document, map to `m ∈ [0,1]` via a
  calibrated squashing function (SPEC §17 lists `llama-cpp-2` perplexity as
  the mechanism).
- Add `src/attn.rs` (or a sibling) `perplexity(text) -> f64` and a mapping to
  `m`. Feed into `detect --score --models`.
- Keep it explainable: the score breakdown already reports the machine term
  when present.

**Acceptance.** `detect --score --models` on the slop fixture reports a
`machineLikelihood` field and a blended score; deterministic-only score is
unchanged without `--models`.

**Effort.** M (perplexity is cheap given the loaded model; calibration is the
work).

---

## 3.3 — Zero-shot slop-span extraction (P2, L)

**What.** SPEC §17 Tier 1 + `mari detect --slop-spans` (requires `--models`):
zero-shot span extraction with labels {marketing buzzword, hype phrase, vague
corporate jargon, empty filler phrase, overused cliché}. SPEC names
`gline-rs` (GLiNER) as the mechanism.

**Where.** `src/detector/runner.rs` — the `--slop-spans` flag currently only
prints the ML-tier note; findings would be emitted alongside the
deterministic rules.

**Design.**
- Add `src/detector/slop_spans.rs`: run GLiNER (`gline-rs` + `ort` +
  `tokenizers`) with the five labels; map extracted spans to findings with a
  distinct family/rule id (e.g. `slop-span-<label>`, family ai-slop,
  advisory).
- These are *additive* leads over the deterministic rules — do not let them
  change exit codes (advisory only), consistent with "leads, not verdicts."

**Acceptance.** `detect --models --slop-spans` on marketing copy surfaces
buzzword/hype spans the deterministic rules missed; without the flag, output
is unchanged.

**Effort.** L (GLiNER model + `gline-rs` integration + label calibration).

---

## 3.4 — Audit-KB contradiction detection via NLI (P2, M)

**What.** SPEC §5.3 `audit kb`: "contradiction candidates (near-duplicate
embeddings, plus NLI contradiction when models are available)". The
near-duplicate embedding half depends on #1 embeddings working; the NLI half
depends on 3.1.

**Where.** `src/curation.rs::audit_kb`.

**Design.** For near-duplicate chunk pairs (cosine over the Lance vectors),
run NLI; report high-contradiction pairs as `contradiction-candidate`.

**Acceptance.** `audit kb` on a corpus with two docs stating contradictory
prices flags the pair.

**Effort.** M (mostly gated on 3.1 + working embeddings).

---

## Runtime decision (do this before 3.1)

Choose **one** ML runtime for Tier 1 and document it in SPEC §22:

- **Option A — ONNX Runtime (`ort`) + `tokenizers` + `gline-rs`.** Matches
  SPEC's stated crates; best model availability for NLI cross-encoders and
  GLiNER. Cost: a second heavy runtime (ONNX) alongside llama.cpp; larger
  binary; another cross-compile surface.
- **Option B — llama.cpp/GGUF for everything.** One runtime already in the
  tree. NLI-as-generation is possible (prompt a small instruct model for
  entailment) but less precise and slower than a purpose-built cross-encoder;
  GLiNER has no GGUF path, so slop-spans would need a different approach
  (e.g. a prompted extraction, which blurs the "deterministic vs model" line).
- **Recommendation.** A-for-NLI-and-GLiNER, reuse-llama-for-perplexity. The
  perplexity blend (3.2) is nearly free on the already-loaded attention
  model; NLI and slop-spans genuinely want `ort`. Accept the second runtime,
  feature-gate it (`--features ml`) so the default build stays lean.

## Cross-cutting for this tier

- **Model provisioning.** Each Tier-1 model is another download; fold into the
  `mari model pull` story (`05` #4) with pinned revisions + checksums (`07`).
- **CI.** Real-inference tests only run where the models are cached
  (`09-testing-ci.md`); unit-test the plumbing (thresholds, blend math,
  finding shapes) without the models.
