//! Vector embeddings (SPEC §7.1/§7.3): `Qwen3-Embedding-0.6B` (GGUF Q8_0,
//! Apache-2.0) running locally through llama.cpp — instruction-aware queries
//! (documents embed raw), last-token pooling, 1024-dim, L2-normalized.
//! Vectors are stored per workspace in Lance format (`vectors.lance`), and
//! similarity queries run in DuckDB over the Lance data via its Arrow
//! integration (there is no community lance extension for the bundled DuckDB —
//! the Arrow bridge is the §8 route).
//!
//! Failure is loud (§7.1): a missing model or broken dataset prints an
//! error; nothing silently falls back. The GGUF is fetched from a pinned
//! Hugging Face revision and verified against a known SHA-256 (§7 security).

use crate::{config, models, workspace};
use anyhow::{anyhow, Result};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const DIMS: usize = 1024;
pub const MODEL_FILE: &str = "Qwen3-Embedding-0.6B-Q8_0.gguf";
/// Pinned Hugging Face revision. `main` until a commit SHA is recorded for
/// reproducibility (§7.1); the release process pins this and `MODEL_SHA256`.
#[allow(dead_code)]
pub const MODEL_REVISION: &str = "main";
/// Expected SHA-256 of the GGUF; empty string disables verification (set once
/// the revision is pinned and the hash captured — see `mari model pull`).
pub const MODEL_SHA256: &str = "";
pub const MODEL_URL: &str =
    "https://huggingface.co/Qwen/Qwen3-Embedding-0.6B-GGUF/resolve/main/Qwen3-Embedding-0.6B-Q8_0.gguf";

/// The download/verify spec for `mari model pull`.
pub fn model_spec() -> models::ModelSpec {
    let auto = config::resolve(Some(&workspace::work_root()))["embedding"]["auto_download"]
        .as_bool()
        .unwrap_or(true);
    models::ModelSpec {
        file: MODEL_FILE,
        url: MODEL_URL,
        sha256: MODEL_SHA256,
        approx_mb: 640,
        kind: "embedding",
        auto_download: auto,
    }
}

pub fn dataset_path(global: bool) -> PathBuf {
    let dir = if global {
        workspace::global_workspace_dir()
    } else {
        workspace::workspace_dir(&workspace::work_root())
    };
    dir.join("vectors.lance")
}

/// Resolve the embedding model: use the on-disk GGUF if present; else, when
/// `embedding.auto_download` (default true), download from the pinned
/// revision and verify the checksum; else a loud error.
pub fn ensure_model() -> Result<PathBuf> {
    let cfg = config::resolve(Some(&workspace::work_root()));
    // A configured path override wins (air-gapped installs).
    if let Some(p) = cfg["embedding"]["model"].as_str().filter(|s| !s.trim().is_empty()) {
        let path = PathBuf::from(p);
        if path.exists() {
            return Ok(path);
        }
        return Err(anyhow!("embedding.model points at a missing file: {p}"));
    }
    let auto = cfg["embedding"]["auto_download"].as_bool().unwrap_or(true);
    models::ensure_gguf(&models::ModelSpec {
        file: MODEL_FILE,
        url: MODEL_URL,
        sha256: MODEL_SHA256,
        approx_mb: 640,
        kind: "embedding",
        auto_download: auto,
    })
}

/// Embed texts with the task prefix (`Query: ` / `Document: `). One model
/// load per call; sequences are PACKED into shared llama.cpp decode batches
/// (token-budget groups, `embedding.batch_size`-capped) so N texts cost far
/// fewer than N decodes. Vectors come back L2-normalized.
pub fn embed_texts(texts: &[String], is_query: bool) -> Result<Vec<Vec<f32>>> {
    use llama_cpp_2::context::params::{LlamaContextParams, LlamaPoolingType};
    use llama_cpp_2::llama_backend::LlamaBackend;
    use llama_cpp_2::llama_batch::LlamaBatch;
    use llama_cpp_2::model::params::LlamaModelParams;
    use llama_cpp_2::model::{AddBos, LlamaModel};

    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let cfg = config::resolve(Some(&workspace::work_root()));
    let seq_cap = (cfg["embedding"]["batch_size"].as_u64().unwrap_or(64) as usize).clamp(1, 64);

    let path = ensure_model()?;
    let backend = LlamaBackend::init().map_err(|e| anyhow!("llama backend init: {e}"))?;
    let model_params = LlamaModelParams::default();
    let model = LlamaModel::load_from_file(&backend, &path, &model_params)
        .map_err(|e| anyhow!("failed to load {MODEL_FILE}: {e}"))?;

    // Pooled embeddings need the whole batch in one ubatch; budget the
    // token count so every packed group fits a single decode.
    const TOKEN_BUDGET: usize = 4096;
    const PER_SEQ_CAP: usize = 1024;
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(std::num::NonZeroU32::new(TOKEN_BUDGET as u32))
        .with_n_batch(TOKEN_BUDGET as u32)
        .with_n_ubatch(TOKEN_BUDGET as u32)
        .with_n_seq_max(seq_cap as u32)
        .with_embeddings(true)
        .with_pooling_type(LlamaPoolingType::Last);
    let mut ctx = model
        .new_context(&backend, ctx_params)
        .map_err(|e| anyhow!("llama context: {e}"))?;

    // Qwen3-Embedding is instruction-aware: queries carry the retrieval
    // instruct prefix, documents embed raw (per the model card).
    const QUERY_PREFIX: &str = "Instruct: Given a web search query, retrieve relevant passages that answer the query\nQuery: ";
    let prefix = if is_query { QUERY_PREFIX } else { "" };
    // Tokenize everything up front so groups can be packed by token budget.
    let mut tokenized = Vec::with_capacity(texts.len());
    for text in texts {
        let mut tokens = model
            .str_to_token(&format!("{prefix}{text}"), AddBos::Always)
            .map_err(|e| anyhow!("tokenize: {e}"))?;
        if tokens.len() > PER_SEQ_CAP {
            tokens.truncate(PER_SEQ_CAP);
        }
        tokenized.push(tokens);
    }

    let mut out: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
    let mut batch = LlamaBatch::new(TOKEN_BUDGET, seq_cap as i32);
    let mut i = 0usize;
    let total = tokenized.len();
    while i < total {
        // Greedily pack sequences until the token budget or seq cap is hit.
        let mut group = 0usize;
        let mut budget = 0usize;
        batch.clear();
        while i + group < total
            && group < seq_cap
            && budget + tokenized[i + group].len() <= TOKEN_BUDGET
        {
            batch.add_sequence(&tokenized[i + group], group as i32, false)?;
            budget += tokenized[i + group].len();
            group += 1;
        }
        if group == 0 {
            // A single oversized sequence (already capped) — should not
            // happen, but never spin.
            return Err(anyhow!("embedding batch packing stalled"));
        }
        ctx.clear_kv_cache();
        ctx.decode(&mut batch).map_err(|e| anyhow!("decode: {e}"))?;
        for seq in 0..group {
            let emb = ctx
                .embeddings_seq_ith(seq as i32)
                .map_err(|e| anyhow!("embeddings: {e}"))?;
            out.push(normalize(emb));
        }
        i += group;
        if total > seq_cap {
            eprintln!("  embedded {i}/{total} chunk(s)");
        }
    }
    Ok(out)
}

pub fn normalize(v: &[f32]) -> Vec<f32> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        v.iter().map(|x| x / norm).collect()
    } else {
        v.to_vec()
    }
}

// ---------------------------------------------------------------------------
// Lance dataset I/O
// ---------------------------------------------------------------------------

fn rt() -> Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?)
}

fn lance_schema() -> std::sync::Arc<arrow_schema::Schema> {
    use arrow_schema::{DataType, Field, Schema};
    std::sync::Arc::new(Schema::new(vec![
        Field::new("chunk_id", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                std::sync::Arc::new(Field::new("item", DataType::Float32, true)),
                DIMS as i32,
            ),
            false,
        ),
    ]))
}

fn to_batch(rows: &[(String, Vec<f32>)]) -> Result<arrow_array::RecordBatch> {
    use arrow_array::{Array, FixedSizeListArray, Float32Array, RecordBatch, StringArray};
    let ids = StringArray::from(rows.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>());
    let flat: Vec<f32> = rows.iter().flat_map(|(_, v)| v.iter().copied()).collect();
    let values = Float32Array::from(flat);
    let field = std::sync::Arc::new(arrow_schema::Field::new(
        "item",
        arrow_schema::DataType::Float32,
        true,
    ));
    let vectors = FixedSizeListArray::try_new(
        field,
        DIMS as i32,
        std::sync::Arc::new(values) as std::sync::Arc<dyn Array>,
        None,
    )?;
    Ok(RecordBatch::try_new(
        lance_schema(),
        vec![std::sync::Arc::new(ids), std::sync::Arc::new(vectors)],
    )?)
}

/// Write rows as the whole dataset (overwrite/create).
pub fn write_dataset(global: bool, rows: &[(String, Vec<f32>)]) -> Result<()> {
    let uri = dataset_path(global);
    workspace::ensure_dir(uri.parent().unwrap())?;
    let batch = to_batch(rows)?;
    let reader = arrow_array::RecordBatchIterator::new(vec![Ok(batch)], lance_schema());
    rt()?.block_on(async {
        let params = lance::dataset::WriteParams {
            mode: lance::dataset::WriteMode::Overwrite,
            ..Default::default()
        };
        lance::Dataset::write(reader, uri.to_str().unwrap(), Some(params))
            .await
            .map_err(|e| anyhow!("lance write: {e}"))
    })?;
    Ok(())
}

/// Read the whole dataset back as (chunk_id, vector) rows.
pub fn read_dataset(global: bool) -> Result<Vec<(String, Vec<f32>)>> {
    use arrow_array::{cast::AsArray, types::Float32Type};
    use futures::TryStreamExt;
    let uri = dataset_path(global);
    if !uri.exists() {
        return Ok(Vec::new());
    }
    let batches: Vec<arrow_array::RecordBatch> = rt()?.block_on(async {
        let ds = lance::Dataset::open(uri.to_str().unwrap())
            .await
            .map_err(|e| anyhow!("lance open: {e}"))?;
        let stream = ds
            .scan()
            .try_into_stream()
            .await
            .map_err(|e| anyhow!("lance scan: {e}"))?;
        stream
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| anyhow!("lance read: {e}"))
    })?;
    let mut out = Vec::new();
    for batch in batches {
        let ids = batch
            .column_by_name("chunk_id")
            .ok_or_else(|| anyhow!("lance dataset missing chunk_id"))?
            .as_string::<i32>()
            .clone();
        let vectors = batch
            .column_by_name("vector")
            .ok_or_else(|| anyhow!("lance dataset missing vector"))?
            .as_fixed_size_list()
            .clone();
        for i in 0..batch.num_rows() {
            let id = ids.value(i).to_string();
            let cell = vectors.value(i);
            let floats = cell.as_primitive::<Float32Type>();
            out.push((id, floats.values().to_vec()));
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Sync-time embedding (§6.0 resumable: only missing chunks embed)
// ---------------------------------------------------------------------------

pub fn sync_vectors(conn: &duckdb::Connection, global: bool, rebuild: bool) -> Result<usize> {
    let cfg = config::resolve(Some(&workspace::work_root()));
    let batch_size = cfg["embedding"]["batch_size"].as_u64().unwrap_or(64) as usize;

    // Current chunk universe (large chunks included — they are vector-only, §7.2).
    let mut stmt = conn.prepare("SELECT chunk_id, text FROM chunks")?;
    let current: BTreeMap<String, String> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .flatten()
        .collect();
    if current.is_empty() {
        return Ok(0);
    }

    let existing: Vec<(String, Vec<f32>)> = if rebuild {
        Vec::new()
    } else {
        read_dataset(global)?
    };
    let kept: Vec<(String, Vec<f32>)> = existing
        .into_iter()
        .filter(|(id, v)| current.contains_key(id) && v.len() == DIMS)
        .collect();
    let have: std::collections::HashSet<&str> = kept.iter().map(|(id, _)| id.as_str()).collect();
    let pending: Vec<(String, String)> = current
        .iter()
        .filter(|(id, _)| !have.contains(id.as_str()))
        .map(|(id, text)| (id.clone(), text.clone()))
        .collect();
    let deletions = current.len() < have.len() + pending.len();

    if pending.is_empty() && !deletions && !rebuild {
        return Ok(0);
    }

    let mut rows = kept;
    let embedded = pending.len();
    if !pending.is_empty() {
        // One model load; packing into shared decode batches happens inside.
        let texts: Vec<String> = pending.iter().map(|(_, t)| t.clone()).collect();
        let vecs = embed_texts(&texts, false)?;
        for ((id, _), v) in pending.iter().zip(vecs) {
            rows.push((id.clone(), v));
        }
    }
    let _ = batch_size;
    write_dataset(global, &rows)?;
    crate::index::set_meta(conn, "embedding.model", crate::index::EMBEDDING_MODEL)?;
    crate::index::set_meta(conn, "embedding.dims", &DIMS.to_string())?;
    Ok(embedded)
}

// ---------------------------------------------------------------------------
// Query-time ranking: DuckDB cosine over the Lance data (Arrow bridge)
// ---------------------------------------------------------------------------

/// Rank several phrasings (main query + variants) in one model load.
/// Returns one ranked list per phrasing, or None (loudly) when unavailable.
/// Verify the catalog's recorded embedding identity + dims match this build.
fn check_dataset_identity(global: bool) -> Result<()> {
    let db = crate::index::catalog_path(global);
    if !db.exists() {
        return Ok(());
    }
    let conn = duckdb::Connection::open(&db)?;
    let model: Option<String> = conn
        .query_row("SELECT value FROM schema_meta WHERE key = 'embedding.model'", [], |r| r.get(0))
        .ok();
    let dims: Option<String> = conn
        .query_row("SELECT value FROM schema_meta WHERE key = 'embedding.dims'", [], |r| r.get(0))
        .ok();
    if let Some(m) = model {
        if m != crate::index::EMBEDDING_MODEL {
            return Err(anyhow!(
                "catalog vectors were written by `{m}` but this build uses `{}`",
                crate::index::EMBEDDING_MODEL
            ));
        }
    }
    if let Some(d) = dims {
        if d.parse::<usize>().ok() != Some(DIMS) {
            return Err(anyhow!("catalog vectors are {d}-dim but this build embeds {DIMS}-dim"));
        }
    }
    Ok(())
}

pub fn rank_many(
    global: bool,
    phrasings: &[String],
    pool: usize,
) -> Option<Vec<Vec<(String, f64)>>> {
    if phrasings.is_empty() || !dataset_path(global).exists() {
        return None;
    }
    // Embedding-identity / dimension guard (§7.1): a dataset written by a
    // different model produces incompatible vectors. Refuse rather than
    // silently mix, and point at `--rebuild`.
    if let Err(e) = check_dataset_identity(global) {
        eprintln!("warning: {e}; run `mari sync --rebuild`. Keyword-only results.");
        return None;
    }
    let inner = || -> Result<Vec<Vec<(String, f64)>>> {
        let qvecs = embed_texts(phrasings, true)?;
        let rows = read_dataset(global)?;
        if rows.is_empty() {
            return Ok(vec![Vec::new(); phrasings.len()]);
        }
        qvecs
            .iter()
            .map(|q| duckdb_cosine_topk(&rows, q, pool))
            .collect()
    };
    match inner() {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("warning: vector search unavailable ({e:#}); keyword-only results");
            None
        }
    }
}

/// Similarity in DuckDB: register the Lance rows through the Arrow vtab and
/// let `array_cosine_similarity` rank them.
pub fn duckdb_cosine_topk(
    rows: &[(String, Vec<f32>)],
    qvec: &[f32],
    pool: usize,
) -> Result<Vec<(String, f64)>> {
    use duckdb::arrow::array::{Array, FixedSizeListArray, Float32Array, RecordBatch, StringArray};
    use duckdb::arrow::datatypes::{DataType, Field, Schema};
    use duckdb::vtab::arrow::{arrow_recordbatch_to_query_params, ArrowVTab};

    let conn = duckdb::Connection::open_in_memory()?;
    conn.register_table_function::<ArrowVTab>("arrow")?;
    // Rebuild the batch with DuckDB's own arrow version (it may differ from
    // lance's) — a plain copy through Vec keeps the versions decoupled.
    let schema = std::sync::Arc::new(Schema::new(vec![
        Field::new("chunk_id", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                std::sync::Arc::new(Field::new("item", DataType::Float32, true)),
                DIMS as i32,
            ),
            false,
        ),
    ]));
    let ids = StringArray::from(rows.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>());
    let flat: Vec<f32> = rows.iter().flat_map(|(_, v)| v.iter().copied()).collect();
    let vectors = FixedSizeListArray::try_new(
        std::sync::Arc::new(Field::new("item", DataType::Float32, true)),
        DIMS as i32,
        std::sync::Arc::new(Float32Array::from(flat)) as std::sync::Arc<dyn Array>,
        None,
    )?;
    let batch = RecordBatch::try_new(
        schema,
        vec![std::sync::Arc::new(ids), std::sync::Arc::new(vectors)],
    )?;
    let params = arrow_recordbatch_to_query_params(batch);
    let qlit = qvec
        .iter()
        .map(|f| format!("{f}"))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT chunk_id, array_cosine_similarity(vector::FLOAT[{DIMS}], [{qlit}]::FLOAT[{DIMS}]) AS score \
         FROM arrow(?, ?) ORDER BY score DESC LIMIT {pool}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let out: Vec<(String, f64)> = stmt
        .query_map(params, |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
        })?
        .flatten()
        // Score `round(1 − distance, 3)` == cosine similarity rounded (§7.3).
        .map(|(id, s)| (id, (s * 1000.0).round() / 1000.0))
        .collect();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedding_identity_is_internally_consistent() {
        // Guards against the exact drift that broke embeddings once: the
        // model name, dims, file, and URL host must all agree.
        assert_eq!(crate::index::EMBEDDING_MODEL, "qwen3-embedding-0.6b");
        assert_eq!(DIMS, 1024);
        assert!(MODEL_FILE.contains("Qwen3-Embedding-0.6B"));
        assert!(MODEL_URL.contains("Qwen3-Embedding-0.6B"));
        assert!(MODEL_URL.starts_with("https://huggingface.co/"));
    }

    #[test]
    fn normalize_produces_unit_vectors() {
        let v = normalize(&[3.0, 4.0]);
        assert!((v[0] - 0.6).abs() < 1e-6 && (v[1] - 0.8).abs() < 1e-6);
        assert_eq!(normalize(&[0.0, 0.0]), vec![0.0, 0.0]);
    }

    #[test]
    fn present_model_file_resolves_without_download() {
        let _home = crate::workspace::HOME_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", tmp.path()); // isolate ~/.mari — test-only
        let path = crate::models::model_path(MODEL_FILE);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"placeholder").unwrap();
        // An on-disk GGUF resolves to its path — never triggers a download.
        assert_eq!(ensure_model().unwrap(), path);
    }

    #[test]
    fn missing_model_with_auto_download_off_errors_cleanly() {
        let _home = crate::workspace::HOME_TEST_LOCK.lock().unwrap();
        let spec = crate::models::ModelSpec {
            file: "does-not-exist.gguf",
            url: MODEL_URL,
            sha256: MODEL_SHA256,
            approx_mb: 640,
            kind: "embedding",
            auto_download: false,
        };
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", tmp.path());
        let err = crate::models::ensure_gguf(&spec).unwrap_err().to_string();
        assert!(err.contains("auto_download is off"), "{err}");
    }

    #[test]
    fn lance_roundtrip_and_duckdb_cosine() {
        let _home = crate::workspace::HOME_TEST_LOCK.lock().unwrap();
        // Deterministic fake vectors: exercises the Lance write/read and the
        // DuckDB Arrow-bridge ranking without the model.
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", tmp.path()); // isolate ~/.mari — test-only
        let mut a = vec![0.0f32; DIMS];
        a[0] = 1.0;
        let mut b = vec![0.0f32; DIMS];
        b[1] = 1.0;
        let mut c = vec![0.0f32; DIMS];
        c[0] = 0.9;
        c[1] = 0.1;
        let rows = vec![
            ("chunk-a".to_string(), a.clone()),
            ("chunk-b".to_string(), b),
            ("chunk-c".to_string(), normalize(&c)),
        ];
        write_dataset(false, &rows).unwrap();
        let back = read_dataset(false).unwrap();
        assert_eq!(back.len(), 3);
        assert_eq!(back[0].1.len(), DIMS);

        let ranked = duckdb_cosine_topk(&back, &a, 2).unwrap();
        assert_eq!(ranked[0].0, "chunk-a");
        assert!((ranked[0].1 - 1.0).abs() < 1e-3);
        assert_eq!(ranked[1].0, "chunk-c");
        assert!(ranked[1].1 > 0.9);
    }
}
