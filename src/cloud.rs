//! Team sharing per SPEC §9: one authoritative shared catalog per repo,
//! full local replica per machine, reads always on the replica.
//! Backends: `git` (catalog under <repo>/.mari/catalog, data on Git LFS) and
//! `s3` (via the AWS CLI — implementation decision; keeps the core dependency-free).

use crate::{config, workspace};
use anyhow::{anyhow, Result};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;

const CATALOG_FILE: &str = "catalog.duckdb";

fn replica_path() -> PathBuf {
    workspace::workspace_dir(&workspace::work_root()).join(CATALOG_FILE)
}

fn git_catalog_path(root: &Path) -> PathBuf {
    root.join(".mari").join("catalog").join(CATALOG_FILE)
}

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

fn write_cloud_cfg(backend: &str, bucket: &str, prefix: &str, region: &str) -> Result<()> {
    let repo = workspace::work_root();
    let path = config::repo_config_path(&repo);
    config::set_in_file(&path, "cloud.enabled", json!(true))?;
    config::set_in_file(&path, "cloud.backend", json!(backend))?;
    config::set_in_file(&path, "cloud.bucket", json!(bucket))?;
    config::set_in_file(&path, "cloud.prefix", json!(prefix))?;
    config::set_in_file(&path, "cloud.region", json!(region))?;
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
    match args.first().map(|s| s.as_str()) {
        Some("init") => init(backend, bucket, prefix, region, force),
        Some("connect") => connect(backend, bucket, prefix, region),
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

/// `mari cloud sync [--compact] [--no-push] [--retain N]` (writer-only, §9):
/// publish + tidy the shared warehouse. Per-write publishes keep the read layer
/// current incrementally; this is the explicit push + compaction verb.
fn sync(compact: bool, no_push: bool, retain: Option<usize>) -> Result<i32> {
    use crate::{config, index, workspace};
    // One-writer rule: consumers never mutate the shared warehouse.
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
        // Writes already land in the configured warehouse (local dir or s3), so a
        // bare `cloud sync` has nothing to stage; nudge toward the tidy verb.
        println!("✓ warehouse is current (writes publish incrementally). Use --compact to reclaim snapshots.");
        let _ = no_push;
        return Ok(0);
    }

    let mut total = index::icepub::CompactStats::default();
    // Compact each published scope's warehouse (repo + global).
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

fn init(
    backend: Option<&str>,
    bucket: Option<&str>,
    prefix: Option<&str>,
    region: Option<&str>,
    force: bool,
) -> Result<i32> {
    if let Some(code) = validate_backend(backend) {
        return Ok(code);
    }
    let repo = workspace::work_root();
    let replica = replica_path();
    if !replica.exists() {
        eprintln!("✗ no local index yet — run `mari sync` first");
        return Ok(1);
    }
    if backend == Some("git") {
        let catalog_dir = repo.join(".mari").join("catalog");
        std::fs::create_dir_all(&catalog_dir)?;
        let target = catalog_dir.join(CATALOG_FILE);
        if should_refuse_overwrite(&target, force) {
            eprintln!(
                "✗ refusing to overwrite existing cloud catalog at {} — pass --force to replace it",
                target.display()
            );
            return Ok(1);
        }
        std::fs::copy(&replica, &target)?;
        // Replicate the Lance vector dataset alongside the catalog so a
        // consumer's search isn't silently keyword-only (§7 robustness).
        replicate_vectors_git(&catalog_dir)?;
        // Data files ride Git LFS (SPEC §9).
        let gitattributes = catalog_dir.join(".gitattributes");
        if !gitattributes.exists() {
            std::fs::write(
                &gitattributes,
                "*.duckdb filter=lfs diff=lfs merge=lfs -text\n*.lance filter=lfs diff=lfs merge=lfs -text\ncatalog/vectors.lance/** filter=lfs diff=lfs merge=lfs -text\n",
            )?;
        }
        write_cloud_cfg("git", "", "", "")?;
        set_role("writer")?;
        println!(
            "✓ git-backed cloud catalog at {} — this machine is the writer.",
            catalog_dir.display()
        );
        println!("commit .mari (with Git LFS enabled) so teammates can consume it.");
        Ok(0)
    } else {
        let Some(bucket) = bucket else {
            eprintln!("--bucket required for the s3 backend");
            return Ok(2);
        };
        let prefix = prefix.unwrap_or("");
        let region = region.unwrap_or("");
        write_cloud_cfg("s3", bucket, prefix, region)?;
        set_role("writer")?;
        push_s3(&replica, bucket, prefix, region)?;
        println!("✓ s3-backed cloud catalog pushed to s3://{bucket}/{prefix} — this machine is the writer.");
        Ok(0)
    }
}

fn should_refuse_overwrite(path: &Path, force: bool) -> bool {
    path.exists() && !force
}

fn connect(
    backend: Option<&str>,
    bucket: Option<&str>,
    prefix: Option<&str>,
    region: Option<&str>,
) -> Result<i32> {
    if let Some(code) = validate_backend(backend) {
        return Ok(code);
    }
    if backend == Some("git") {
        let root = workspace::work_root();
        let src = git_catalog_path(&root);
        if !src.exists() {
            eprintln!(
                "✗ no shared git catalog at {} — pull the writer's .mari/catalog first",
                src.display()
            );
            return Ok(1);
        }
        write_cloud_cfg("git", "", "", "")?;
        set_role("consumer")?;
        pull()?;
        println!("✓ connected as read-only consumer of git-backed catalog");
        return Ok(0);
    }

    let Some(bucket) = bucket else {
        eprintln!("--bucket required for the s3 backend");
        return Ok(2);
    };
    write_cloud_cfg("s3", bucket, prefix.unwrap_or(""), region.unwrap_or(""))?;
    set_role("consumer")?;
    pull()?;
    println!("✓ connected as read-only consumer of s3://{bucket}");
    Ok(0)
}

fn validate_backend(backend: Option<&str>) -> Option<i32> {
    match backend {
        Some("s3" | "git") | None => None,
        Some(other) => {
            eprintln!("unknown cloud backend: {other} (expected s3 | git)");
            Some(2)
        }
    }
}

fn s3_url(bucket: &str, prefix: &str) -> String {
    if prefix.is_empty() {
        format!("s3://{bucket}/{CATALOG_FILE}")
    } else {
        format!(
            "s3://{}/{}/{CATALOG_FILE}",
            bucket,
            prefix.trim_matches('/')
        )
    }
}

fn aws(args: &[&str], region: &str) -> Result<()> {
    let mut cmd = Command::new("aws");
    cmd.args(args);
    if !region.is_empty() {
        cmd.args(["--region", region]);
    }
    let out = cmd
        .output()
        .map_err(|_| anyhow!("aws CLI not found — the s3 backend shells out to `aws s3`"))?;
    if !out.status.success() {
        return Err(anyhow!(
            "aws failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Copy the local Lance vector dataset into the git catalog dir.
fn replicate_vectors_git(catalog_dir: &Path) -> Result<()> {
    let local = crate::index::vector::dataset_path(false);
    if local.exists() {
        let dst = catalog_dir.join("vectors.lance");
        if dst.exists() {
            std::fs::remove_dir_all(&dst)?;
        }
        copy_dir_recursive(&local, &dst)?;
    }
    Ok(())
}

/// Restore the Lance vector dataset from the git catalog dir into the replica.
fn restore_vectors_git() -> Result<()> {
    let src = workspace::work_root()
        .join(".mari")
        .join("catalog")
        .join("vectors.lance");
    if src.exists() {
        let dst = crate::index::vector::dataset_path(false);
        if dst.exists() {
            std::fs::remove_dir_all(&dst)?;
        }
        copy_dir_recursive(&src, &dst)?;
    }
    Ok(())
}

fn push_s3(replica: &Path, bucket: &str, prefix: &str, region: &str) -> Result<()> {
    aws(
        &[
            "s3",
            "cp",
            &replica.to_string_lossy(),
            &s3_url(bucket, prefix),
        ],
        region,
    )?;
    // Sync the Lance vector dataset directory so consumers get vectors too.
    let local = crate::index::vector::dataset_path(false);
    if local.exists() {
        aws(
            &[
                "s3",
                "sync",
                &local.to_string_lossy(),
                &s3_vectors_url(bucket, prefix),
                "--delete",
            ],
            region,
        )?;
    }
    Ok(())
}

fn s3_vectors_url(bucket: &str, prefix: &str) -> String {
    if prefix.is_empty() {
        format!("s3://{bucket}/vectors.lance")
    } else {
        format!("s3://{}/{}/vectors.lance", bucket, prefix.trim_matches('/'))
    }
}

/// `mari pull` — fetch the latest cloud index into the replica.
pub fn pull() -> Result<i32> {
    let cfg = cloud_cfg();
    if !cfg["enabled"].as_bool().unwrap_or(false) {
        eprintln!("✗ cloud sharing is not enabled (run `mari cloud init` or `mari cloud connect`)");
        return Ok(1);
    }
    let replica = replica_path();
    workspace::ensure_dir(replica.parent().unwrap())?;
    match cfg["backend"].as_str().unwrap_or("s3") {
        "git" => {
            let src = git_catalog_path(&workspace::work_root());
            if !src.exists() {
                eprintln!(
                    "✗ no shared catalog at {} — has the writer committed it?",
                    src.display()
                );
                return Ok(1);
            }
            std::fs::copy(&src, &replica)?;
            restore_vectors_git()?;
        }
        _ => {
            let bucket = cfg["bucket"].as_str().unwrap_or("");
            let prefix = cfg["prefix"].as_str().unwrap_or("");
            let region = cfg["region"].as_str().unwrap_or("");
            aws(
                &[
                    "s3",
                    "cp",
                    &s3_url(bucket, prefix),
                    &replica.to_string_lossy(),
                ],
                region,
            )?;
            let dst = crate::index::vector::dataset_path(false);
            let _ = aws(
                &[
                    "s3",
                    "sync",
                    &s3_vectors_url(bucket, prefix),
                    &dst.to_string_lossy(),
                    "--delete",
                ],
                region,
            );
        }
    }
    record_last_pull();
    println!("✓ replica updated at {}", replica.display());
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

/// Read commands auto-pull first when cloud-enabled, throttled to once per
/// 60s; on failure they warn to stderr and read the stale replica (SPEC §5).
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
        eprintln!("warning: cloud pull failed ({e}); reading the stale replica");
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
    fn git_catalog_path_matches_spec_location() {
        let root = Path::new("/repo");
        assert_eq!(
            git_catalog_path(root),
            PathBuf::from("/repo/.mari/catalog/catalog.duckdb")
        );
    }

    #[test]
    fn cloud_init_requires_force_for_existing_git_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CATALOG_FILE);
        std::fs::write(&path, "old").unwrap();

        assert!(should_refuse_overwrite(&path, false));
        assert!(!should_refuse_overwrite(&path, true));
    }

    #[test]
    fn cloud_backend_accepts_only_s3_or_git() {
        assert_eq!(validate_backend(None), None);
        assert_eq!(validate_backend(Some("s3")), None);
        assert_eq!(validate_backend(Some("git")), None);
        assert_eq!(validate_backend(Some("sqlite")), Some(2));
    }
}
