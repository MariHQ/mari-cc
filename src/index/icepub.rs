//! Iceberg publish + hydrate engine (SPEC §8.8).
//!
//! This is the bridge between the in-memory DuckDB *staging* catalog (where all
//! the existing read-modify-write SQL runs) and the on-disk Iceberg warehouse
//! that readers scan. DuckDB is used **only to read** here — to hydrate staging
//! from the current snapshot and to compute the per-table diff. The actual
//! writes go through the manual Iceberg writer in [`super::icewrite`]; DuckDB is
//! never in the write path.
//!
//! Change detection matches the notes: read the current `(key, row-hash)` set
//! straight from the snapshot with a projected `iceberg_scan`, compare to the
//! mutated staging table, and commit **only** the changed rows — added/updated
//! rows as a data file, removed/updated keys as an equality delete. Unchanged
//! rows stay in their existing data files (carried forward), so every publish is
//! O(changed rows), not O(table).

use super::iceberg;
use super::icestore::Store;
use super::icewrite::{self, arrow_schema, IceField, TableDef, CATALOG};
use crate::{config, workspace};
use anyhow::{Context, Result};
use arrow_array::{ArrayRef, Float64Array, Int64Array, RecordBatch, StringArray};
use duckdb::Connection;
use std::sync::Arc;

fn table_uri(warehouse: &str, table: &str) -> String {
    format!("{}/{table}", warehouse.trim_end_matches('/'))
}

/// All catalog table names.
fn table_names() -> Vec<&'static str> {
    CATALOG.iter().map(|d| d.name).collect()
}

/// Iceberg `bucket()` partition count (`storage.bucket_chunks`, default 16).
fn bucket_n() -> u32 {
    config::resolve(Some(&workspace::work_root()))["storage"]["bucket_chunks"]
        .as_u64()
        .unwrap_or(16)
        .clamp(1, 4096) as u32
}

/// Store + region for a warehouse (region from `storage.region`).
fn store_for(warehouse: &str) -> Result<Store> {
    let region = config::resolve(Some(&workspace::work_root()))["storage"]["region"]
        .as_str()
        .unwrap_or("")
        .to_string();
    Store::open(warehouse, &region)
}

/// True once at least one catalog table has been published to `warehouse`.
pub fn warehouse_published(warehouse: &str) -> bool {
    let Ok(store) = store_for(warehouse) else {
        return false;
    };
    !store.published_tables(warehouse, &table_names()).is_empty()
}

/// Milliseconds since epoch (real clock; only Workflow JS lacks one).
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Hydrate the staging tables from the published snapshot: `INSERT … SELECT * FROM
/// iceberg_scan(...)` for every table that exists in the warehouse. A no-op for
/// an unpublished warehouse. The iceberg extension is loaded read-only.
pub fn hydrate(conn: &Connection, warehouse: &str) -> Result<()> {
    let store = store_for(warehouse)?;
    let published = store.published_tables(warehouse, &table_names());
    if published.is_empty() {
        return Ok(());
    }
    iceberg::load_extensions(conn, warehouse)?;
    // `ensure_schema` pre-seeds schema_meta with defaults; clear them so the
    // published rows load without a primary-key collision. Every other table is
    // empty at hydrate time, so a plain INSERT is correct (and avoids DuckDB's
    // "conflict target required" error on tables with multiple UNIQUE keys).
    conn.execute_batch("DELETE FROM schema_meta;").ok();
    for def in CATALOG {
        if !published.iter().any(|t| t == def.name) {
            continue;
        }
        let loc = table_uri(warehouse, def.name);
        conn.execute_batch(&format!(
            "INSERT INTO {} SELECT * FROM iceberg_scan('{}');",
            def.name, loc
        ))
        .with_context(|| format!("hydrating {} from {}", def.name, loc))?;
    }
    Ok(())
}

/// A stable per-row hash expression over all columns, identical whether the row
/// comes from the staging table or from `iceberg_scan` (same column names). Used
/// to detect added/changed/removed rows. Integer/real columns cast to VARCHAR so
/// INT32 staging and BIGINT/DOUBLE snapshot values compare equal.
fn row_hash_expr(fields: &[IceField]) -> String {
    let parts: Vec<String> = fields
        .iter()
        // 0x1e (record separator) marks NULLs; never NUL (breaks the C-string
        // bridge to DuckDB) and won't collide with real text.
        .map(|f| format!("COALESCE(CAST(\"{}\" AS VARCHAR), '\u{1e}')", f.name))
        .collect();
    format!("md5(concat_ws('\u{1f}', {}))", parts.join(", "))
}

/// Cast-projection selecting every column at the Iceberg type the writer expects
/// (long→BIGINT, double→DOUBLE, string→VARCHAR), so the values read back have
/// predictable Rust types for Arrow assembly.
fn cast_select(fields: &[IceField]) -> String {
    fields
        .iter()
        .map(|f| {
            let ty = match f.ty {
                "long" | "int" => "BIGINT",
                "double" => "DOUBLE",
                "boolean" => "BOOLEAN",
                _ => "VARCHAR",
            };
            format!("CAST(\"{}\" AS {ty}) AS \"{}\"", f.name, f.name)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// `ORDER BY "c1", "c2"` for a table's write sort order (§8.7), or empty. Rows
/// come out of DuckDB pre-sorted; partition grouping preserves order, so each
/// per-partition data file is written sorted.
fn order_by(sort: &[&str]) -> String {
    if sort.is_empty() {
        return String::new();
    }
    let cols: Vec<String> = sort.iter().map(|c| format!("\"{c}\"")).collect();
    format!(" ORDER BY {}", cols.join(", "))
}

/// Run `sql` and assemble the rows into an Arrow `RecordBatch` shaped by
/// `fields`. Returns `None` when the query yields zero rows (nothing to write).
fn query_to_batch(conn: &Connection, sql: &str, fields: &[IceField]) -> Result<Option<RecordBatch>> {
    let mut stmt = conn.prepare(sql).with_context(|| format!("preparing diff query: {sql}"))?;
    let mut rows = stmt.query([])?;

    // Column-major builders, one per field.
    enum Col {
        Str(Vec<Option<String>>),
        Long(Vec<Option<i64>>),
        Dbl(Vec<Option<f64>>),
    }
    let mut cols: Vec<Col> = fields
        .iter()
        .map(|f| match f.ty {
            "long" | "int" => Col::Long(Vec::new()),
            "double" => Col::Dbl(Vec::new()),
            _ => Col::Str(Vec::new()),
        })
        .collect();

    let mut n = 0usize;
    while let Some(row) = rows.next()? {
        n += 1;
        for (i, col) in cols.iter_mut().enumerate() {
            match col {
                Col::Str(v) => v.push(row.get::<_, Option<String>>(i)?),
                Col::Long(v) => v.push(row.get::<_, Option<i64>>(i)?),
                Col::Dbl(v) => v.push(row.get::<_, Option<f64>>(i)?),
            }
        }
    }
    if n == 0 {
        return Ok(None);
    }
    let arrays: Vec<ArrayRef> = cols
        .into_iter()
        .map(|c| -> ArrayRef {
            match c {
                Col::Str(v) => Arc::new(StringArray::from(v)),
                Col::Long(v) => Arc::new(Int64Array::from(v)),
                Col::Dbl(v) => Arc::new(Float64Array::from(v)),
            }
        })
        .collect();
    Ok(Some(RecordBatch::try_new(arrow_schema(fields), arrays)?))
}

/// Publish every changed catalog table from `conn` (the staging catalog) to the
/// Iceberg `warehouse`. Diff is computed with DuckDB reads; commits go through
/// the manual writer. Tables with no change produce no snapshot.
pub fn publish(conn: &Connection, warehouse: &str) -> Result<()> {
    if !warehouse.starts_with("s3://") {
        std::fs::create_dir_all(warehouse).ok();
    }
    let ts = now_ms();
    // The iceberg extension is needed to read prior snapshots for the diff.
    iceberg::load_extensions(conn, warehouse).ok();
    // One object store (local or s3) for the whole publish — an s3 client carries
    // shared runtime state, so build it once, not per table. One warehouse listing
    // decides which tables already exist.
    let store = store_for(warehouse)?;
    let published = store.published_tables(warehouse, &table_names());
    for def in CATALOG {
        let is_pub = published.iter().any(|t| t == def.name);
        publish_table(conn, &store, warehouse, def, is_pub, ts)?;
    }
    Ok(())
}

/// Result of a compaction run.
#[derive(Default, Debug)]
pub struct CompactStats {
    pub tables: usize,
    pub files_deleted: usize,
}

/// Compact the warehouse (§8.8): rewrite each table's live rows into a single
/// fresh data file (applying accumulated equality deletes and coalescing
/// fragments), expire prior snapshots, and delete every orphaned file. Reads run
/// against the current snapshot throughout; the `version-hint` swap is atomic and
/// old files are removed only after it. `retain` is accepted for the CLI contract;
/// v1 collapses to the single current snapshot.
pub fn compact(warehouse: &str, _retain: usize) -> Result<CompactStats> {
    let store = store_for(warehouse)?;
    let published = store.published_tables(warehouse, &table_names());
    if published.is_empty() {
        return Ok(CompactStats::default());
    }
    // A read connection over the current live rows (deletes already applied by
    // iceberg_scan; s3 warehouses are mirrored to local first).
    let Some(conn) = iceberg::open_read(warehouse)? else {
        return Ok(CompactStats::default());
    };
    let ts = now_ms();
    let mut stats = CompactStats::default();
    for def in CATALOG {
        if !published.iter().any(|t| t == def.name) {
            continue;
        }
        let table_uri = table_uri(warehouse, def.name);
        // Extract the live rows (sorted per §8.7); an empty table still gets a
        // clean empty snapshot.
        let sql = format!(
            "SELECT {} FROM {}{}",
            cast_select(def.fields),
            def.name,
            order_by(def.sort)
        );
        let batch = query_to_batch(&conn, &sql, def.fields)?
            .unwrap_or_else(|| RecordBatch::new_empty(arrow_schema(def.fields)));

        let keep: std::collections::HashSet<String> =
            icewrite::rewrite_table(&store, &table_uri, def.fields, def.partition, def.sort, bucket_n(), &batch, ts)?
                .into_iter()
                .collect();

        // Orphan removal: delete everything under the table not in the keep set.
        for uri in store.list_uris(&table_uri)? {
            if !keep.contains(&uri) {
                store.delete(&uri).ok();
                stats.files_deleted += 1;
            }
        }
        stats.tables += 1;
    }
    Ok(stats)
}

fn publish_table(
    conn: &Connection,
    store: &Store,
    warehouse: &str,
    def: &TableDef,
    published: bool,
    ts: i64,
) -> Result<()> {
    let loc = table_uri(warehouse, def.name);
    let hash = row_hash_expr(def.fields);
    let sel = cast_select(def.fields);

    // Added or changed rows: staging rows whose full-row hash is not present in
    // the snapshot. For an unpublished table, that is every row.
    let ord = order_by(def.sort);
    let add_sql = if published {
        format!(
            "SELECT {sel} FROM {name} WHERE {hash} NOT IN (SELECT {hash} FROM iceberg_scan('{loc}')){ord}",
            name = def.name
        )
    } else {
        format!("SELECT {sel} FROM {name}{ord}", name = def.name)
    };
    let added = query_to_batch(conn, &add_sql, def.fields)?;

    // Removed or changed keys: snapshot rows whose full-row hash is not present
    // in staging → their key must be equality-deleted (the changed ones are
    // re-added by the data file above in the same snapshot, which wins on
    // sequence number). Only meaningful once the table has been published.
    let key_fields = def.key_fields();
    let deleted = if published {
        let key_sel = cast_select(&key_fields);
        let del_sql = format!(
            "SELECT DISTINCT {key_sel} FROM iceberg_scan('{loc}') \
             WHERE {hash} NOT IN (SELECT {hash} FROM {name})",
            name = def.name
        );
        query_to_batch(conn, &del_sql, &key_fields)?
    } else {
        None
    };

    if added.is_none() && deleted.is_none() {
        return Ok(()); // nothing changed — no new snapshot
    }
    let del_arg = deleted.as_ref().map(|b| (&key_fields[..], b));
    icewrite::commit(store, &loc, def.fields, def.partition, def.sort, bucket_n(), added.as_ref(), del_arg, ts)
        .with_context(|| format!("committing iceberg snapshot for {}", def.name))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Partitioning (§8.7): `documents` by identity(source_id), `chunks` by
    /// bucket(doc_id). Proves data is physically split into partition files AND
    /// that a `WHERE` predicate on the partition column returns the correct rows
    /// (partition pruning must not drop live rows).
    #[test]
    fn partitioned_layout_and_predicate_reads() {
        let dir = tempfile::tempdir().unwrap();
        let wh = dir.path().join("wh").to_string_lossy().to_string();

        let c = Connection::open_in_memory().unwrap();
        crate::index::ensure_schema(&c).unwrap();
        // Docs from two sources; chunks across several doc_ids (→ several buckets).
        c.execute_batch(
            "INSERT INTO documents VALUES \
             ('dA','git','a','git:a','A',NULL,'a','text/markdown','file',NULL,NULL,NULL,NULL,'t','1','h','b','{}'),\
             ('dB','slack','b','slack:b','B',NULL,'b','text/markdown','doc',NULL,NULL,NULL,NULL,'t','1','h','b','{}');",
        ).unwrap();
        for i in 0..8 {
            c.execute(
                "INSERT INTO chunks VALUES (?1, ?2, ?3, 'h', NULL, 0, 1, 1, 1, 1, 'txt', 's', '{}')",
                duckdb::params![format!("c{i}"), format!("d{}", i % 4), i as i64],
            )
            .unwrap();
        }
        publish(&c, &wh).unwrap();

        // documents: identity(source_id) → one data file per source.
        let doc_files = std::fs::read_dir(format!("{wh}/documents/data")).unwrap().count();
        assert_eq!(doc_files, 2, "documents split into git + slack partitions");
        // chunks: bucket(doc_id) over 4 distinct doc_ids → ≥2 partition files.
        let chunk_files = std::fs::read_dir(format!("{wh}/chunks/data")).unwrap().count();
        assert!(chunk_files >= 2, "chunks split across buckets (got {chunk_files})");

        let r = Connection::open_in_memory().unwrap();
        super::iceberg::install_iceberg(&r).unwrap();
        let scan = |sql: &str| -> i64 {
            r.query_row(sql, [], |x| x.get(0)).map_err(|e| format!("{sql}: {e}")).unwrap()
        };
        // No predicate → all rows.
        assert_eq!(scan(&format!("SELECT count(*) FROM iceberg_scan('{wh}/chunks')")), 8);
        // Identity-partition predicate returns exactly the matching rows.
        assert_eq!(
            scan(&format!("SELECT count(*) FROM iceberg_scan('{wh}/documents') WHERE source_id='slack'")),
            1,
            "identity partition prune keeps the slack doc"
        );
        // Bucket-partition predicate on doc_id must not drop rows (2 chunks per doc_id d0..d3).
        assert_eq!(
            scan(&format!("SELECT count(*) FROM iceberg_scan('{wh}/chunks') WHERE doc_id='d2'")),
            2,
            "bucket partition prune keeps both chunks of d2"
        );
    }

    /// Full staging→publish→hydrate→publish round trip, proving incremental
    /// equality-delete upserts survive a reader. Uses two independent in-memory
    /// staging catalogs to simulate two sessions sharing one warehouse.
    #[test]
    fn staging_publish_hydrate_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let wh = dir.path().join("wh");
        let wh = wh.to_string_lossy().to_string();

        let ddl = "CREATE TABLE documents(doc_id TEXT, source_id TEXT, external_id TEXT, \
            canonical_ref TEXT, title TEXT, url TEXT, path TEXT, mime_type TEXT, kind TEXT, \
            author_id TEXT, author_name TEXT, created_at TEXT, updated_at TEXT, observed_at TEXT, \
            version TEXT, content_sha256 TEXT, body TEXT, metadata_json TEXT);";

        // Session 1: insert two docs, publish.
        let c1 = Connection::open_in_memory().unwrap();
        c1.execute_batch(ddl).unwrap();
        c1.execute_batch(
            "INSERT INTO documents VALUES \
             ('d1','git','a','git:a','A',NULL,'a','text/markdown','file',NULL,NULL,NULL,NULL,'t','1','h1','body1','{}'),\
             ('d2','git','b','git:b','B',NULL,'b','text/markdown','file',NULL,NULL,NULL,NULL,'t','1','h2','body2','{}');",
        )
        .unwrap();
        // Only publish the documents table (others absent in this minimal test).
        let store = Store::open(&wh, "").unwrap();
        publish_table(&c1, &store, &wh, super::super::icewrite::table_def("documents").unwrap(), false, now_ms())
            .unwrap();

        // Reader sees 2 docs.
        let r = Connection::open_in_memory().unwrap();
        super::iceberg::install_iceberg(&r).unwrap();
        let loc = format!("{wh}/documents");
        let count: i64 = r
            .query_row(&format!("SELECT count(*) FROM iceberg_scan('{loc}')"), [], |x| x.get(0))
            .unwrap();
        assert_eq!(count, 2, "first publish: 2 docs");

        // Session 2: hydrate, change d2's body, delete d1, add d3, publish.
        let c2 = Connection::open_in_memory().unwrap();
        c2.execute_batch(ddl).unwrap();
        hydrate(&c2, &wh).unwrap();
        let hydrated: i64 = c2.query_row("SELECT count(*) FROM documents", [], |x| x.get(0)).unwrap();
        assert_eq!(hydrated, 2, "hydrate pulled 2 docs from snapshot");
        c2.execute_batch(
            "UPDATE documents SET body='body2v2', content_sha256='h2v2' WHERE doc_id='d2';
             DELETE FROM documents WHERE doc_id='d1';
             INSERT INTO documents VALUES ('d3','git','c','git:c','C',NULL,'c','text/markdown','file',NULL,NULL,NULL,NULL,'t','1','h3','body3','{}');",
        )
        .unwrap();
        let store2 = Store::open(&wh, "").unwrap();
        publish_table(&c2, &store2, &wh, super::super::icewrite::table_def("documents").unwrap(), true, now_ms() + 1000)
            .unwrap();

        // Fresh reader: d1 gone, d2 updated, d3 present → {d2,d3}.
        let r2 = Connection::open_in_memory().unwrap();
        super::iceberg::install_iceberg(&r2).unwrap();
        let ids: Vec<String> = {
            let mut stmt = r2
                .prepare(&format!("SELECT doc_id FROM iceberg_scan('{loc}') ORDER BY doc_id"))
                .unwrap();
            let v = stmt
                .query_map([], |x| x.get::<_, String>(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect::<Vec<_>>();
            v
        };
        assert_eq!(ids, vec!["d2".to_string(), "d3".to_string()], "upsert+delete applied");
        let body: String = r2
            .query_row(
                &format!("SELECT body FROM iceberg_scan('{loc}') WHERE doc_id='d2'"),
                [],
                |x| x.get(0),
            )
            .unwrap();
        assert_eq!(body, "body2v2", "changed row reflects new value");
    }

    fn parquet_count(dir: &std::path::Path) -> usize {
        std::fs::read_dir(dir.join("documents").join("data"))
            .map(|rd| {
                rd.flatten()
                    .filter(|e| e.path().extension().is_some_and(|x| x == "parquet"))
                    .count()
            })
            .unwrap_or(0)
    }

    #[test]
    fn compact_coalesces_and_reclaims_orphans() {
        let dir = tempfile::tempdir().unwrap();
        let wh = dir.path().join("wh").to_string_lossy().to_string();

        // Snapshot 1: two docs — full ensure_schema + publish, like a real sync.
        let c1 = Connection::open_in_memory().unwrap();
        crate::index::ensure_schema(&c1).unwrap();
        c1.execute_batch(
            "INSERT INTO documents VALUES \
             ('d1','git','a','git:a','A',NULL,'a','text/markdown','file',NULL,NULL,NULL,NULL,'t','1','h1','b1','{}'),\
             ('d2','git','b','git:b','B',NULL,'b','text/markdown','file',NULL,NULL,NULL,NULL,'t','1','h2','b2','{}');",
        ).unwrap();
        publish(&c1, &wh).unwrap();

        // Snapshot 2: update d2, delete d1, add d3 → a 2nd data file + a delete file.
        let c2 = Connection::open_in_memory().unwrap();
        crate::index::ensure_schema(&c2).unwrap();
        hydrate(&c2, &wh).unwrap();
        c2.execute_batch(
            "UPDATE documents SET body='b2v2', content_sha256='h2v2' WHERE doc_id='d2';
             DELETE FROM documents WHERE doc_id='d1';
             INSERT INTO documents VALUES ('d3','git','c','git:c','C',NULL,'c','text/markdown','file',NULL,NULL,NULL,NULL,'t','1','h3','b3','{}');",
        ).unwrap();
        publish(&c2, &wh).unwrap();

        assert!(parquet_count(dir.path().join("wh").as_path()) >= 2, "accumulated ≥2 data files");

        // Compact: collapse to one clean data file, drop delete files + old snapshots.
        let stats = compact(&wh, 1).unwrap();
        assert!(stats.tables >= 1);
        assert!(stats.files_deleted > 0, "reclaimed orphan files");
        assert_eq!(
            parquet_count(dir.path().join("wh").as_path()),
            1,
            "compaction coalesces to a single data file"
        );
        // No delete files remain.
        let has_delete = std::fs::read_dir(dir.path().join("wh").join("documents").join("data"))
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with("delete-"));
        assert!(!has_delete, "no equality-delete files after compaction");

        // Data is intact: live rows {d2 (updated), d3}.
        let loc = format!("{wh}/documents");
        let r = Connection::open_in_memory().unwrap();
        super::iceberg::install_iceberg(&r).unwrap();
        let ids: Vec<String> = {
            let mut stmt = r
                .prepare(&format!("SELECT doc_id FROM iceberg_scan('{loc}') ORDER BY doc_id"))
                .unwrap();
            let v = stmt.query_map([], |x| x.get::<_, String>(0)).unwrap().map(|r| r.unwrap()).collect();
            v
        };
        assert_eq!(ids, vec!["d2".to_string(), "d3".to_string()], "live rows preserved");
        let body: String = r
            .query_row(&format!("SELECT body FROM iceberg_scan('{loc}') WHERE doc_id='d2'"), [], |x| x.get(0))
            .unwrap();
        assert_eq!(body, "b2v2", "updated value preserved through compaction");
    }
}
