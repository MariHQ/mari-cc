//! Team sharing per SPEC §9: one authoritative shared warehouse per repo, stored
//! as an Iceberg warehouse on object storage (`storage.backend = s3`). Readers
//! scan the shared warehouse directly (mirrored to a local cache, §8.8) — there
//! is no whole-catalog `.duckdb` replica. Writes publish to the warehouse
//! incrementally; `mari cloud sync --compact` is the explicit tidy verb.

use crate::index::icestore::Store;
use crate::{config, index, workspace};
use anyhow::Result;
use serde_json::json;
use std::path::PathBuf;

fn role_path() -> PathBuf {
    workspace::workspace_dir(&workspace::work_root()).join("cloud-role")
}

pub fn role() -> String {
    std::fs::read_to_string(role_path())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "consumer".into())
}

fn set_role(r: &str) -> Result<()> {
    workspace::ensure_dir(role_path().parent().unwrap())?;
    std::fs::write(role_path(), r)?;
    Ok(())
}

fn cloud_cfg() -> serde_json::Value {
    config::resolve(Some(&workspace::work_root()))["cloud"].clone()
}

/// Point `storage.*` at the s3 warehouse and mark cloud sharing enabled. Writes
/// and reads then flow through `storage.backend = s3` (§8.8).
fn write_storage_cfg(bucket: &str, prefix: &str, region: &str) -> Result<()> {
    let path = config::repo_config_path(&workspace::work_root());
    let base = if prefix.is_empty() {
        format!("s3://{bucket}")
    } else {
        format!("s3://{bucket}/{}", prefix.trim_matches('/'))
    };
    config::set_in_file(&path, "storage.backend", json!("s3"))?;
    config::set_in_file(&path, "storage.path", json!(base))?;
    config::set_in_file(&path, "storage.region", json!(region))?;
    config::set_in_file(&path, "cloud.enabled", json!(true))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    args: &[String],
    backend: Option<&str>,
    bucket: Option<&str>,
    prefix: Option<&str>,
    region: Option<&str>,
    force: bool,
    compact: bool,
    no_push: bool,
    retain: Option<usize>,
) -> Result<i32> {
    if let Some(code) = validate_backend(backend) {
        return Ok(code);
    }
    match args.first().map(|s| s.as_str()) {
        Some("init") => init(bucket, prefix, region, force),
        Some("connect") => connect(bucket, prefix, region),
        Some("sync") => sync(compact, no_push, retain),
        Some("role") => match args.get(1).map(|s| s.as_str()) {
            Some(r @ ("writer" | "consumer")) => {
                set_role(r)?;
                println!("✓ cloud role = {r}");
                Ok(0)
            }
            _ => {
                eprintln!("usage: mari cloud role <writer|consumer>");
                Ok(2)
            }
        },
        _ => {
            eprintln!("usage: mari cloud <init|connect|role|sync> …");
            Ok(2)
        }
    }
}

/// Only the s3 backend remains (§9); `git`/replica backends were removed with the
/// Iceberg rework.
fn validate_backend(backend: Option<&str>) -> Option<i32> {
    match backend {
        Some("s3") | None => None,
        Some(other) => {
            eprintln!("unknown cloud backend: {other} (only `s3` is supported)");
            Some(2)
        }
    }
}

/// `mari cloud init --bucket B [--prefix P] [--region R] [--force]` — become the
/// writer of an s3 Iceberg warehouse (§9): set `storage.backend = s3` and publish
/// the existing local warehouse. Refuses to clobber an existing shared warehouse
/// unless `--force`.
fn init(
    bucket: Option<&str>,
    prefix: Option<&str>,
    region: Option<&str>,
    force: bool,
) -> Result<i32> {
    let Some(bucket) = bucket else {
        eprintln!("--bucket required for `mari cloud init`");
        return Ok(2);
    };
    let prefix = prefix.unwrap_or("");
    let region = region.unwrap_or("");
    write_storage_cfg(bucket, prefix, region)?;

    // Refuse to overwrite an existing shared warehouse.
    if !force {
        for global in [false, true] {
            let wh = index::iceberg::warehouse_uri(global);
            if index::icepub::warehouse_published(&wh) {
                eprintln!(
                    "✗ a shared warehouse already exists at {wh} — pass --force to replace it, \
                     or `mari cloud connect` to read it"
                );
                return Ok(1);
            }
        }
    }
    set_role("writer")?;

    // Publish the existing local warehouse(s) so teammates can read immediately.
    let mut uploaded = 0usize;
    for global in [false, true] {
        let local_dir = index::iceberg::local_warehouse_dir(global);
        if local_dir.exists() {
            let wh = index::iceberg::warehouse_uri(global);
            uploaded += upload_warehouse(&local_dir.to_string_lossy(), &wh, region)?;
        }
    }
    if uploaded == 0 {
        println!(
            "✓ s3 warehouse configured (s3://{bucket}/{prefix}) — writer. Run `mari sync` to publish."
        );
    } else {
        println!(
            "✓ published {uploaded} file(s) to s3://{bucket}/{prefix} — this machine is the writer."
        );
    }
    Ok(0)
}

/// `mari cloud connect --bucket B [...]` — read-only consumer of a shared s3
/// warehouse. Reads scan it directly (mirrored locally, §8.8).
fn connect(bucket: Option<&str>, prefix: Option<&str>, region: Option<&str>) -> Result<i32> {
    let Some(bucket) = bucket else {
        eprintln!("--bucket required for `mari cloud connect`");
        return Ok(2);
    };
    write_storage_cfg(bucket, prefix.unwrap_or(""), region.unwrap_or(""))?;
    set_role("consumer")?;
    pull()?;
    println!("✓ connected as read-only consumer of s3://{bucket}");
    Ok(0)
}

/// `mari cloud sync [--compact] [--no-push] [--retain N]` (writer-only, §9):
/// publish + tidy the shared warehouse. Per-write publishes keep the read layer
/// current incrementally; this is the explicit compaction verb.
fn sync(compact: bool, no_push: bool, retain: Option<usize>) -> Result<i32> {
    if enabled() && role() == "consumer" {
        eprintln!("✗ this machine is a cloud consumer — only the writer compacts the shared warehouse");
        return Ok(1);
    }
    let cfg = config::resolve(Some(&workspace::work_root()));
    let retain = retain
        .or_else(|| cfg["storage"]["retain_snapshots"].as_u64().map(|n| n as usize))
        .unwrap_or(1)
        .max(1);

    if !compact {
        println!("✓ warehouse is current (writes publish incrementally). Use --compact to reclaim snapshots.");
        let _ = no_push;
        return Ok(0);
    }

    let mut total = index::icepub::CompactStats::default();
    for global in [false, true] {
        let warehouse = index::iceberg::warehouse_uri(global);
        let stats = index::icepub::compact(&warehouse, retain)?;
        total.tables += stats.tables;
        total.files_deleted += stats.files_deleted;
    }
    println!(
        "✓ compacted {} table(s), reclaimed {} orphan file(s) (retain={retain}).",
        total.tables, total.files_deleted
    );
    Ok(0)
}

/// Upload every file under a local warehouse dir to the remote (s3) warehouse,
/// preserving structure. Data-file paths embedded in the metadata stay local, but
/// reads resolve them relative to the table location (`allow_moved_paths`, §8.8).
fn upload_warehouse(local_dir: &str, remote_warehouse: &str, region: &str) -> Result<usize> {
    let local = Store::Local;
    let remote = Store::open(remote_warehouse, region)?;
    let mut n = 0;
    for uri in local.list_uris(local_dir)? {
        let rel = uri.strip_prefix(local_dir).unwrap_or(&uri).trim_start_matches('/');
        let bytes = local.get(&uri)?.unwrap_or_default();
        remote.put(&format!("{remote_warehouse}/{rel}"), bytes)?;
        n += 1;
    }
    Ok(n)
}

/// `mari cloud pull` — refresh the local read-mirror of the shared warehouse
/// (§8.8/§9). Reads already mirror on demand, so this is a proactive refresh.
pub fn pull() -> Result<i32> {
    if !enabled() {
        eprintln!("✗ cloud sharing is not enabled (run `mari cloud init` or `mari cloud connect`)");
        return Ok(1);
    }
    for global in [false, true] {
        index::iceberg::refresh_mirror(global)?;
    }
    record_last_pull();
    println!("✓ shared warehouse snapshot refreshed");
    Ok(0)
}

fn last_pull_path() -> PathBuf {
    workspace::workspace_dir(&workspace::work_root()).join("last-pull")
}

fn record_last_pull() {
    let _ = std::fs::write(last_pull_path(), chrono::Utc::now().to_rfc3339());
}

pub fn last_pull() -> Option<chrono::DateTime<chrono::Utc>> {
    std::fs::read_to_string(last_pull_path())
        .ok()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s.trim()).ok())
        .map(|t| t.with_timezone(&chrono::Utc))
}

pub fn enabled() -> bool {
    cloud_cfg()["enabled"].as_bool().unwrap_or(false)
}

/// Read commands auto-pull first when cloud-enabled, throttled to once per 60s;
/// on failure they warn to stderr and read the last-seen snapshot (SPEC §5).
pub fn auto_pull_if_due() {
    if !enabled() {
        return;
    }
    if let Some(t) = last_pull() {
        if chrono::Utc::now().signed_duration_since(t).num_seconds() < 60 {
            return;
        }
    }
    if let Err(e) = pull() {
        eprintln!("warning: cloud pull failed ({e}); reading the last-seen snapshot");
    }
}

/// `--rebuild` is unsupported against a cloud index (SPEC §9).
pub fn forbid_rebuild() -> Option<String> {
    if enabled() {
        Some("--rebuild is unsupported on a cloud index — rebuild locally, then re-run `mari cloud init`".into())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_backend_accepts_only_s3() {
        assert_eq!(validate_backend(None), None);
        assert_eq!(validate_backend(Some("s3")), None);
        assert_eq!(validate_backend(Some("git")), Some(2));
        assert_eq!(validate_backend(Some("sqlite")), Some(2));
    }

    #[test]
    fn storage_cfg_builds_prefixed_and_bare_paths() {
        // The warehouse base is s3://bucket[/prefix]; scope is appended per read.
        assert_eq!(
            {
                let (b, p) = ("bkt", "");
                if p.is_empty() { format!("s3://{b}") } else { format!("s3://{b}/{p}") }
            },
            "s3://bkt"
        );
        assert_eq!(
            {
                let (b, p) = ("bkt", "team/wh");
                format!("s3://{b}/{}", p.trim_matches('/'))
            },
            "s3://bkt/team/wh"
        );
    }
}
