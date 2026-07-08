//! DuckDB catalog, SQL surface, search, and sync (SPEC §7/§8).
pub mod iceberg;
pub mod icepub;
pub mod icestore;
pub mod icewrite;
pub mod search;
pub mod sync;
pub mod vector;

/// Base catalog tables published to Iceberg (§8.8). Order matters only for
/// readability; publish/read treat them independently. `embeddings` is included
/// for completeness though vectors live in Lance.
pub const CATALOG_TABLES: [&str; 12] = [
    "schema_meta",
    "sources",
    "documents",
    "chunks",
    "embeddings",
    "spans",
    "symbols",
    "edges",
    "lineage_edges",
    "facts",
    "tags",
    "sync_events",
];

use crate::{cloud, config, workspace};
use anyhow::Result;
use duckdb::Connection;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const CATALOG_FILE: &str = "catalog.duckdb";
pub const SCHEMA_VERSION: &str = "1";
pub const EMBEDDING_MODEL: &str = "qwen3-embedding-0.6b";
pub const EMBEDDING_DIMS: &str = "1024";
pub const CHUNKING_VERSION: &str = "line-v1";
pub const EXTRACTOR_VERSION: &str = "plain-v1";

pub fn catalog_path(global: bool) -> PathBuf {
    if global {
        workspace::global_workspace_dir().join(CATALOG_FILE)
    } else {
        workspace::workspace_dir(&workspace::work_root()).join(CATALOG_FILE)
    }
}

/// Open a **writable staging** catalog: an in-memory DuckDB hydrated from the
/// current Iceberg snapshot (§8.8). There is no `catalog.duckdb` master and no
/// file lock — writers mutate this in-memory copy with the existing SQL, then
/// [`publish_catalog`] commits the changed rows back to the Iceberg warehouse
/// through the manual writer (DuckDB is never in the write path). Because it is
/// in-memory, mutations are lost unless `publish_catalog` is called before the
/// connection is dropped.
pub fn open_catalog(global: bool) -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    ensure_schema(&conn)?;
    icepub::hydrate(&conn, &iceberg::warehouse_uri(global))?;
    Ok(conn)
}

/// Commit a staging catalog's changes to the Iceberg warehouse for `global`:
/// per table, diff against the current snapshot and append data + equality
/// deletes for only the changed rows (§8.8). Idempotent; a no-op per table when
/// nothing changed.
pub fn publish_catalog(conn: &Connection, global: bool) -> Result<()> {
    icepub::publish(conn, &iceberg::warehouse_uri(global))
}

/// The Iceberg warehouse URI backing an explicit catalog-file path (the two
/// well-known repo/global catalogs honor the s3 backend; any other path maps to
/// a sibling `iceberg/` dir). Lets path-based writers (tag/fact mirroring,
/// lineage) address the same warehouse the scope-based API uses.
pub fn warehouse_for_path(path: &Path) -> String {
    iceberg::warehouse_for_catalog(path)
}

/// True once the warehouse behind `path` has any published table.
pub fn warehouse_published_at(path: &Path) -> bool {
    icepub::warehouse_published(&warehouse_for_path(path))
}

/// Open a writable staging catalog for the warehouse behind an explicit path:
/// an in-memory DuckDB hydrated from that snapshot. Commit with
/// [`publish_to_path`]; mutations are otherwise lost on drop.
pub fn open_catalog_at(path: &Path) -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    ensure_schema(&conn)?;
    icepub::hydrate(&conn, &warehouse_for_path(path))?;
    Ok(conn)
}

/// Commit a staging catalog opened with [`open_catalog_at`] back to the
/// warehouse behind `path`.
pub fn publish_to_path(conn: &Connection, path: &Path) -> Result<()> {
    icepub::publish(conn, &warehouse_for_path(path))
}

/// Open the published catalog **read-only** for a scope. Reads run through
/// `duckdb-iceberg` against the Iceberg snapshot (local dir or `s3://`), so any
/// number of concurrent readers — local or remote — see a consistent snapshot
/// with no file lock. `Ok(None)` when nothing has been published yet.
pub fn open_catalog_readonly(global: bool) -> Result<Option<Connection>> {
    iceberg::open_read_scope(global)
}

/// Read-only open for an explicit catalog-file path. The path selects the
/// Iceberg warehouse (repo/global honor the s3 backend; other paths map to a
/// sibling `iceberg/` dir). `Ok(None)` when unpublished.
pub fn open_readonly_path(path: &Path) -> Result<Option<Connection>> {
    iceberg::open_read(&iceberg::warehouse_for_catalog(path))
}

/// Catalog open for a read command: the published Iceberg snapshot when it
/// exists, else an in-memory empty catalog so the command runs and returns
/// nothing (no file, no lock) until the first sync publishes.
pub fn open_catalog_read(global: bool) -> Result<Connection> {
    match open_catalog_readonly(global)? {
        Some(conn) => Ok(conn),
        None => {
            let conn = Connection::open_in_memory()?;
            ensure_schema(&conn)?;
            Ok(conn)
        }
    }
}

pub fn read_preflight(global: bool) {
    if !global {
        cloud::auto_pull_if_due();
    }
    warn_if_stale(global);
}

fn warn_if_stale(global: bool) {
    let cfg = config::resolve(Some(&workspace::work_root()));
    let stale_days = cfg["sync"]["stale_days"].as_i64().unwrap_or(7);
    if stale_days <= 0 {
        return;
    }
    let path = catalog_path(global);
    let Ok(Some(conn)) = open_readonly_path(&path) else {
        return;
    };
    let last_sync: Option<String> = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE key = 'last_sync'",
            [],
            |r| r.get(0),
        )
        .ok();
    let Some(last_sync) = last_sync else {
        return;
    };
    let Some(age_days) = sync_age_days(&last_sync) else {
        return;
    };
    if age_days >= stale_days {
        eprintln!(
            "warning: index last synced {age_days}d ago (threshold {stale_days}d); run `mari sync`"
        );
    }
}

fn sync_age_days(last_sync: &str) -> Option<i64> {
    let sync_time = chrono::DateTime::parse_from_rfc3339(last_sync).ok()?;
    Some(
        chrono::Utc::now()
            .signed_duration_since(sync_time.with_timezone(&chrono::Utc))
            .num_days(),
    )
}

/// Idempotent schema migrations (SPEC §8.6): run ordered, version-gated
/// upgrades so an older catalog is brought forward instead of erroring. Each
/// step is a no-op when already applied. New migrations append here and bump
/// SCHEMA_VERSION.
fn migrate_schema(conn: &Connection) -> Result<()> {
    let current: Option<String> = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE key = 'schema.version'",
            [],
            |r| r.get(0),
        )
        .ok();
    // Version 1 is the baseline; no upgrade steps exist yet. Future steps:
    //   if version < 2 { ALTER TABLE …; }
    // Guarded by CREATE … IF NOT EXISTS in ensure_schema, additive columns
    // should use `ALTER TABLE … ADD COLUMN IF NOT EXISTS` inside a step.
    let _ = current;
    Ok(())
}

pub fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
CREATE TABLE IF NOT EXISTS schema_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sources (
  source_id TEXT PRIMARY KEY,
  provider TEXT NOT NULL,
  scope TEXT NOT NULL,
  connector_version TEXT NOT NULL,
  auth_provider TEXT,
  list_keys_json TEXT NOT NULL,
  config_hash TEXT NOT NULL,
  last_sync_at TEXT,
  last_success_at TEXT,
  last_error TEXT
);
CREATE TABLE IF NOT EXISTS documents (
  doc_id TEXT PRIMARY KEY,
  source_id TEXT NOT NULL,
  external_id TEXT NOT NULL,
  canonical_ref TEXT NOT NULL,
  title TEXT,
  url TEXT,
  path TEXT,
  mime_type TEXT,
  kind TEXT NOT NULL,
  author_id TEXT,
  author_name TEXT,
  created_at TEXT,
  updated_at TEXT,
  observed_at TEXT NOT NULL,
  version TEXT NOT NULL,
  content_sha256 TEXT NOT NULL,
  body TEXT NOT NULL,
  metadata_json TEXT NOT NULL,
  UNIQUE (source_id, external_id)
);
CREATE TABLE IF NOT EXISTS chunks (
  chunk_id TEXT PRIMARY KEY,
  doc_id TEXT NOT NULL,
  chunk_index INTEGER NOT NULL,
  heading_path TEXT NOT NULL,
  section_anchor TEXT,
  start_byte INTEGER NOT NULL,
  end_byte INTEGER NOT NULL,
  start_line INTEGER NOT NULL,
  end_line INTEGER NOT NULL,
  token_count INTEGER NOT NULL,
  text TEXT NOT NULL,
  text_sha256 TEXT NOT NULL,
  metadata_json TEXT NOT NULL,
  UNIQUE (doc_id, chunk_index)
);
CREATE TABLE IF NOT EXISTS embeddings (
  chunk_id TEXT PRIMARY KEY,
  model_id TEXT NOT NULL CHECK (model_id = 'qwen3-embedding-0.6b'),
  dims INTEGER NOT NULL,
  vector_json TEXT NOT NULL,
  norm REAL NOT NULL,
  embedded_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS spans (
  span_id TEXT PRIMARY KEY,
  doc_id TEXT NOT NULL,
  chunk_id TEXT,
  span_kind TEXT NOT NULL,
  label TEXT,
  start_byte INTEGER NOT NULL,
  end_byte INTEGER NOT NULL,
  start_line INTEGER NOT NULL,
  end_line INTEGER NOT NULL,
  stable_hash TEXT NOT NULL,
  metadata_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS symbols (
  symbol_id TEXT PRIMARY KEY,
  doc_id TEXT NOT NULL,
  span_id TEXT,
  language TEXT,
  symbol_kind TEXT NOT NULL,
  name TEXT NOT NULL,
  qualified_name TEXT NOT NULL,
  signature TEXT,
  start_byte INTEGER NOT NULL,
  end_byte INTEGER NOT NULL,
  start_line INTEGER NOT NULL,
  end_line INTEGER NOT NULL,
  metadata_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS edges (
  edge_id TEXT PRIMARY KEY,
  from_type TEXT NOT NULL,
  from_id TEXT NOT NULL,
  to_type TEXT NOT NULL,
  to_id TEXT NOT NULL,
  rel TEXT NOT NULL,
  confidence REAL NOT NULL DEFAULT 1.0,
  evidence_span_id TEXT,
  created_by TEXT NOT NULL,
  created_at TEXT NOT NULL,
  metadata_json TEXT NOT NULL,
  UNIQUE (from_type, from_id, to_type, to_id, rel)
);
CREATE TABLE IF NOT EXISTS lineage_edges (
  lineage_id TEXT PRIMARY KEY,
  from_span_id TEXT NOT NULL,
  to_span_id TEXT NOT NULL,
  rel TEXT NOT NULL,
  status TEXT NOT NULL,
  confidence REAL NOT NULL,
  confirmed_by TEXT,
  confirmed_at TEXT,
  last_checked_at TEXT,
  metadata_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS facts (
  fact_id TEXT PRIMARY KEY,
  claim TEXT NOT NULL,
  source_ref TEXT,
  source_span_id TEXT,
  status TEXT NOT NULL,
  created_by TEXT NOT NULL,
  created_at TEXT NOT NULL,
  metadata_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS tags (
  target_type TEXT NOT NULL,
  target_id TEXT NOT NULL,
  status TEXT NOT NULL,
  note TEXT,
  "by" TEXT NOT NULL,
  "at" TEXT NOT NULL,
  metadata_json TEXT NOT NULL,
  PRIMARY KEY (target_type, target_id)
);
CREATE TABLE IF NOT EXISTS sync_events (
  event_id TEXT PRIMARY KEY,
  source_id TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT,
  status TEXT NOT NULL,
  docs_seen INTEGER NOT NULL DEFAULT 0,
  docs_changed INTEGER NOT NULL DEFAULT 0,
  docs_deleted INTEGER NOT NULL DEFAULT 0,
  error TEXT,
  metadata_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_documents_source_updated ON documents(source_id, updated_at);
CREATE INDEX IF NOT EXISTS idx_documents_ref ON documents(canonical_ref);
CREATE INDEX IF NOT EXISTS idx_chunks_doc_byte ON chunks(doc_id, start_byte, end_byte);
CREATE INDEX IF NOT EXISTS idx_chunks_heading ON chunks(heading_path);
CREATE INDEX IF NOT EXISTS idx_spans_doc_byte ON spans(doc_id, start_byte, end_byte);
CREATE INDEX IF NOT EXISTS idx_symbols_qualified ON symbols(qualified_name);
CREATE INDEX IF NOT EXISTS idx_edges_from ON edges(from_type, from_id, rel);
CREATE INDEX IF NOT EXISTS idx_edges_to ON edges(to_type, to_id, rel);
CREATE INDEX IF NOT EXISTS idx_lineage_from ON lineage_edges(from_span_id, status);
CREATE INDEX IF NOT EXISTS idx_lineage_to ON lineage_edges(to_span_id, status);
CREATE INDEX IF NOT EXISTS idx_tags_status ON tags(status);
"#,
    )?;
    conn.execute_batch(VIEW_DDL)?;
    migrate_schema(conn)?;
    set_meta(conn, "schema.version", SCHEMA_VERSION)?;
    set_meta_default(conn, "schema.created_at", &now())?;
    set_meta(conn, "schema.migrated_at", &now())?;
    // Identity is stamped on creation only; sync_vectors rewrites it when it
    // actually writes vectors. Overwriting here would defeat the guard that
    // detects a catalog embedded by a different model (§7.1).
    set_meta_default(conn, "embedding.model", EMBEDDING_MODEL)?;
    set_meta_default(conn, "embedding.dims", EMBEDDING_DIMS)?;
    set_meta(conn, "chunking.version", CHUNKING_VERSION)?;
    set_meta(conn, "extractor.version", EXTRACTOR_VERSION)?;
    Ok(())
}

/// Derived views over the catalog base tables. Kept in a shared const so both
/// the writable DuckDB catalog (`ensure_schema`) and the read-only Iceberg
/// connection (`iceberg::open_read`, where the base tables are `iceberg_scan`
/// views) build identical view definitions. Referenced base-table names resolve
/// to whichever form exists in the target connection.
pub const VIEW_DDL: &str = r#"
CREATE OR REPLACE VIEW navigation_targets AS
SELECT
  'doc' AS target_type,
  d.doc_id AS target_id,
  d.doc_id,
  NULL AS chunk_id,
  NULL AS span_id,
  NULL AS symbol_id,
  d.source_id,
  d.canonical_ref,
  d.title,
  d.url,
  d.path,
  d.kind,
  NULL AS label,
  NULL AS language,
  NULL AS qualified_name,
  0 AS start_byte,
  length(d.body) AS end_byte,
  1 AS start_line,
  CAST(length(d.body) - length(replace(d.body, chr(10), '')) + 1 AS INTEGER) AS end_line,
  d.metadata_json
FROM documents d
UNION ALL
SELECT
  'chunk' AS target_type,
  c.chunk_id AS target_id,
  c.doc_id,
  c.chunk_id,
  NULL AS span_id,
  NULL AS symbol_id,
  d.source_id,
  d.canonical_ref,
  d.title,
  d.url,
  d.path,
  d.kind,
  c.heading_path AS label,
  NULL AS language,
  NULL AS qualified_name,
  c.start_byte,
  c.end_byte,
  c.start_line,
  c.end_line,
  c.metadata_json
FROM chunks c
JOIN documents d ON d.doc_id = c.doc_id
UNION ALL
SELECT
  'span' AS target_type,
  s.span_id AS target_id,
  s.doc_id,
  s.chunk_id,
  s.span_id,
  NULL AS symbol_id,
  d.source_id,
  d.canonical_ref,
  d.title,
  d.url,
  d.path,
  d.kind,
  COALESCE(s.label, s.span_kind) AS label,
  NULL AS language,
  NULL AS qualified_name,
  s.start_byte,
  s.end_byte,
  s.start_line,
  s.end_line,
  s.metadata_json
FROM spans s
JOIN documents d ON d.doc_id = s.doc_id
UNION ALL
SELECT
  'symbol' AS target_type,
  y.symbol_id AS target_id,
  y.doc_id,
  NULL AS chunk_id,
  y.span_id,
  y.symbol_id,
  d.source_id,
  d.canonical_ref,
  d.title,
  d.url,
  d.path,
  d.kind,
  y.name AS label,
  y.language,
  y.qualified_name,
  y.start_byte,
  y.end_byte,
  y.start_line,
  y.end_line,
  y.metadata_json
FROM symbols y
JOIN documents d ON d.doc_id = y.doc_id;
CREATE OR REPLACE VIEW graph_edges AS
SELECT
  e.edge_id,
  e.rel,
  e.from_type,
  e.from_id,
  from_target.doc_id AS from_doc_id,
  from_target.canonical_ref AS from_ref,
  from_target.path AS from_path,
  e.to_type,
  e.to_id,
  to_target.doc_id AS to_doc_id,
  to_target.canonical_ref AS to_ref,
  to_target.path AS to_path,
  e.confidence,
  e.evidence_span_id,
  e.created_by,
  e.created_at,
  e.metadata_json
FROM edges e
LEFT JOIN navigation_targets from_target
  ON from_target.target_type = e.from_type AND from_target.target_id = e.from_id
LEFT JOIN navigation_targets to_target
  ON to_target.target_type = e.to_type AND to_target.target_id = e.to_id;
"#;

pub fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute("DELETE FROM schema_meta WHERE key = ?1", [key])?;
    conn.execute(
        "INSERT INTO schema_meta (key, value) VALUES (?1, ?2)",
        [key, value],
    )?;
    Ok(())
}

fn set_meta_default(conn: &Connection, key: &str, value: &str) -> Result<()> {
    let exists: Option<String> = conn
        .query_row("SELECT value FROM schema_meta WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .ok();
    if exists.is_none() {
        set_meta(conn, key, value)?;
    }
    Ok(())
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn hash_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    format!("{:x}", h.finalize())
}

pub fn is_text_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "md" | "markdown"
                    | "mdown"
                    | "mkd"
                    | "mkdn"
                    | "mdx"
                    | "mdc"
                    | "txt"
                    | "text"
                    | "rst"
                    | "org"
                    | "adoc"
                    | "asciidoc"
                    | "asc"
                    | "textile"
                    | "tex"
                    | "me"
                    | "html"
                    | "htm"
                    | "xhtml"
            )
        })
        .unwrap_or(false)
}

pub fn repo_rel(path: &Path) -> String {
    let root = workspace::work_root();
    path.strip_prefix(&root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

pub fn sqlcmd(query: Option<&str>, global: bool) -> Result<i32> {
    let Some(q) = query else {
        println!("{}", catalog_path(global).display());
        return Ok(0);
    };
    read_preflight(global);
    if !is_read_only_sql(q) {
        eprintln!("mari sql is read-only; use SELECT/WITH/SHOW/DESCRIBE");
        return Ok(2);
    }
    let conn = open_catalog_read(global)?;
    let mut stmt = conn.prepare(q)?;
    let mut rows = stmt.query([])?;
    let stmt_ref = rows.as_ref().unwrap();
    let col_count = stmt_ref.column_count();
    let names = stmt_ref.column_names();
    println!("{}", names.join("\t"));
    let mut emitted = 0usize;
    while let Some(row) = rows.next()? {
        let mut vals = Vec::new();
        for i in 0..col_count {
            let v = row.get_ref(i)?;
            vals.push(format_value(v));
        }
        println!("{}", vals.join("\t"));
        emitted += 1;
    }
    Ok(read_result_exit_code(emitted))
}

fn read_result_exit_code(count: usize) -> i32 {
    if count == 0 {
        1
    } else {
        0
    }
}

fn is_read_only_sql(query: &str) -> bool {
    if has_multiple_sql_statements(query) {
        return false;
    }
    let sanitized = strip_sql_literals_and_comments(query);
    let tokens = sql_word_tokens(&sanitized);
    let Some(first) = tokens.first() else {
        return false;
    };
    matches!(first.as_str(), "select" | "with" | "show" | "describe")
        && !tokens.iter().any(|token| is_mutating_sql_token(token))
}

fn has_multiple_sql_statements(query: &str) -> bool {
    let sanitized = strip_sql_literals_and_comments(query);
    let trimmed = sanitized.trim();
    let without_final = trimmed.strip_suffix(';').unwrap_or(trimmed);
    without_final.contains(';')
}

fn strip_sql_literals_and_comments(query: &str) -> String {
    let mut out = String::with_capacity(query.len());
    let mut chars = query.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    while let Some(c) = chars.next() {
        if in_line_comment {
            if c == '\n' {
                in_line_comment = false;
                out.push('\n');
            } else {
                out.push(' ');
            }
            continue;
        }
        if in_block_comment {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block_comment = false;
                out.push_str("  ");
            } else {
                out.push(if c == '\n' { '\n' } else { ' ' });
            }
            continue;
        }
        if in_single {
            if c == '\'' {
                if chars.peek() == Some(&'\'') {
                    chars.next();
                    out.push_str("  ");
                } else {
                    in_single = false;
                    out.push(' ');
                }
            } else {
                out.push(if c == '\n' { '\n' } else { ' ' });
            }
            continue;
        }
        if in_double {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    out.push_str("  ");
                } else {
                    in_double = false;
                    out.push(' ');
                }
            } else {
                out.push(if c == '\n' { '\n' } else { ' ' });
            }
            continue;
        }
        if c == '-' && chars.peek() == Some(&'-') {
            chars.next();
            in_line_comment = true;
            out.push_str("  ");
            continue;
        }
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block_comment = true;
            out.push_str("  ");
            continue;
        }
        if c == '\'' {
            in_single = true;
            out.push(' ');
            continue;
        }
        if c == '"' {
            in_double = true;
            out.push(' ');
            continue;
        }
        out.push(c);
    }
    out
}

fn sql_word_tokens(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
        .collect()
}

fn is_mutating_sql_token(token: &str) -> bool {
    matches!(
        token,
        "alter"
            | "attach"
            | "call"
            | "checkpoint"
            | "copy"
            | "create"
            | "delete"
            | "detach"
            | "drop"
            | "export"
            | "grant"
            | "import"
            | "insert"
            | "install"
            | "load"
            | "merge"
            | "pragma"
            | "replace"
            | "reset"
            | "revoke"
            | "set"
            | "truncate"
            | "update"
            | "vacuum"
    )
}

fn format_value(v: duckdb::types::ValueRef<'_>) -> String {
    use duckdb::types::ValueRef;
    match v {
        ValueRef::Null => "".into(),
        ValueRef::Boolean(b) => b.to_string(),
        ValueRef::TinyInt(n) => n.to_string(),
        ValueRef::SmallInt(n) => n.to_string(),
        ValueRef::Int(n) => n.to_string(),
        ValueRef::BigInt(n) => n.to_string(),
        ValueRef::HugeInt(n) => n.to_string(),
        ValueRef::UTinyInt(n) => n.to_string(),
        ValueRef::USmallInt(n) => n.to_string(),
        ValueRef::UInt(n) => n.to_string(),
        ValueRef::UBigInt(n) => n.to_string(),
        ValueRef::Float(n) => n.to_string(),
        ValueRef::Double(n) => n.to_string(),
        ValueRef::Text(s) => String::from_utf8_lossy(s).replace('\n', "\\n"),
        ValueRef::Blob(b) => format!("<{} bytes>", b.len()),
        _ => format!("{v:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{ensure_schema, is_read_only_sql, is_text_path, sync_age_days};
    use duckdb::Connection;
    use std::path::Path;

    #[test]
    fn sql_allowlist_matches_spec() {
        for query in [
            "SELECT * FROM documents",
            "with x as (select 1) select * from x",
            "SHOW TABLES",
            "describe documents",
            "\n\tSELECT 1",
            "SELECT 'delete from docs' AS example;",
            "-- delete from docs\nSELECT 1",
            "/* drop table docs */ SHOW TABLES",
        ] {
            assert!(is_read_only_sql(query), "{query}");
        }

        for query in [
            "",
            "PRAGMA database_list",
            "INSERT INTO documents VALUES (1)",
            "UPDATE documents SET title = 'x'",
            "DELETE FROM documents",
            "CREATE TABLE x AS SELECT 1",
            "SELECT 1; DELETE FROM documents",
            "WITH doomed AS (DELETE FROM documents RETURNING *) SELECT * FROM doomed",
            "WITH x AS (SELECT 1) DROP TABLE documents",
            "SELECT * FROM documents; UPDATE documents SET title = 'x'",
        ] {
            assert!(!is_read_only_sql(query), "{query}");
        }
    }

    #[test]
    fn empty_sql_results_exit_nonzero() {
        assert_eq!(super::read_result_exit_code(0), 1);
        assert_eq!(super::read_result_exit_code(1), 0);
    }

    #[test]
    fn schema_stamps_embedding_identity_on_creation() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_schema(&conn).unwrap();

        let dims: String = conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = 'embedding.dims'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(dims, super::EMBEDDING_DIMS);
        let model: String = conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = 'embedding.model'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(model, super::EMBEDDING_MODEL);
    }

    #[test]
    fn sync_age_days_parses_rfc3339() {
        let old = (chrono::Utc::now() - chrono::Duration::days(9)).to_rfc3339();
        assert!(sync_age_days(&old).unwrap() >= 8);
        assert!(sync_age_days("not a timestamp").is_none());
    }

    #[test]
    fn localfiles_accept_spec_text_and_html_extensions_only() {
        for ext in [
            "md", "markdown", "mdown", "mkd", "mkdn", "mdx", "txt", "text", "rst", "org", "adoc",
            "asciidoc", "asc", "textile", "tex", "me", "html", "htm", "xhtml",
        ] {
            assert!(is_text_path(Path::new(&format!("doc.{ext}"))), "{ext}");
        }
        for ext in ["rs", "js", "log", "pdf", "docx", "pptx", "xlsx"] {
            assert!(!is_text_path(Path::new(&format!("doc.{ext}"))), "{ext}");
        }
    }

    #[test]
    fn schema_exposes_navigation_views() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO documents (doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind, author_id, author_name, created_at, updated_at, observed_at, version, content_sha256, body, metadata_json)
             VALUES ('doc1', 'localfiles', 'docs/a.md', 'localfiles:docs/a.md', 'A', NULL, 'docs/a.md', 'text/markdown', 'file', NULL, NULL, NULL, NULL, 'now', 'v', 'sha', '# A\nBody\nEnd', '{}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO chunks (chunk_id, doc_id, chunk_index, heading_path, section_anchor, start_byte, end_byte, start_line, end_line, token_count, text, text_sha256, metadata_json)
             VALUES ('chunk1', 'doc1', 0, 'A', NULL, 0, 3, 1, 1, 1, '# A', 'sha', '{}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO spans (span_id, doc_id, chunk_id, span_kind, label, start_byte, end_byte, start_line, end_line, stable_hash, metadata_json)
             VALUES ('span1', 'doc1', 'chunk1', 'heading', 'A', 0, 3, 1, 1, 'hash', '{}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO symbols (symbol_id, doc_id, span_id, language, symbol_kind, name, qualified_name, signature, start_byte, end_byte, start_line, end_line, metadata_json)
             VALUES ('symbol1', 'doc1', 'span1', 'markdown', 'heading', 'A', 'docs/a.md#A', '# A', 0, 3, 1, 1, '{}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO edges (edge_id, from_type, from_id, to_type, to_id, rel, confidence, evidence_span_id, created_by, created_at, metadata_json)
             VALUES ('edge1', 'doc', 'doc1', 'doc', 'doc1', 'links_to', 1.0, 'span1', 'test', 'now', '{}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO edges (edge_id, from_type, from_id, to_type, to_id, rel, confidence, evidence_span_id, created_by, created_at, metadata_json)
             VALUES ('edge2', 'symbol', 'symbol1', 'span', 'span1', 'documents', 1.0, 'span1', 'test', 'now', '{}')",
            [],
        )
        .unwrap();

        let targets: i64 = conn
            .query_row("SELECT COUNT(*) FROM navigation_targets", [], |row| {
                row.get(0)
            })
            .unwrap();
        let edge_ref: String = conn
            .query_row(
                "SELECT from_ref FROM graph_edges WHERE edge_id = 'edge1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(targets, 4);
        assert_eq!(edge_ref, "localfiles:docs/a.md");
        let doc_end_line: i64 = conn
            .query_row(
                "SELECT end_line FROM navigation_targets WHERE target_type = 'doc' AND target_id = 'doc1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(doc_end_line, 3);
        let (from_doc_id, from_ref, to_doc_id, to_ref): (String, String, String, String) = conn
            .query_row(
                "SELECT from_doc_id, from_ref, to_doc_id, to_ref FROM graph_edges WHERE edge_id = 'edge2'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(from_doc_id, "doc1");
        assert_eq!(from_ref, "localfiles:docs/a.md");
        assert_eq!(to_doc_id, "doc1");
        assert_eq!(to_ref, "localfiles:docs/a.md");
    }
}
