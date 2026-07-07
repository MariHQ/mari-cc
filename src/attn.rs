//! Attention engine (SPEC §17 Tier 2), harvested from the mari-cli native
//! extractor (`native/attn/main.cpp`) and ported to Rust over the
//! `llama-cpp-sys-2` FFI (the safe wrapper does not expose `cb_eval`).
//!
//! One mechanism — "how much does query text engage context text?" — drives
//! every attention feature:
//!   coverage  — flag CONTEXT spans the query barely attends to
//!               (i18n: source passages the translation dropped)
//!   grounding — flag QUERY rows that barely attend to the context
//!               (factcheck: sentences unanchored to the source)
//!
//! Faithful mechanics from the prototype: causal `<TASK>/<CONTEXT>/<QUERY>`
//! layout; `kq_soft_max-<layer>` capture via the graph eval callback (flash
//! attention disabled so the tensor exists); mid-band layers 0.60–0.88;
//! layer+head averaging; causal shift (query token i reads row i−1);
//! sink-column masking (log-median outliers > mean+3σ); ~10-token context
//! phrase chunks; runs below `threshold × peak` merge into flagged regions
//! (min 12 chars). Findings are leads, not verdicts.

use crate::{config, workspace};
use anyhow::{anyhow, Result};
use llama_cpp_sys_2 as ll;
use std::ffi::{c_void, CStr, CString};
use std::path::PathBuf;

pub const DEFAULT_MODEL_FILE: &str = "Qwen3.5-0.8B-Q4_K_M.gguf";
pub const DEFAULT_MODEL_URL: &str =
    "https://huggingface.co/unsloth/Qwen3.5-0.8B-GGUF/resolve/main/Qwen3.5-0.8B-Q4_K_M.gguf";
/// Expected SHA-256 of the default attention GGUF; empty disables verification.
pub const DEFAULT_MODEL_SHA256: &str = "";
const LAYER_BAND: (f64, f64) = (0.60, 0.88);
const PHRASE_TOKENS: usize = 10;
const MIN_FLAG_CHARS: usize = 12;
const UBATCH: usize = 512;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Coverage,
    Grounding,
    /// Inverse of coverage: the context runs the query attends to MOST.
    Focus,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Flagged {
    /// Fraction of the peak attention mass this region received (0..1).
    pub score: f64,
    /// Char offset into the flagged side's text (context for coverage,
    /// query for grounding).
    pub offset: usize,
    pub text: String,
}

/// Resolve the attention model: `attention.model` config path, else a GGUF in
/// `~/.mari/models` (preferring the prototype's qwen choice, excluding the
/// embedding model), else auto-download the default (config
/// `attention.auto_download`, default true).
pub fn model_file() -> Result<PathBuf> {
    let cfg = config::resolve(Some(&workspace::work_root()));
    if let Some(p) = cfg["attention"]["model"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
    {
        let path = PathBuf::from(p);
        if path.exists() {
            return Ok(path);
        }
        return Err(anyhow!("attention.model points at a missing file: {p}"));
    }
    let dir = config::mari_home().join("models");
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("gguf"))
                .filter(|p| {
                    let n = p
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase();
                    // The embedding model is not a generative attention model.
                    !n.contains("retrieval") && !n.contains("embed")
                })
                .collect()
        })
        .unwrap_or_default();
    candidates.sort();
    if let Some(qwen) = candidates.iter().find(|p| {
        p.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase()
            .contains("qwen")
    }) {
        return Ok(qwen.clone());
    }
    if let Some(first) = candidates.first() {
        return Ok(first.clone());
    }
    // Auto-download the default through the shared, checksum-verified,
    // resumable provisioner (§7 security).
    crate::models::ensure_gguf(&model_spec())
}

/// The download/verify spec for the default attention model.
pub fn model_spec() -> crate::models::ModelSpec {
    let auto = config::resolve(Some(&workspace::work_root()))["attention"]["auto_download"]
        .as_bool()
        .unwrap_or(true);
    crate::models::ModelSpec {
        file: DEFAULT_MODEL_FILE,
        url: DEFAULT_MODEL_URL,
        sha256: DEFAULT_MODEL_SHA256,
        approx_mb: 520,
        kind: "attention",
        auto_download: auto,
    }
}

const TASK_TRANSLATION: &str =
    "Match each translated passage to the source passages it translates. \
Focus on meaning, not shared words.";
const TASK_GROUNDING: &str = "Find evidence in the context for each query statement. \
Prefer exact supporting passages. Focus on semantic matches, not shared words.";

pub fn default_task(mode: Mode) -> &'static str {
    match mode {
        Mode::Coverage => TASK_TRANSLATION,
        Mode::Grounding | Mode::Focus => TASK_GROUNDING,
    }
}

// ---------------------------------------------------------------------------
// Capture state (the Rust CaptureContext)
// ---------------------------------------------------------------------------

struct Capture {
    selected_layers: std::collections::HashSet<i32>,
    /// Global sequence positions of context tokens (contiguous range).
    ctx_start: i32,
    ctx_len: usize,
    /// Causal-shifted query row positions: [q_start-1, q_end-1).
    qrow_start: i32,
    qrow_len: usize,
    /// Start position of the batch currently being decoded.
    batch_start: i32,
    /// Accumulated attention: [qrow_len × ctx_len], summed over layers+heads.
    sum: Vec<f32>,
    n_heads: usize,
    layers_seen: std::collections::HashSet<i32>,
}

/// The ggml graph eval callback — the heart of the harvest. Matches
/// `kq_soft_max-<layer>` (shape [n_kv, n_tokens, n_heads]) and accumulates
/// the selected layers' rows for query positions into `sum`.
unsafe extern "C" fn eval_callback(
    t: *mut ll::ggml_tensor,
    ask: bool,
    user_data: *mut c_void,
) -> bool {
    let cap = &mut *(user_data as *mut Capture);
    let name = CStr::from_ptr((*t).name.as_ptr());
    let Ok(name) = name.to_str() else {
        return false;
    };
    let Some(rest) = name.strip_prefix("kq_soft_max-") else {
        return false;
    };
    let Ok(layer) = rest.parse::<i32>() else {
        return false;
    };
    if !cap.selected_layers.contains(&layer) {
        return false;
    }
    if ask {
        return true; // yes, we want this tensor's data
    }
    // ne0 = n_kv (cache columns), ne1 = n_tokens (this ubatch), ne2 = heads.
    let n_kv = (*t).ne[0] as usize;
    let n_tokens = (*t).ne[1] as usize;
    let n_heads = (*t).ne[2] as usize;
    if (*t).type_ != ll::GGML_TYPE_F32 {
        return true; // unexpected precision; skip rather than misread
    }
    let bytes = ll::ggml_nbytes(t);
    let mut host = vec![0f32; bytes / std::mem::size_of::<f32>()];
    ll::ggml_backend_tensor_get(t, host.as_mut_ptr() as *mut c_void, 0, bytes);

    cap.n_heads = n_heads;
    cap.layers_seen.insert(layer);
    for q_local in 0..n_tokens {
        let global = cap.batch_start + q_local as i32;
        let row = global - cap.qrow_start;
        if row < 0 || row as usize >= cap.qrow_len {
            continue;
        }
        let out_base = row as usize * cap.ctx_len;
        for h in 0..n_heads {
            let in_base = h * n_tokens * n_kv + q_local * n_kv;
            for c in 0..cap.ctx_len {
                let col = (cap.ctx_start as usize) + c;
                if col < n_kv {
                    cap.sum[out_base + c] += host[in_base + col];
                }
            }
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Tokenization with char offsets (piece round-trip, as the prototype)
// ---------------------------------------------------------------------------

struct Tokenized {
    tokens: Vec<ll::llama_token>,
    /// (start, end) byte offsets into the source text per token.
    offsets: Vec<(usize, usize)>,
}

unsafe fn tokenize(vocab: *const ll::llama_vocab, text: &str) -> Result<Tokenized> {
    let c = CString::new(text.replace('\0', " "))?;
    let max = text.len() as i32 + 16;
    let mut tokens = vec![0 as ll::llama_token; max as usize];
    let n = ll::llama_tokenize(
        vocab,
        c.as_ptr(),
        text.len() as i32,
        tokens.as_mut_ptr(),
        max,
        false,
        false,
    );
    if n < 0 {
        return Err(anyhow!("tokenization failed ({n})"));
    }
    tokens.truncate(n as usize);
    // Offsets by accumulating piece byte lengths.
    let mut offsets = Vec::with_capacity(tokens.len());
    let mut pos = 0usize;
    let mut buf = vec![0i8; 256];
    for &tok in &tokens {
        let len =
            ll::llama_token_to_piece(vocab, tok, buf.as_mut_ptr(), buf.len() as i32, 0, false);
        let piece_len = if len > 0 { len as usize } else { 0 };
        let end = (pos + piece_len).min(text.len());
        offsets.push((pos.min(text.len()), end));
        pos = end;
    }
    Ok(Tokenized { tokens, offsets })
}

// ---------------------------------------------------------------------------
// The scan
// ---------------------------------------------------------------------------

/// Run one attention scan: context before query, causal. Returns the flagged
/// regions per `mode` (context spans for coverage, query spans for grounding).
pub fn analyze(
    context: &str,
    query: &str,
    mode: Mode,
    threshold: f64,
    task: Option<&str>,
) -> Result<Vec<Flagged>> {
    let model_path = model_file()?;
    let task = task.unwrap_or(default_task(mode));
    let prefix_text = format!("<TASK>\n{task}\n</TASK>\n\n<CONTEXT>\n");
    let middle_text = "\n</CONTEXT>\n<QUERY>\n";
    let close_text = "\n</QUERY>";

    let gpu_layers = config::resolve(Some(&workspace::work_root()))["attention"]["gpu_layers"]
        .as_i64()
        .unwrap_or(999) as i32;
    unsafe {
        ll::llama_backend_init();
        let mut mparams = ll::llama_model_default_params();
        // Configurable GPU offload (§8.3): default 999 offloads all layers,
        // clamped by llama.cpp; CPU fallback when no GPU is present.
        mparams.n_gpu_layers = gpu_layers;
        let cpath = CString::new(model_path.to_string_lossy().as_bytes())?;
        let model = ll::llama_model_load_from_file(cpath.as_ptr(), mparams);
        if model.is_null() {
            return Err(anyhow!(
                "failed to load attention model {}",
                model_path.display()
            ));
        }
        let vocab = ll::llama_model_get_vocab(model);

        let prefix = tokenize(vocab, &prefix_text)?;
        let ctx_toks = tokenize(vocab, context)?;
        let middle = tokenize(vocab, middle_text)?;
        let query_toks = tokenize(vocab, query)?;
        let close = tokenize(vocab, close_text)?;

        let add_bos = ll::llama_vocab_get_add_bos(vocab);
        let bos = ll::llama_vocab_bos(vocab);
        let mut seq: Vec<ll::llama_token> = Vec::new();
        if add_bos {
            seq.push(bos);
        }
        let base = seq.len() as i32;
        seq.extend(&prefix.tokens);
        let ctx_start = base + prefix.tokens.len() as i32;
        seq.extend(&ctx_toks.tokens);
        seq.extend(&middle.tokens);
        let q_start = ctx_start + ctx_toks.tokens.len() as i32 + middle.tokens.len() as i32;
        seq.extend(&query_toks.tokens);
        seq.extend(&close.tokens);
        let total = seq.len();

        // Layer band 0.60–0.88 (§ prototype defaults).
        let n_layer = ll::llama_model_n_layer(model);
        let lo = (n_layer as f64 * LAYER_BAND.0).floor() as i32;
        let hi = (n_layer as f64 * LAYER_BAND.1).ceil() as i32;
        let selected: std::collections::HashSet<i32> = (lo..=hi.min(n_layer - 1)).collect();

        let qrow_len = query_toks.tokens.len();
        let cap = Box::new(Capture {
            selected_layers: selected,
            ctx_start,
            ctx_len: ctx_toks.tokens.len(),
            qrow_start: q_start - 1, // causal shift: query token i reads row i−1
            qrow_len,
            batch_start: 0,
            sum: vec![0f32; qrow_len * ctx_toks.tokens.len()],
            n_heads: 0,
            layers_seen: std::collections::HashSet::new(),
        });
        let cap_ptr = Box::into_raw(cap);

        let mut cparams = ll::llama_context_default_params();
        cparams.n_ctx = (total + 8) as u32;
        cparams.n_batch = UBATCH as u32;
        cparams.n_ubatch = UBATCH as u32;
        cparams.flash_attn_type = ll::LLAMA_FLASH_ATTN_TYPE_DISABLED;
        cparams.cb_eval = Some(eval_callback);
        cparams.cb_eval_user_data = cap_ptr as *mut c_void;
        let lctx = ll::llama_init_from_model(model, cparams);
        if lctx.is_null() {
            drop(Box::from_raw(cap_ptr));
            ll::llama_model_free(model);
            return Err(anyhow!(
                "failed to create attention context (sequence: {total} tokens)"
            ));
        }

        // Decode sequentially in UBATCH slices; the callback maps each
        // slice's rows by batch_start.
        let batch = ll::llama_batch_init(UBATCH as i32, 0, 1);
        let mut pos = 0usize;
        let mut decode_err = None;
        while pos < total {
            let n = UBATCH.min(total - pos);
            (*cap_ptr).batch_start = pos as i32;
            for i in 0..n {
                *batch.token.add(i) = seq[pos + i];
                *batch.pos.add(i) = (pos + i) as i32;
                *batch.n_seq_id.add(i) = 1;
                *(*batch.seq_id.add(i)).add(0) = 0;
                *batch.logits.add(i) = 0;
            }
            let mut b = batch;
            b.n_tokens = n as i32;
            let rc = ll::llama_decode(lctx, b);
            if rc != 0 {
                decode_err = Some(anyhow!(
                    "attention decode failed (rc={rc}) at {pos}/{total} tokens"
                ));
                break;
            }
            pos += n;
        }
        ll::llama_batch_free(batch);
        ll::llama_free(lctx);
        ll::llama_model_free(model);
        let cap = Box::from_raw(cap_ptr);
        if let Some(e) = decode_err {
            return Err(e);
        }
        if cap.layers_seen.is_empty() {
            return Err(anyhow!(
                "no attention captured — the model graph exposed no kq_soft_max tensors"
            ));
        }

        // Average layers + heads.
        let denom = (cap.layers_seen.len() * cap.n_heads.max(1)) as f32;
        let n_c = cap.ctx_len;
        let n_q = cap.qrow_len;
        let mut matrix: Vec<Vec<f32>> = (0..n_q)
            .map(|q| {
                cap.sum[q * n_c..(q + 1) * n_c]
                    .iter()
                    .map(|v| v / denom)
                    .collect()
            })
            .collect();
        matrix = apply_sink_norm(matrix);

        match mode {
            Mode::Coverage => Ok(coverage_findings(
                &matrix, &ctx_toks, context, threshold, false,
            )),
            Mode::Focus => Ok(coverage_findings(
                &matrix, &ctx_toks, context, threshold, true,
            )),
            Mode::Grounding => Ok(grounding_findings(&matrix, &query_toks, query, threshold)),
        }
    }
}

/// Sink-column masking (port of `apply_sink_norm`): zero context columns
/// whose log column-median is a > mean+3σ outlier. Preserves absolute mass.
fn apply_sink_norm(matrix: Vec<Vec<f32>>) -> Vec<Vec<f32>> {
    let rows = matrix.len();
    if rows == 0 {
        return matrix;
    }
    let cols = matrix[0].len();
    if cols == 0 {
        return matrix;
    }
    let mut col_median = vec![0f32; cols];
    let mut column = vec![0f32; rows];
    for c in 0..cols {
        for r in 0..rows {
            column[r] = matrix[r][c];
        }
        column.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        col_median[c] = column[rows / 2];
    }
    let mut min_positive = 1e-12f64;
    for &v in &col_median {
        if v > 0.0 && (v as f64) < min_positive {
            min_positive = v as f64;
        }
    }
    let logs: Vec<f64> = col_median
        .iter()
        .map(|&v| ((v as f64) + min_positive / 2.0 + 1e-12).ln())
        .collect();
    let mean = logs.iter().sum::<f64>() / cols as f64;
    let var = logs.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / cols as f64;
    let threshold = mean + 3.0 * var.sqrt();
    let sink: Vec<bool> = logs.iter().map(|&v| v > threshold).collect();
    if !sink.iter().any(|&s| s) {
        return matrix;
    }
    matrix
        .into_iter()
        .map(|row| {
            row.into_iter()
                .enumerate()
                .map(|(c, v)| if sink[c] { 0.0 } else { v })
                .collect()
        })
        .collect()
}

/// Coverage (port of `write_mari_coverage`): per ~10-token context phrase
/// chunk, total attention received from all query rows; runs below
/// `threshold × peak` merge into flagged regions.
fn coverage_findings(
    matrix: &[Vec<f32>],
    ctx_toks: &Tokenized,
    context: &str,
    threshold: f64,
    focus: bool,
) -> Vec<Flagged> {
    let n_c = ctx_toks.tokens.len();
    let mut per_token = vec![0f64; n_c];
    for row in matrix {
        for (c, v) in row.iter().enumerate() {
            per_token[c] += *v as f64;
        }
    }
    // Phrase chunks of ~10 tokens.
    let mut chunks: Vec<(usize, usize, f64)> = Vec::new(); // (byte start, byte end, mass)
    let mut i = 0usize;
    while i < n_c {
        let j = (i + PHRASE_TOKENS).min(n_c);
        let start = ctx_toks.offsets[i].0;
        let end = ctx_toks.offsets[j - 1].1;
        let mass: f64 = per_token[i..j].iter().sum();
        chunks.push((start, end, mass));
        i = j;
    }
    if focus {
        flag_high_runs(&chunks, context, threshold)
    } else {
        flag_low_runs(&chunks, context, threshold, false)
    }
}

/// Focus (port of `write_mari_focus`): merge adjacent AT-OR-ABOVE-threshold
/// chunks into regions scored by the run's max — where attention concentrates.
fn flag_high_runs(chunks: &[(usize, usize, f64)], text: &str, threshold: f64) -> Vec<Flagged> {
    let peak = chunks.iter().map(|c| c.2).fold(1e-9f64, f64::max);
    let mut out = Vec::new();
    let mut run: Option<(usize, usize, f64)> = None; // (start, end, max score)
    let close = |run: &mut Option<(usize, usize, f64)>, out: &mut Vec<Flagged>| {
        if let Some((s, e, score)) = run.take() {
            let snippet = floor_str(text, s, e);
            if snippet.chars().count() >= MIN_FLAG_CHARS {
                out.push(Flagged {
                    score,
                    offset: s,
                    text: snippet.chars().take(480).collect(),
                });
            }
        }
    };
    for &(s, e, mass) in chunks {
        let n = mass / peak;
        if n >= threshold {
            match &mut run {
                Some((_, end, max)) => {
                    *end = e;
                    *max = max.max(n);
                }
                None => run = Some((s, e, n)),
            }
        } else {
            close(&mut run, &mut out);
        }
    }
    close(&mut run, &mut out);
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// Grounding (per-row totals): query sentence groups whose attention into
/// the context falls below `threshold × peak`.
fn grounding_findings(
    matrix: &[Vec<f32>],
    query_toks: &Tokenized,
    query: &str,
    threshold: f64,
) -> Vec<Flagged> {
    let n_q = query_toks.tokens.len();
    let row_total: Vec<f64> = (0..n_q)
        .map(|q| matrix[q].iter().map(|v| *v as f64).sum())
        .collect();
    // Sentence groups over the query text.
    let bounds = sentence_bounds(query);
    let mut chunks: Vec<(usize, usize, f64)> = Vec::new();
    for (s, e) in bounds {
        let rows: Vec<usize> = (0..n_q)
            .filter(|&q| {
                let (ts, te) = query_toks.offsets[q];
                ts < e && te > s
            })
            .collect();
        if rows.is_empty() {
            continue;
        }
        let mass = rows.iter().map(|&q| row_total[q]).sum::<f64>() / rows.len() as f64;
        chunks.push((s, e, mass));
    }
    flag_low_runs(&chunks, query, threshold, true)
}

fn sentence_bounds(text: &str) -> Vec<(usize, usize)> {
    let re = regex::Regex::new(r"[.!?]+[\s\n]+|\n\n+").unwrap();
    let mut out = Vec::new();
    let mut start = 0usize;
    for m in re.find_iter(text) {
        if m.end() > start {
            out.push((start, m.end()));
        }
        start = m.end();
    }
    if start < text.len() {
        out.push((start, text.len()));
    }
    out
}

/// Merge adjacent below-threshold chunks into flagged regions (min 12 chars),
/// scores relative to the peak chunk mass.
fn flag_low_runs(
    chunks: &[(usize, usize, f64)],
    text: &str,
    threshold: f64,
    per_chunk: bool,
) -> Vec<Flagged> {
    let peak = chunks.iter().map(|c| c.2).fold(1e-9f64, f64::max);
    let mut out = Vec::new();
    let mut run: Option<(usize, usize, f64)> = None; // (start, end, min score)
    let close = |run: &mut Option<(usize, usize, f64)>, out: &mut Vec<Flagged>| {
        if let Some((s, e, score)) = run.take() {
            let snippet = floor_str(text, s, e);
            if snippet.chars().count() >= MIN_FLAG_CHARS {
                out.push(Flagged {
                    score,
                    offset: s,
                    text: snippet.chars().take(240).collect(),
                });
            }
        }
    };
    for &(s, e, mass) in chunks {
        let n = mass / peak;
        if n < threshold {
            match &mut run {
                Some((_, end, min)) if !per_chunk => {
                    *end = e;
                    *min = min.min(n);
                }
                Some(_) => {
                    close(&mut run, &mut out);
                    run = Some((s, e, n));
                }
                None => run = Some((s, e, n)),
            }
        } else {
            close(&mut run, &mut out);
        }
    }
    close(&mut run, &mut out);
    out
}

fn floor_str(text: &str, mut s: usize, mut e: usize) -> String {
    s = s.min(text.len());
    e = e.min(text.len());
    while s > 0 && !text.is_char_boundary(s) {
        s -= 1;
    }
    while e > s && !text.is_char_boundary(e) {
        e -= 1;
    }
    text[s..e].to_string()
}

/// Approximate 1-based line of a char offset.
pub fn line_of_offset(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())]
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
        + 1
}

/// Machine-likelihood `m ∈ [0,1]` for the slop score's model blend (SPEC §12
/// step 5 / §17 Tier 1). Reuses the local attention model to compute the
/// document's mean per-token cross-entropy (log-perplexity) and maps it: text
/// the model finds highly predictable (low perplexity) reads more
/// machine-generated → higher `m`. This is an explainable *signal*, never an
/// assertion that text is AI-written (§13.4). Returns None (loudly) when the
/// model is unavailable.
/// Map mean bits/token to `m ∈ [0,1]`: human technical prose sits around
/// 3.5–5.5 bits/token; heavily templated text is lower (more predictable →
/// more machine-like). Logistic centered at 4.5 bits.
fn squash_bits_to_likelihood(bits: f64) -> f64 {
    (1.0 / (1.0 + ((bits - 4.5) * 1.1).exp())).clamp(0.0, 1.0)
}

pub fn machine_likelihood(text: &str) -> Option<f64> {
    match perplexity_bits(text) {
        Ok(Some(bits)) => Some(squash_bits_to_likelihood(bits)),
        Ok(None) => None,
        Err(e) => {
            eprintln!("note: machine-likelihood unavailable ({e:#})");
            None
        }
    }
}

/// Mean cross-entropy in bits/token over the document, via the local model's
/// next-token logits. None when the text is too short to score.
fn perplexity_bits(text: &str) -> Result<Option<f64>> {
    const MAX_TOKENS: usize = 1024;
    let model_path = model_file()?;
    let gpu_layers = config::resolve(Some(&workspace::work_root()))["attention"]["gpu_layers"]
        .as_i64()
        .unwrap_or(999) as i32;
    unsafe {
        ll::llama_backend_init();
        let mut mparams = ll::llama_model_default_params();
        mparams.n_gpu_layers = gpu_layers;
        let cpath = CString::new(model_path.to_string_lossy().as_bytes())?;
        let model = ll::llama_model_load_from_file(cpath.as_ptr(), mparams);
        if model.is_null() {
            return Err(anyhow!("failed to load attention model for perplexity"));
        }
        let vocab = ll::llama_model_get_vocab(model);
        let n_vocab = ll::llama_vocab_n_tokens(vocab) as usize;
        let tokens = tokenize(vocab, text)?.tokens;
        if tokens.len() < 8 {
            ll::llama_model_free(model);
            return Ok(None);
        }
        let tokens: Vec<ll::llama_token> = tokens.into_iter().take(MAX_TOKENS).collect();
        let n = tokens.len();

        let mut cparams = ll::llama_context_default_params();
        cparams.n_ctx = (n + 8) as u32;
        cparams.n_batch = n as u32;
        cparams.n_ubatch = n as u32;
        let lctx = ll::llama_init_from_model(model, cparams);
        if lctx.is_null() {
            ll::llama_model_free(model);
            return Err(anyhow!("failed to create perplexity context"));
        }

        // Decode the whole sequence, requesting logits at every position.
        let batch = ll::llama_batch_init(n as i32, 0, 1);
        for (i, &tok) in tokens.iter().enumerate() {
            *batch.token.add(i) = tok;
            *batch.pos.add(i) = i as i32;
            *batch.n_seq_id.add(i) = 1;
            *(*batch.seq_id.add(i)).add(0) = 0;
            *batch.logits.add(i) = 1;
        }
        let mut b = batch;
        b.n_tokens = n as i32;
        let rc = ll::llama_decode(lctx, b);
        if rc != 0 {
            ll::llama_batch_free(batch);
            ll::llama_free(lctx);
            ll::llama_model_free(model);
            return Err(anyhow!("perplexity decode failed (rc={rc})"));
        }

        // Cross-entropy of the actual next token given each position's logits.
        let mut total_bits = 0.0f64;
        let mut counted = 0usize;
        for i in 0..n - 1 {
            let logits = ll::llama_get_logits_ith(lctx, i as i32);
            if logits.is_null() {
                continue;
            }
            let slice = std::slice::from_raw_parts(logits, n_vocab);
            // log-softmax at the target token, numerically stable.
            let max = slice.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let mut sum = 0.0f64;
            for &l in slice {
                sum += ((l - max) as f64).exp();
            }
            let target = tokens[i + 1] as usize;
            if target >= n_vocab {
                continue;
            }
            let log_p = (slice[target] - max) as f64 - sum.ln();
            total_bits += -log_p / std::f64::consts::LN_2;
            counted += 1;
        }
        ll::llama_batch_free(batch);
        ll::llama_free(lctx);
        ll::llama_model_free(model);
        if counted == 0 {
            return Ok(None);
        }
        Ok(Some(total_bits / counted as f64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_likelihood_squash_is_monotone_and_bounded() {
        // Lower perplexity (more predictable) → higher machine-likelihood.
        let low = squash_bits_to_likelihood(2.0);
        let mid = squash_bits_to_likelihood(4.5);
        let high = squash_bits_to_likelihood(8.0);
        assert!(low > mid && mid > high);
        assert!((0.0..=1.0).contains(&low) && (0.0..=1.0).contains(&high));
        assert!((mid - 0.5).abs() < 1e-9); // centered at 4.5 bits
    }

    #[test]
    fn sink_norm_masks_outlier_columns() {
        // One column with enormous median gets zeroed; others survive.
        let mut m = vec![vec![0.01f32; 40]; 6];
        for row in &mut m {
            row[7] = 50.0;
        }
        let out = apply_sink_norm(m);
        assert_eq!(out[0][7], 0.0);
        assert!(out[0][6] > 0.0);
    }

    #[test]
    fn low_runs_merge_and_respect_min_chars() {
        // chunks: high, low, low, high — the two lows merge into one region.
        let text = "aaaaaaaaaa bbbbbbbbbb cccccccccc dddddddddd";
        let chunks = vec![
            (0usize, 10usize, 1.0f64),
            (11, 21, 0.05),
            (22, 32, 0.02),
            (33, 43, 0.9),
        ];
        let flags = flag_low_runs(&chunks, text, 0.3, false);
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].offset, 11);
        assert!(flags[0].text.contains("bbbbbbbbbb"));
        assert!((flags[0].score - 0.02).abs() < 1e-9);
        // Tiny fragments are suppressed.
        let flags = flag_low_runs(&[(0, 4, 0.0), (5, 43, 1.0)], text, 0.3, false);
        assert!(flags.is_empty());
    }

    #[test]
    fn high_runs_capture_focus_regions() {
        let text = "aaaaaaaaaa bbbbbbbbbb cccccccccc dddddddddd";
        let chunks = vec![
            (0usize, 10usize, 0.1f64),
            (11, 21, 1.0),
            (22, 32, 0.8),
            (33, 43, 0.05),
        ];
        let flags = flag_high_runs(&chunks, text, 0.5);
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].offset, 11);
        assert!(flags[0].text.contains("cccccccccc"));
        assert!((flags[0].score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn sentence_bounds_split_prose() {
        let b = sentence_bounds("One sentence. Another one!\n\nA third paragraph");
        assert_eq!(b.len(), 3);
        assert_eq!(
            &"One sentence. Another one!\n\nA third paragraph"[b[2].0..b[2].1],
            "A third paragraph"
        );
    }
}
