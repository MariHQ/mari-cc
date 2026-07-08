//! Iceberg read layer (SPEC §8.8).
//!
//! The catalog is a writable local DuckDB file that only the syncing/curating
//! process touches (§8.6). After every write it is **published** to Iceberg
//! tables — one filesystem-catalog Iceberg table per catalog table — under a
//! warehouse that is either a local directory or an `s3://` prefix. All *reads*
//! (search, audit, sql, hooks, curation queries) go through `duckdb-iceberg`
//! (`iceberg_scan`) against the published snapshot.
//!
//! Why: DuckDB takes a single-file lock for the lifetime of a read-write
//! connection and refuses any other opener — even a reader — for that whole
//! window. That made concurrent mari sessions ("Conflicting lock is held").
//! Iceberg tables are immutable snapshots published via an atomic
//! `version-hint.text` swap, so any number of local or remote readers query a
//! consistent snapshot while a writer builds the next one, with no lock and no
//! whole-file download. This is also the substrate for the cloud read service:
//! point the warehouse at `s3://…` and remote readers `iceberg_scan` it.

use crate::{config, index, workspace};
use anyhow::{Context, Result};
use duckdb::Connection;
use std::path::{Path, PathBuf};

/// Where the published Iceberg warehouse lives for a scope. Local by default
/// (`<workspace>/iceberg`); an `s3://bucket/prefix` base when `storage.backend`
/// is `s3`, with the scope appended so repo and global catalogs stay distinct.
pub fn warehouse_uri(global: bool) -> String {
    let cfg = config::resolve(Some(&workspace::work_root()));
    let backend = cfg["storage"]["backend"].as_str().unwrap_or("local");
    if backend == "s3" {
        let base = cfg["storage"]["path"].as_str().unwrap_or("").trim_end_matches('/');
        let scope: String = if global {
            "_global".to_string()
        } else {
            workspace::workspace_id(&workspace::work_root())
        };
        return format!("{base}/{scope}");
    }
    local_warehouse(&index::catalog_path(global)).to_string_lossy().to_string()
}

/// Local warehouse dir that sits beside a catalog file (`…/catalog.duckdb` →
/// `…/iceberg`). Used both by the scope-based API and by the path-based read
/// entry point (`open_readonly_path`).
pub fn local_warehouse(catalog_file: &Path) -> PathBuf {
    catalog_file
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("iceberg")
}

/// Resolve the warehouse for an explicit catalog-file path. Honors the s3
/// backend for the two well-known catalog files (repo + global); any other path
/// (e.g. a test fixture) maps to its sibling `iceberg/` dir.
pub fn warehouse_for_catalog(catalog_file: &Path) -> String {
    let cfg = config::resolve(Some(&workspace::work_root()));
    if cfg["storage"]["backend"].as_str() == Some("s3") {
        let repo = index::catalog_path(false);
        let global = workspace::global_workspace_dir().join(index::CATALOG_FILE);
        if catalog_file == repo {
            return warehouse_uri(false);
        }
        if catalog_file == global {
            return warehouse_uri(true);
        }
    }
    local_warehouse(catalog_file).to_string_lossy().to_string()
}

fn is_s3(uri: &str) -> bool {
    uri.starts_with("s3://")
}

/// Any warehouse that reads over the network — `s3://` (a user's cloud) or
/// `https://` (the CDN-hosted KB, v2). Both go through `cache_httpfs`.
fn is_remote(uri: &str) -> bool {
    is_s3(uri) || uri.starts_with("https://") || uri.starts_with("http://")
}

/// `cache_httpfs` on-disk cache directory (§4.4 `storage.cache_dir`, default
/// `~/.mari/cache/httpfs`). Applied at connection open so repeat remote reads of
/// the same Iceberg metadata/Parquet pages are served locally.
fn cache_dir() -> PathBuf {
    let cfg = config::resolve(Some(&workspace::work_root()));
    match cfg["storage"]["cache_dir"].as_str().filter(|s| !s.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => config::mari_home().join("cache").join("httpfs"),
    }
}

/// Region for the s3 client (from `storage.region`).
fn storage_region() -> String {
    config::resolve(Some(&workspace::work_root()))["storage"]["region"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

/// `INSTALL iceberg; LOAD iceberg;` serialized process-wide. INSTALL writes into
/// the shared `~/.duckdb` extension cache, so concurrent installs from multiple
/// connections (or parallel test threads) can race on cold cache; the mutex
/// makes first-use safe. LOAD is per-connection and cheap.
pub fn install_iceberg(conn: &Connection) -> Result<()> {
    static EXT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = EXT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Pin the extension cache to a stable directory rather than DuckDB's default
    // `$HOME/.duckdb`. `$HOME` is process-global and some tests override it for
    // workspace isolation; without pinning, a concurrent test's temp `$HOME`
    // would send the extension install/load to a missing directory. This dir is
    // machine-stable, so first-use downloads once and subsequent loads are
    // offline.
    let dir = extension_dir();
    std::fs::create_dir_all(&dir).ok();
    conn.execute_batch(&format!("SET extension_directory='{}';", dir.display()))
        .ok();
    conn.execute_batch("INSTALL iceberg; LOAD iceberg;")
        .context("loading the duckdb iceberg extension")?;
    Ok(())
}

/// Stable, `$HOME`-independent DuckDB extension cache (`<tmp>/mari-duckdb-ext`).
fn extension_dir() -> PathBuf {
    std::env::temp_dir().join("mari-duckdb-ext")
}

/// Load the read-path extensions on `conn`: the signed `iceberg` extension
/// always, and — for a remote warehouse (`s3://` or `https://`) — the
/// `cache_httpfs` community extension as the **sole** remote filesystem (§8.8).
/// We deliberately do **not** load plain `httpfs`: `cache_httpfs` provides the
/// s3/http filesystem itself *and* layers an on-disk read cache over it, so
/// loading `httpfs` alongside it double-registers the filesystem. `iceberg`
/// autoinstalls from the signed registry; `cache_httpfs` from the community
/// registry. Both cache under `~/.duckdb`; subsequent loads are offline.
pub fn load_extensions(conn: &Connection, uri: &str) -> Result<()> {
    conn.execute_batch(
        "SET autoinstall_known_extensions=true; SET autoload_known_extensions=true;",
    )
    .ok();
    install_iceberg(conn)?;
    if is_remote(uri) {
        conn.execute_batch("INSTALL cache_httpfs FROM community; LOAD cache_httpfs;")
            .context("loading the duckdb cache_httpfs extension (the sole remote filesystem)")?;
        // Point the on-disk read cache at storage.cache_dir (§4.4).
        let dir = cache_dir();
        std::fs::create_dir_all(&dir).ok();
        conn.execute_batch(&format!(
            "SET cache_httpfs_cache_directory='{}';",
            dir.display()
        ))
        .ok();
        // An `s3://` warehouse needs credentials (via the AWS credential chain,
        // so we never persist keys); a public `https://` CDN warehouse needs no
        // secret.
        if is_s3(uri) {
            let cfg = config::resolve(Some(&workspace::work_root()));
            let region = cfg["storage"]["region"].as_str().unwrap_or("");
            let region_clause = if region.is_empty() {
                String::new()
            } else {
                format!(", REGION '{region}'")
            };
            conn.execute_batch(&format!(
                "CREATE SECRET IF NOT EXISTS mari_s3 (TYPE s3, PROVIDER credential_chain{region_clause});"
            ))
            .context("creating the duckdb s3 secret")?;
        }
    }
    Ok(())
}

/// Build a read-only connection over the published warehouse: an in-memory
/// DuckDB whose base-table names are `iceberg_scan` views and whose derived
/// views (`navigation_targets`, `graph_edges`) match the writable catalog.
/// `Ok(None)` when nothing has been published yet.
pub fn open_read(uri: &str) -> Result<Option<Connection>> {
    // For an s3 warehouse, mirror it to local disk once and read locally — the
    // warehouse is small and its data files are immutable, so reading it directly
    // over s3 (a serial metadata round-trip per table on every command) is far
    // slower than a local scan. `moved` means the mirrored metadata still carries
    // the original s3 file paths, so `iceberg_scan` must resolve data files
    // relative to the table location (`allow_moved_paths`).
    let (read_uri, moved) = read_warehouse(uri)?;

    // One listing decides which tables exist (local stat after mirroring).
    let store = crate::index::icestore::Store::open(&read_uri, "")?;
    let published = store.published_tables(&read_uri, &index::CATALOG_TABLES);
    if !published.iter().any(|t| t == "schema_meta") {
        return Ok(None);
    }
    let conn = Connection::open_in_memory()?;
    load_extensions(&conn, &read_uri)?;
    let moved_arg = if moved { ", allow_moved_paths = true" } else { "" };
    for table in index::CATALOG_TABLES {
        // A table may be absent if it was never published (e.g. a fresh catalog
        // that only wrote schema_meta). Present tables become views; missing ones
        // get empty typed stand-ins below.
        if !published.iter().any(|t| t == table) {
            continue;
        }
        let loc = table_uri(&read_uri, table);
        conn.execute_batch(&format!(
            "CREATE VIEW {table} AS SELECT * FROM iceberg_scan('{loc}'{moved_arg});"
        ))
        .with_context(|| format!("mounting iceberg table {table} from {loc}"))?;
    }
    // Any base table not published yet is created as an empty typed table so the
    // derived views (and read SQL) resolve.
    ensure_missing_tables(&conn, &published)?;
    conn.execute_batch(index::VIEW_DDL)
        .context("building catalog views over iceberg tables")?;
    Ok(Some(conn))
}

/// Resolve the URI a read should actually scan: a local warehouse as-is, or an
/// s3 warehouse mirrored to a local cache dir. Returns `(uri, moved)` where
/// `moved` is true for the mirrored case (data paths in the metadata still point
/// at s3, so the reader must be told the files "moved" to the local location).
fn read_warehouse(warehouse: &str) -> Result<(String, bool)> {
    if !is_s3(warehouse) {
        return Ok((warehouse.to_string(), false));
    }
    let store = crate::index::icestore::Store::open(warehouse, &storage_region())?;
    let mirror = mirror_dir(warehouse);
    std::fs::create_dir_all(&mirror).ok();
    let mirror = mirror.to_string_lossy().to_string();
    store.mirror(warehouse, &mirror)?;
    Ok((mirror, true))
}

/// Local mirror directory for a remote warehouse (`<cache>/mirror/<hash>`).
fn mirror_dir(warehouse: &str) -> PathBuf {
    let hash = crate::index::hash_hex(warehouse);
    config::mari_home()
        .join("cache")
        .join("mirror")
        .join(&hash[..16])
}

/// Read-open for a scope (repo/global).
pub fn open_read_scope(global: bool) -> Result<Option<Connection>> {
    open_read(&warehouse_uri(global))
}

/// Proactively refresh the local read-mirror of a scope's s3 warehouse (used by
/// `mari cloud pull` and throttled auto-pull). No-op for a local backend.
pub fn refresh_mirror(global: bool) -> Result<()> {
    let wh = warehouse_uri(global);
    if is_s3(&wh) {
        read_warehouse(&wh)?; // mirrors as a side effect
    }
    Ok(())
}

/// Local warehouse directory for a scope, regardless of the configured backend
/// (used by `mari cloud init` to find data to upload to s3).
pub fn local_warehouse_dir(global: bool) -> PathBuf {
    local_warehouse(&index::catalog_path(global))
}

fn table_uri(uri: &str, table: &str) -> String {
    format!("{}/{table}", uri.trim_end_matches('/'))
}

/// Create empty, correctly-typed stand-ins for any base table that has no
/// published Iceberg data yet, so the shared `VIEW_DDL` and downstream reads
/// never hit an unknown table. Shapes come from the canonical schema applied to
/// a throwaway in-memory catalog.
fn ensure_missing_tables(conn: &Connection, published: &[String]) -> Result<()> {
    let missing: Vec<&str> = index::CATALOG_TABLES
        .into_iter()
        .filter(|t| !published.iter().any(|p| p == t))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let tmp = Connection::open_in_memory()?;
    index::ensure_schema(&tmp)?;
    for table in missing {
        // `CREATE TABLE … AS SELECT … WHERE false` clones the column types
        // without rows. Pull the shape from the canonical schema connection.
        let ddl = table_shape_ddl(&tmp, table)?;
        conn.execute_batch(&ddl)
            .with_context(|| format!("creating empty stand-in for {table}"))?;
    }
    Ok(())
}

/// Column list + types for `table` in `schema_conn`, rendered as an empty
/// `CREATE TABLE`. DuckDB's `information_schema` gives portable type names.
fn table_shape_ddl(schema_conn: &Connection, table: &str) -> Result<String> {
    let mut stmt = schema_conn.prepare(
        "SELECT column_name, data_type FROM information_schema.columns \
         WHERE table_name = ? ORDER BY ordinal_position",
    )?;
    let cols: Vec<(String, String)> = stmt
        .query_map([table], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<std::result::Result<_, _>>()?;
    let body = cols
        .iter()
        .map(|(n, t)| format!("\"{n}\" {t}"))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!("CREATE TABLE {table} ({body});"))
}
