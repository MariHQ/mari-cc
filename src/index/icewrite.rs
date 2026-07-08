//! Manual Apache Iceberg writer (SPEC §8.8).
//!
//! Mari writes Iceberg tables **directly** — DuckDB is never in the write path.
//! A commit emits Parquet data (and, later, equality-delete) files from Arrow
//! batches with Iceberg field-ids stamped into the Parquet schema, then writes
//! the Iceberg metadata itself: an Avro manifest, an Avro manifest list, a new
//! `metadata.json`, and an atomic `version-hint.text` swap. DuckDB reads the
//! result with `iceberg_scan` (§8.8 read path).
//!
//! This module is the low-level format layer. It is deliberately DuckDB-free so
//! the same code path serves a local dir or (later) an `s3://` object store.

use super::icestore::Store;
use anyhow::{Context, Result};
use apache_avro::types::Value as Av;
use apache_avro::{Schema as AvSchema, Writer as AvWriter};
use arrow_array::RecordBatch;
use arrow_schema::{DataType, Field, Schema as ArrowSchema};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// One column of the logical Iceberg schema: a stable field-id, a name, an
/// Iceberg primitive type name, and whether it is required.
#[derive(Clone, Debug)]
pub struct IceField {
    pub id: i32,
    pub name: &'static str,
    /// Iceberg primitive type: "long" | "string" | "int" | "double" | "boolean".
    pub ty: &'static str,
    pub required: bool,
}

impl IceField {
    fn arrow_type(&self) -> DataType {
        match self.ty {
            "long" => DataType::Int64,
            "int" => DataType::Int32,
            "double" => DataType::Float64,
            "boolean" => DataType::Boolean,
            _ => DataType::Utf8,
        }
    }

    /// Iceberg type token as it appears in metadata.json `fields[].type`.
    fn ice_type(&self) -> &'static str {
        self.ty
    }
}

/// A catalog table's Iceberg shape: its name, ordered fields (with stable
/// field-ids), and the logical merge key (§8.7) used for equality deletes.
pub struct TableDef {
    pub name: &'static str,
    pub fields: &'static [IceField],
    pub key: &'static [&'static str],
}

impl TableDef {
    /// The `IceField`s making up this table's equality-delete key.
    pub fn key_fields(&self) -> Vec<IceField> {
        self.key
            .iter()
            .map(|k| self.fields.iter().find(|f| f.name == *k).unwrap().clone())
            .collect()
    }
}

macro_rules! f {
    ($id:expr, $name:literal, $ty:literal) => {
        IceField { id: $id, name: $name, ty: $ty, required: false }
    };
}

// Field-ids are stable per table (1..N in column order). All columns are marked
// optional for Iceberg (null-safety); NOT NULL stays enforced in the mutable
// staging layer. Types: TEXT→string, INTEGER→long, REAL→double (§8.7).
const SCHEMA_META: &[IceField] = &[f!(1, "key", "string"), f!(2, "value", "string")];
const SOURCES: &[IceField] = &[
    f!(1, "source_id", "string"), f!(2, "provider", "string"), f!(3, "scope", "string"),
    f!(4, "connector_version", "string"), f!(5, "auth_provider", "string"),
    f!(6, "list_keys_json", "string"), f!(7, "config_hash", "string"),
    f!(8, "last_sync_at", "string"), f!(9, "last_success_at", "string"), f!(10, "last_error", "string"),
];
const DOCUMENTS: &[IceField] = &[
    f!(1, "doc_id", "string"), f!(2, "source_id", "string"), f!(3, "external_id", "string"),
    f!(4, "canonical_ref", "string"), f!(5, "title", "string"), f!(6, "url", "string"),
    f!(7, "path", "string"), f!(8, "mime_type", "string"), f!(9, "kind", "string"),
    f!(10, "author_id", "string"), f!(11, "author_name", "string"), f!(12, "created_at", "string"),
    f!(13, "updated_at", "string"), f!(14, "observed_at", "string"), f!(15, "version", "string"),
    f!(16, "content_sha256", "string"), f!(17, "body", "string"), f!(18, "metadata_json", "string"),
];
const CHUNKS: &[IceField] = &[
    f!(1, "chunk_id", "string"), f!(2, "doc_id", "string"), f!(3, "chunk_index", "long"),
    f!(4, "heading_path", "string"), f!(5, "section_anchor", "string"), f!(6, "start_byte", "long"),
    f!(7, "end_byte", "long"), f!(8, "start_line", "long"), f!(9, "end_line", "long"),
    f!(10, "token_count", "long"), f!(11, "text", "string"), f!(12, "text_sha256", "string"),
    f!(13, "metadata_json", "string"),
];
const EMBEDDINGS: &[IceField] = &[
    f!(1, "chunk_id", "string"), f!(2, "model_id", "string"), f!(3, "dims", "long"),
    f!(4, "vector_json", "string"), f!(5, "norm", "double"), f!(6, "embedded_at", "string"),
];
const SPANS: &[IceField] = &[
    f!(1, "span_id", "string"), f!(2, "doc_id", "string"), f!(3, "chunk_id", "string"),
    f!(4, "span_kind", "string"), f!(5, "label", "string"), f!(6, "start_byte", "long"),
    f!(7, "end_byte", "long"), f!(8, "start_line", "long"), f!(9, "end_line", "long"),
    f!(10, "stable_hash", "string"), f!(11, "metadata_json", "string"),
];
const SYMBOLS: &[IceField] = &[
    f!(1, "symbol_id", "string"), f!(2, "doc_id", "string"), f!(3, "span_id", "string"),
    f!(4, "language", "string"), f!(5, "symbol_kind", "string"), f!(6, "name", "string"),
    f!(7, "qualified_name", "string"), f!(8, "signature", "string"), f!(9, "start_byte", "long"),
    f!(10, "end_byte", "long"), f!(11, "start_line", "long"), f!(12, "end_line", "long"),
    f!(13, "metadata_json", "string"),
];
const EDGES: &[IceField] = &[
    f!(1, "edge_id", "string"), f!(2, "from_type", "string"), f!(3, "from_id", "string"),
    f!(4, "to_type", "string"), f!(5, "to_id", "string"), f!(6, "rel", "string"),
    f!(7, "confidence", "double"), f!(8, "evidence_span_id", "string"), f!(9, "created_by", "string"),
    f!(10, "created_at", "string"), f!(11, "metadata_json", "string"),
];
const LINEAGE_EDGES: &[IceField] = &[
    f!(1, "lineage_id", "string"), f!(2, "from_span_id", "string"), f!(3, "to_span_id", "string"),
    f!(4, "rel", "string"), f!(5, "status", "string"), f!(6, "confidence", "double"),
    f!(7, "confirmed_by", "string"), f!(8, "confirmed_at", "string"), f!(9, "last_checked_at", "string"),
    f!(10, "metadata_json", "string"),
];
const FACTS: &[IceField] = &[
    f!(1, "fact_id", "string"), f!(2, "claim", "string"), f!(3, "source_ref", "string"),
    f!(4, "source_span_id", "string"), f!(5, "status", "string"), f!(6, "created_by", "string"),
    f!(7, "created_at", "string"), f!(8, "metadata_json", "string"),
];
const TAGS: &[IceField] = &[
    f!(1, "target_type", "string"), f!(2, "target_id", "string"), f!(3, "status", "string"),
    f!(4, "note", "string"), f!(5, "by", "string"), f!(6, "at", "string"), f!(7, "metadata_json", "string"),
];
const SYNC_EVENTS: &[IceField] = &[
    f!(1, "event_id", "string"), f!(2, "source_id", "string"), f!(3, "started_at", "string"),
    f!(4, "finished_at", "string"), f!(5, "status", "string"), f!(6, "docs_seen", "long"),
    f!(7, "docs_changed", "long"), f!(8, "docs_deleted", "long"), f!(9, "error", "string"),
    f!(10, "metadata_json", "string"),
];

/// All 12 published catalog tables (§8.8), each with its Iceberg fields + merge
/// key. `tags` has a composite key; all others a single id column.
pub const CATALOG: &[TableDef] = &[
    TableDef { name: "schema_meta", fields: SCHEMA_META, key: &["key"] },
    TableDef { name: "sources", fields: SOURCES, key: &["source_id"] },
    TableDef { name: "documents", fields: DOCUMENTS, key: &["doc_id"] },
    TableDef { name: "chunks", fields: CHUNKS, key: &["chunk_id"] },
    TableDef { name: "embeddings", fields: EMBEDDINGS, key: &["chunk_id"] },
    TableDef { name: "spans", fields: SPANS, key: &["span_id"] },
    TableDef { name: "symbols", fields: SYMBOLS, key: &["symbol_id"] },
    TableDef { name: "edges", fields: EDGES, key: &["edge_id"] },
    TableDef { name: "lineage_edges", fields: LINEAGE_EDGES, key: &["lineage_id"] },
    TableDef { name: "facts", fields: FACTS, key: &["fact_id"] },
    TableDef { name: "tags", fields: TAGS, key: &["target_type", "target_id"] },
    TableDef { name: "sync_events", fields: SYNC_EVENTS, key: &["event_id"] },
];

#[allow(dead_code)] // public lookup used by tests and the future compaction path
pub fn table_def(name: &str) -> Option<&'static TableDef> {
    CATALOG.iter().find(|t| t.name == name)
}

/// Build an Arrow schema that stamps each Iceberg field-id into the Parquet
/// column metadata (`PARQUET:field_id`), which is what makes the emitted Parquet
/// a valid Iceberg data file rather than a plain Parquet file.
pub fn arrow_schema(fields: &[IceField]) -> Arc<ArrowSchema> {
    let cols: Vec<Field> = fields
        .iter()
        .map(|f| {
            let mut md = HashMap::new();
            md.insert("PARQUET:field_id".to_string(), f.id.to_string());
            Field::new(f.name, f.arrow_type(), !f.required).with_metadata(md)
        })
        .collect();
    Arc::new(ArrowSchema::new(cols))
}

/// Encode a Parquet data file in memory and return `(bytes, record_count)`. The
/// caller writes the bytes to the store (local or s3), so the writer is storage
/// agnostic.
fn parquet_bytes(batch: &RecordBatch) -> Result<(Vec<u8>, i64)> {
    use parquet::arrow::ArrowWriter;
    use parquet::basic::{Compression, ZstdLevel};
    use parquet::file::properties::WriterProperties;

    let props = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::default()))
        .build();
    let mut buf: Vec<u8> = Vec::new();
    let mut writer = ArrowWriter::try_new(&mut buf, batch.schema(), Some(props))?;
    writer.write(batch)?;
    writer.close()?;
    Ok((buf, batch.num_rows() as i64))
}

/// The Avro schema of an Iceberg **manifest file** (list of data/delete files in
/// one snapshot's contribution). Minimal but valid: the fields DuckDB's reader
/// consumes, plus `equality_ids` so the same schema serves data and
/// equality-delete manifests. Unpartitioned (`partition` is an empty struct).
fn manifest_entry_schema() -> AvSchema {
    AvSchema::parse_str(
        r#"{
      "type": "record", "name": "manifest_entry",
      "fields": [
        {"name": "status", "type": "int", "field-id": 0},
        {"name": "snapshot_id", "type": ["null","long"], "default": null, "field-id": 1},
        {"name": "sequence_number", "type": ["null","long"], "default": null, "field-id": 3},
        {"name": "file_sequence_number", "type": ["null","long"], "default": null, "field-id": 4},
        {"name": "data_file", "type": {
          "type": "record", "name": "r2",
          "fields": [
            {"name": "content", "type": "int", "field-id": 134},
            {"name": "file_path", "type": "string", "field-id": 100},
            {"name": "file_format", "type": "string", "field-id": 101},
            {"name": "partition", "type": {"type":"record","name":"r102","fields":[]}, "field-id": 102},
            {"name": "record_count", "type": "long", "field-id": 103},
            {"name": "file_size_in_bytes", "type": "long", "field-id": 104},
            {"name": "equality_ids", "type": ["null",{"type":"array","items":"int","element-id":136}], "default": null, "field-id": 135}
          ]
        }, "field-id": 2}
      ]
    }"#,
    )
    .expect("static manifest_entry avro schema")
}

/// 0 = data manifest/file, 1 = delete manifest, 2 = equality-delete file content.
#[derive(Clone, Copy, PartialEq)]
enum FileContent {
    Data = 0,
    EqualityDeletes = 2,
}

/// The Avro schema of an Iceberg **manifest list** (the snapshot's list of
/// manifest files).
fn manifest_list_schema() -> AvSchema {
    AvSchema::parse_str(
        r#"{
      "type": "record", "name": "manifest_file",
      "fields": [
        {"name": "manifest_path", "type": "string", "field-id": 500},
        {"name": "manifest_length", "type": "long", "field-id": 501},
        {"name": "partition_spec_id", "type": "int", "field-id": 502},
        {"name": "content", "type": "int", "field-id": 517},
        {"name": "sequence_number", "type": "long", "field-id": 515},
        {"name": "min_sequence_number", "type": "long", "field-id": 516},
        {"name": "added_snapshot_id", "type": "long", "field-id": 503},
        {"name": "added_files_count", "type": "int", "field-id": 504},
        {"name": "existing_files_count", "type": "int", "field-id": 505},
        {"name": "deleted_files_count", "type": "int", "field-id": 506},
        {"name": "added_rows_count", "type": "long", "field-id": 512},
        {"name": "existing_rows_count", "type": "long", "field-id": 513},
        {"name": "deleted_rows_count", "type": "long", "field-id": 514}
      ]
    }"#,
    )
    .expect("static manifest_file avro schema")
}

fn nullable_long(v: Option<i64>) -> Av {
    match v {
        Some(x) => Av::Union(1, Box::new(Av::Long(x))),
        None => Av::Union(0, Box::new(Av::Null)),
    }
}

/// One file to add in this snapshot's manifest (data or equality-delete).
struct AddFile {
    content: FileContent,
    path: String,
    record_count: i64,
    file_size: i64,
    /// For equality-delete files: the field-ids the delete matches on.
    equality_ids: Option<Vec<i32>>,
}

/// Encode a manifest file whose entries are all ADDED files of one `content`
/// kind (data OR delete — Iceberg keeps them in separate manifests). Returns
/// `(bytes, total_record_count)`; the caller writes it to the store.
fn manifest_bytes(snapshot_id: i64, seq: i64, files: &[AddFile]) -> Result<(Vec<u8>, i64)> {
    let schema = manifest_entry_schema();
    let mut w = AvWriter::new(&schema, Vec::new());
    let mut total_rows = 0i64;
    for f in files {
        total_rows += f.record_count;
        let equality_ids = match &f.equality_ids {
            Some(ids) => Av::Union(
                1,
                Box::new(Av::Array(ids.iter().map(|i| Av::Int(*i)).collect())),
            ),
            None => Av::Union(0, Box::new(Av::Null)),
        };
        let data_file = Av::Record(vec![
            ("content".into(), Av::Int(f.content as i32)),
            ("file_path".into(), Av::String(f.path.clone())),
            ("file_format".into(), Av::String("PARQUET".into())),
            ("partition".into(), Av::Record(vec![])),
            ("record_count".into(), Av::Long(f.record_count)),
            ("file_size_in_bytes".into(), Av::Long(f.file_size)),
            ("equality_ids".into(), equality_ids),
        ]);
        let entry = Av::Record(vec![
            ("status".into(), Av::Int(1)), // 1 = ADDED
            ("snapshot_id".into(), nullable_long(Some(snapshot_id))),
            ("sequence_number".into(), nullable_long(Some(seq))),
            ("file_sequence_number".into(), nullable_long(Some(seq))),
            ("data_file".into(), data_file),
        ]);
        w.append(entry)?;
    }
    let bytes = w.into_inner()?;
    Ok((bytes, total_rows))
}

/// One entry in the manifest list (points at a manifest file).
struct ManifestRec {
    path: String,
    len: u64,
    content: i32, // 0 = data manifest, 1 = delete manifest
    seq: i64,
    snapshot_id: i64,
    added_files: i32,
    added_rows: i64,
}

fn manifest_list_value(r: &ManifestRec) -> Av {
    Av::Record(vec![
        ("manifest_path".into(), Av::String(r.path.clone())),
        ("manifest_length".into(), Av::Long(r.len as i64)),
        ("partition_spec_id".into(), Av::Int(0)),
        ("content".into(), Av::Int(r.content)),
        ("sequence_number".into(), Av::Long(r.seq)),
        ("min_sequence_number".into(), Av::Long(r.seq)),
        ("added_snapshot_id".into(), Av::Long(r.snapshot_id)),
        ("added_files_count".into(), Av::Int(r.added_files)),
        ("existing_files_count".into(), Av::Int(0)),
        ("deleted_files_count".into(), Av::Int(0)),
        ("added_rows_count".into(), Av::Long(r.added_rows)),
        ("existing_rows_count".into(), Av::Long(0)),
        ("deleted_rows_count".into(), Av::Long(0)),
    ])
}

/// Encode the manifest list = carried-forward prior manifest records (raw Avro
/// values re-appended verbatim) followed by this snapshot's new manifests.
fn manifest_list_bytes(carried: &[Av], new: &[ManifestRec]) -> Result<Vec<u8>> {
    let schema = manifest_list_schema();
    let mut w = AvWriter::new(&schema, Vec::new());
    for v in carried {
        w.append(v.clone())?;
    }
    for r in new {
        w.append(manifest_list_value(r))?;
    }
    Ok(w.into_inner()?)
}

/// Read a manifest list's records back as raw Avro values, to carry the live
/// manifests of the prior snapshot forward into the next one (they are
/// immutable). Returns empty if the file is missing.
fn read_manifest_list(store: &Store, uri: &str) -> Result<Vec<Av>> {
    let Some(bytes) = store.get(uri)? else {
        return Ok(Vec::new());
    };
    let reader = apache_avro::Reader::new(std::io::Cursor::new(bytes))
        .with_context(|| format!("reading manifest list {uri}"))?;
    let mut out = Vec::new();
    for rec in reader {
        out.push(rec?);
    }
    Ok(out)
}

/// Render the Iceberg schema (fields) as metadata.json `schema` struct.
fn schema_json(fields: &[IceField]) -> serde_json::Value {
    let cols: Vec<serde_json::Value> = fields
        .iter()
        .map(|f| {
            serde_json::json!({
                "id": f.id,
                "name": f.name,
                "required": f.required,
                "type": f.ice_type(),
            })
        })
        .collect();
    serde_json::json!({ "type": "struct", "schema-id": 0, "fields": cols })
}

/// Deterministic-ish ids without Date/rand (unavailable in some contexts): the
/// caller passes a monotonically varying seed (e.g. version number + table).
fn snapshot_id_from(seed: u64) -> i64 {
    // Keep it positive and non-zero.
    ((seed.wrapping_mul(0x9E3779B97F4A7C15) >> 1) | 1) as i64
}

/// Read the table's current metadata version + JSON, or `(0, None)` if the table
/// does not exist yet. `meta_dir` is a URI (local path or `s3://…/table/metadata`).
fn load_current(store: &Store, meta_dir: &str) -> Result<(u64, Option<serde_json::Value>)> {
    let Some(txt) = store.get(&format!("{meta_dir}/version-hint.text"))? else {
        return Ok((0, None));
    };
    let version: u64 = String::from_utf8_lossy(&txt).trim().parse().unwrap_or(0);
    if version == 0 {
        return Ok((0, None));
    }
    let meta_uri = format!("{meta_dir}/v{version}.metadata.json");
    let bytes = store
        .get(&meta_uri)?
        .ok_or_else(|| anyhow::anyhow!("missing {meta_uri}"))?;
    let meta: serde_json::Value = serde_json::from_slice(&bytes)?;
    Ok((version, Some(meta)))
}

/// The manifest-list path of a metadata's current snapshot.
fn current_manifest_list(meta: &serde_json::Value) -> Option<String> {
    let cur = meta["current-snapshot-id"].as_i64()?;
    meta["snapshots"]
        .as_array()?
        .iter()
        .find(|s| s["snapshot-id"].as_i64() == Some(cur))
        .and_then(|s| s["manifest-list"].as_str())
        .map(str::to_string)
}

/// A single append commit: add `added` data rows and/or an `delete` equality
/// delete (key field + the key values to remove), producing a new snapshot that
/// carries every prior live manifest forward (§8.8). DuckDB never writes here.
/// This is the one write primitive; `create_table` is the fresh-table case.
pub fn commit(
    store: &Store,
    table_uri: &str,
    fields: &[IceField],
    added: Option<&RecordBatch>,
    delete: Option<(&[IceField], &RecordBatch)>,
    now_ms: i64,
) -> Result<()> {
    let table_uri = table_uri.trim_end_matches('/');
    let data_dir = format!("{table_uri}/data");
    let meta_dir = format!("{table_uri}/metadata");

    let (prev_version, prev_meta) = load_current(store, &meta_dir)?;
    let version = prev_version + 1;
    let seq = version as i64;
    let snapshot_id =
        snapshot_id_from((now_ms as u64) ^ (version.wrapping_mul(0x100_0000_01b3)));

    let mut new_manifests: Vec<ManifestRec> = Vec::new();
    let mut mno = 0;

    // Data file (if any rows were added this commit).
    if let Some(batch) = added {
        let data_uri = format!("{data_dir}/data-{seq}-{snapshot_id:x}.parquet");
        let (bytes, rows) = parquet_bytes(batch)?;
        let size = bytes.len() as i64;
        store.put(&data_uri, bytes)?;
        let man_uri = format!("{meta_dir}/{snapshot_id:x}-m{mno}.avro");
        let files = [AddFile {
            content: FileContent::Data,
            path: data_uri,
            record_count: rows,
            file_size: size,
            equality_ids: None,
        }];
        let (man, total) = manifest_bytes(snapshot_id, seq, &files)?;
        let len = man.len() as u64;
        store.put(&man_uri, man)?;
        new_manifests.push(ManifestRec {
            path: man_uri,
            len,
            content: 0,
            seq,
            snapshot_id,
            added_files: 1,
            added_rows: total,
        });
        mno += 1;
    }

    // Equality-delete file (if this commit removes/updates rows).
    if let Some((keys, dbatch)) = delete {
        let del_uri = format!("{data_dir}/delete-{seq}-{snapshot_id:x}.parquet");
        let (bytes, rows) = parquet_bytes(dbatch)?;
        let size = bytes.len() as i64;
        store.put(&del_uri, bytes)?;
        let man_uri = format!("{meta_dir}/{snapshot_id:x}-m{mno}.avro");
        let files = [AddFile {
            content: FileContent::EqualityDeletes,
            path: del_uri,
            record_count: rows,
            file_size: size,
            equality_ids: Some(keys.iter().map(|k| k.id).collect()),
        }];
        let (man, total) = manifest_bytes(snapshot_id, seq, &files)?;
        let len = man.len() as u64;
        store.put(&man_uri, man)?;
        new_manifests.push(ManifestRec {
            path: man_uri,
            len,
            content: 1, // delete manifest
            seq,
            snapshot_id,
            added_files: 1,
            added_rows: total,
        });
    }

    // Carry forward the prior snapshot's manifests (immutable).
    let carried = match prev_meta.as_ref().and_then(current_manifest_list) {
        Some(prev_list) => read_manifest_list(store, &prev_list)?,
        None => Vec::new(),
    };

    let list_uri = format!("{meta_dir}/snap-{snapshot_id}-{seq}.avro");
    let list_bytes = manifest_list_bytes(&carried, &new_manifests)?;
    store.put(&list_uri, list_bytes)?;

    // Assemble metadata.json vN, carrying prior snapshots forward.
    let table_uuid = prev_meta
        .as_ref()
        .and_then(|m| m["table-uuid"].as_str().map(str::to_string))
        .unwrap_or_else(|| uuid_like(snapshot_id));
    let last_col_id = fields.iter().map(|f| f.id).max().unwrap_or(1);
    let parent = prev_meta.as_ref().and_then(|m| m["current-snapshot-id"].as_i64());
    let mut snapshots = prev_meta
        .as_ref()
        .and_then(|m| m["snapshots"].as_array().cloned())
        .unwrap_or_default();
    let mut summary = serde_json::Map::new();
    summary.insert(
        "operation".into(),
        serde_json::Value::from(if delete.is_some() && added.is_none() {
            "delete"
        } else {
            "append"
        }),
    );
    let mut snap = serde_json::Map::new();
    snap.insert("snapshot-id".into(), snapshot_id.into());
    if let Some(p) = parent {
        snap.insert("parent-snapshot-id".into(), p.into());
    }
    snap.insert("sequence-number".into(), seq.into());
    snap.insert("timestamp-ms".into(), now_ms.into());
    snap.insert("manifest-list".into(), list_uri.into());
    snap.insert("schema-id".into(), 0.into());
    snap.insert("summary".into(), serde_json::Value::Object(summary));
    snapshots.push(serde_json::Value::Object(snap));

    let mut snapshot_log = prev_meta
        .as_ref()
        .and_then(|m| m["snapshot-log"].as_array().cloned())
        .unwrap_or_default();
    snapshot_log.push(serde_json::json!({ "timestamp-ms": now_ms, "snapshot-id": snapshot_id }));

    let metadata = serde_json::json!({
        "format-version": 2,
        "table-uuid": table_uuid,
        "location": table_uri,
        "last-sequence-number": seq,
        "last-updated-ms": now_ms,
        "last-column-id": last_col_id,
        "current-schema-id": 0,
        "schemas": [ schema_json(fields) ],
        "default-spec-id": 0,
        "partition-specs": [ { "spec-id": 0, "fields": [] } ],
        "last-partition-id": 999,
        "default-sort-order-id": 0,
        "sort-orders": [ { "order-id": 0, "fields": [] } ],
        "properties": {},
        "current-snapshot-id": snapshot_id,
        "refs": { "main": { "snapshot-id": snapshot_id, "type": "branch" } },
        "snapshots": snapshots,
        "snapshot-log": snapshot_log,
        "metadata-log": []
    });
    store.put(
        &format!("{meta_dir}/v{version}.metadata.json"),
        serde_json::to_vec_pretty(&metadata)?,
    )?;

    // Pointer swap. On a local fs a single write of a tiny file is effectively
    // atomic for readers; on s3 a put is atomic per key. Readers resolve the new
    // snapshot only once the hint names it.
    store.put(
        &format!("{meta_dir}/version-hint.text"),
        version.to_string().into_bytes(),
    )?;
    Ok(())
}

/// Compaction rewrite (§8.8): replace the table with a **single fresh snapshot**
/// holding exactly `batch` (the current live rows, deletes already applied),
/// carrying **no** prior manifests and **no** delete files. Returns the URIs of
/// the files this wrote — the "keep set". The caller deletes every other file
/// under the table (orphan removal). `version-hint` is bumped so readers move to
/// the clean snapshot atomically; the old snapshot's files are only removed after
/// the swap.
pub fn rewrite_table(
    store: &Store,
    table_uri: &str,
    fields: &[IceField],
    batch: &RecordBatch,
    now_ms: i64,
) -> Result<Vec<String>> {
    let table_uri = table_uri.trim_end_matches('/');
    let data_dir = format!("{table_uri}/data");
    let meta_dir = format!("{table_uri}/metadata");

    let (prev_version, prev_meta) = load_current(store, &meta_dir)?;
    let version = prev_version + 1;
    let seq = version as i64;
    let snapshot_id =
        snapshot_id_from((now_ms as u64) ^ (version.wrapping_mul(0x51_7c_c1_b7_27_22_0a_95)));

    // Single fresh data file with all live rows.
    let data_uri = format!("{data_dir}/data-compact-{version}-{snapshot_id:x}.parquet");
    let (bytes, rows) = parquet_bytes(batch)?;
    let size = bytes.len() as i64;
    store.put(&data_uri, bytes)?;

    // Manifest referencing only that data file.
    let man_uri = format!("{meta_dir}/{snapshot_id:x}-compact-m0.avro");
    let files = [AddFile {
        content: FileContent::Data,
        path: data_uri.clone(),
        record_count: rows,
        file_size: size,
        equality_ids: None,
    }];
    let (man, total) = manifest_bytes(snapshot_id, seq, &files)?;
    let man_len = man.len() as u64;
    store.put(&man_uri, man)?;

    // Manifest list referencing only that manifest — no carry-forward.
    let list_uri = format!("{meta_dir}/snap-{snapshot_id}-{seq}.avro");
    let list_bytes = manifest_list_bytes(
        &[],
        &[ManifestRec {
            path: man_uri.clone(),
            len: man_len,
            content: 0,
            seq,
            snapshot_id,
            added_files: 1,
            added_rows: total,
        }],
    )?;
    store.put(&list_uri, list_bytes)?;

    // Metadata with only the new snapshot (history dropped → old files orphaned).
    let table_uuid = prev_meta
        .as_ref()
        .and_then(|m| m["table-uuid"].as_str().map(str::to_string))
        .unwrap_or_else(|| uuid_like(snapshot_id));
    let last_col_id = fields.iter().map(|f| f.id).max().unwrap_or(1);
    let metadata = serde_json::json!({
        "format-version": 2,
        "table-uuid": table_uuid,
        "location": table_uri,
        "last-sequence-number": seq,
        "last-updated-ms": now_ms,
        "last-column-id": last_col_id,
        "current-schema-id": 0,
        "schemas": [ schema_json(fields) ],
        "default-spec-id": 0,
        "partition-specs": [ { "spec-id": 0, "fields": [] } ],
        "last-partition-id": 999,
        "default-sort-order-id": 0,
        "sort-orders": [ { "order-id": 0, "fields": [] } ],
        "properties": { "mari.compacted": "true" },
        "current-snapshot-id": snapshot_id,
        "refs": { "main": { "snapshot-id": snapshot_id, "type": "branch" } },
        "snapshots": [ {
            "snapshot-id": snapshot_id,
            "sequence-number": seq,
            "timestamp-ms": now_ms,
            "manifest-list": list_uri,
            "schema-id": 0,
            "summary": { "operation": "replace" }
        } ],
        "snapshot-log": [ { "timestamp-ms": now_ms, "snapshot-id": snapshot_id } ],
        "metadata-log": []
    });
    let meta_uri = format!("{meta_dir}/v{version}.metadata.json");
    store.put(&meta_uri, serde_json::to_vec_pretty(&metadata)?)?;

    let hint_uri = format!("{meta_dir}/version-hint.text");
    store.put(&hint_uri, version.to_string().into_bytes())?;

    Ok(vec![data_uri, man_uri, list_uri, meta_uri, hint_uri])
}

/// Fresh-table convenience: the first snapshot holding exactly `batch` at a local
/// path. Used by the writer round-trip tests.
#[allow(dead_code)]
pub fn create_table(
    table_dir: &Path,
    fields: &[IceField],
    batch: &RecordBatch,
    now_ms: i64,
) -> Result<()> {
    commit(
        &Store::Local,
        &table_dir.to_string_lossy(),
        fields,
        Some(batch),
        None,
        now_ms,
    )
}

/// A stable 36-char UUID-shaped string derived from a seed (no rand dependency).
fn uuid_like(seed: i64) -> String {
    let s = seed as u64;
    let a = s.wrapping_mul(0x9E3779B97F4A7C15);
    let b = (s ^ 0xDEADBEEF).wrapping_mul(0xC2B2AE3D27D4EB4F);
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (a >> 32) as u32,
        (a >> 16) as u16,
        (a as u16) | 0x4000,
        (b >> 48) as u16,
        b & 0xFFFF_FFFF_FFFF
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{Int64Array, StringArray};
    use duckdb::Connection;

    #[test]
    fn duckdb_reads_hand_written_iceberg_table() {
        let dir = tempfile::tempdir().unwrap();
        let table = dir.path().join("documents");
        let fields = vec![
            IceField { id: 1, name: "id", ty: "long", required: true },
            IceField { id: 2, name: "name", ty: "string", required: false },
        ];
        let schema = arrow_schema(&fields);
        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(Int64Array::from(vec![1, 2, 3])),
                Arc::new(StringArray::from(vec!["a", "b", "c"])),
            ],
        )
        .unwrap();
        create_table(&table, &fields, &batch, 1_700_000_000_000).unwrap();

        // DuckDB reads it — this is the make-or-break for the manual writer.
        let conn = Connection::open_in_memory().unwrap();
        crate::index::iceberg::install_iceberg(&conn).unwrap();
        let uri = table.to_string_lossy();
        let count: i64 = conn
            .query_row(
                &format!("SELECT count(*) FROM iceberg_scan('{uri}')"),
                [],
                |r| r.get(0),
            )
            .map_err(|e| format!("iceberg_scan failed: {e}"))
            .unwrap();
        assert_eq!(count, 3, "expected 3 rows from hand-written iceberg table");

        let names: i64 = conn
            .query_row(
                &format!("SELECT count(*) FROM iceberg_scan('{uri}') WHERE name = 'b'"),
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(names, 1, "value round-trip: exactly one row with name='b'");
    }

    fn scan_count(conn: &Connection, uri: &str) -> i64 {
        conn.query_row(
            &format!("SELECT count(*) FROM iceberg_scan('{uri}')"),
            [],
            |r| r.get(0),
        )
        .map_err(|e| format!("iceberg_scan('{uri}') failed: {e}"))
        .unwrap()
    }

    #[test]
    fn append_snapshot_then_equality_delete() {
        let dir = tempfile::tempdir().unwrap();
        let table = dir.path().join("documents");
        let fields = vec![
            IceField { id: 1, name: "id", ty: "long", required: true },
            IceField { id: 2, name: "name", ty: "string", required: false },
        ];
        let schema = arrow_schema(&fields);
        let mk = |ids: Vec<i64>, names: Vec<&str>| {
            RecordBatch::try_new(
                schema.clone(),
                vec![
                    Arc::new(Int64Array::from(ids)),
                    Arc::new(StringArray::from(names)),
                ],
            )
            .unwrap()
        };

        // Snapshot 1: rows 1,2,3.
        create_table(&table, &fields, &mk(vec![1, 2, 3], vec!["a", "b", "c"]), 1_700_000_000_000)
            .unwrap();
        // Snapshot 2: append row 4.
        commit(
            &Store::Local,
            &table.to_string_lossy(),
            &fields,
            Some(&mk(vec![4], vec!["d"])),
            None,
            1_700_000_001_000,
        )
        .unwrap();

        let conn = Connection::open_in_memory().unwrap();
        crate::index::iceberg::install_iceberg(&conn).unwrap();
        let uri = table.to_string_lossy().to_string();
        assert_eq!(scan_count(&conn, &uri), 4, "append snapshot: 3 + 1 = 4 rows");

        // Snapshot 3: equality-delete id = 2. Delete file carries just the key
        // column (id) with its Iceberg field-id.
        let key = IceField { id: 1, name: "id", ty: "long", required: true };
        let key_schema = arrow_schema(std::slice::from_ref(&key));
        let del = RecordBatch::try_new(key_schema, vec![Arc::new(Int64Array::from(vec![2i64]))])
            .unwrap();
        let keys = [key.clone()];
        commit(
            &Store::Local,
            &table.to_string_lossy(),
            &fields,
            None,
            Some((&keys[..], &del)),
            1_700_000_002_000,
        )
        .unwrap();

        // Fresh connection to prove the delete is persisted in metadata, not
        // session state. THIS is the equality-delete read-compat verdict.
        let c2 = Connection::open_in_memory().unwrap();
        crate::index::iceberg::install_iceberg(&c2).unwrap();
        let after = scan_count(&c2, &uri);
        eprintln!("VERDICT rows after equality-delete of id=2 (want 3): {after}");
        let has2: i64 = c2
            .query_row(
                &format!("SELECT count(*) FROM iceberg_scan('{uri}') WHERE id = 2"),
                [],
                |r| r.get(0),
            )
            .unwrap();
        eprintln!("VERDICT rows still matching id=2 (want 0): {has2}");
        assert_eq!(after, 3, "equality delete should remove exactly row id=2");
        assert_eq!(has2, 0, "row id=2 must be gone after equality delete");
    }
}
