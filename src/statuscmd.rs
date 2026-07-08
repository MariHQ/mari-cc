//! `mari status` per SPEC §5.1: workspace, cloud, embedding identity,
//! sync age, per-source lines, detector + hook state, tag counts.

use crate::{authcmd, cloud, config, index, workspace};
use anyhow::Result;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Default)]
struct CatalogStatus {
    indexed: BTreeMap<String, i64>,
    last_sync: Option<String>,
    embedding_models: BTreeSet<String>,
    tag_counts: BTreeMap<String, usize>,
    mirrored_tag_entry_targets: BTreeSet<String>,
}

pub fn run() -> Result<i32> {
    let root = workspace::work_root();
    let cfg = config::resolve(Some(&root));
    let ws = workspace::workspace_dir(&root);
    println!("workspace: {}", ws.display());

    // Cloud.
    if cloud::enabled() {
        let cc = &cfg["cloud"];
        let remote = match cc["backend"].as_str() {
            Some("git") => format!("git: {}/.mari/catalog", root.display()),
            _ => format!(
                "s3://{}/{}",
                cc["bucket"].as_str().unwrap_or(""),
                cc["prefix"].as_str().unwrap_or("")
            ),
        };
        let last = cloud::last_pull()
            .map(|t| t.to_rfc3339())
            .unwrap_or_else(|| "never".into());
        println!(
            "cloud: role={} remote={} last-pull={}",
            cloud::role(),
            remote,
            last
        );
    } else {
        println!("cloud: disabled");
    }

    // Catalog-backed facts (embedding identity, last sync, per-source counts).
    let catalog_status = catalog_status_from_paths(&status_catalog_paths(&root));

    match embedding_line(&catalog_status) {
        Some(line) => {
            println!("embedding: {line}");
            if catalog_status
                .embedding_models
                .iter()
                .any(|m| m != index::EMBEDDING_MODEL)
            {
                println!("  warning: index embedding differs from this build — run `mari sync --rebuild`");
            }
        }
        None => println!("embedding: {} (no index yet)", index::EMBEDDING_MODEL),
    }

    let stale_days = cfg["sync"]["stale_days"].as_i64().unwrap_or(7);
    match &catalog_status.last_sync {
        Some(t) => {
            let age_days = chrono::DateTime::parse_from_rfc3339(t)
                .map(|t| {
                    chrono::Utc::now()
                        .signed_duration_since(t.with_timezone(&chrono::Utc))
                        .num_days()
                })
                .unwrap_or(0);
            print!("last sync: {t} ({age_days}d ago)");
            if stale_days > 0 && age_days >= stale_days {
                print!("  — stale; run `mari sync`");
            }
            println!();
        }
        None => println!("last sync: never — run `mari sync`"),
    }

    // Per-source lines.
    println!("sources:");
    for (key, label, auth, list_keys) in source_table() {
        let scope = workspace::source_scope(key);
        let connected = match auth {
            None => "local",
            Some(p) => {
                if authcmd::credential(p).is_some() {
                    "connected"
                } else {
                    "not connected"
                }
            }
        };
        let tracked: usize = list_keys
            .iter()
            .filter_map(|k| config::get_path(&cfg, k))
            .filter_map(|v| v.as_array())
            .map(|a| a.len())
            .sum();
        let idx = catalog_status.indexed.get(key).copied().unwrap_or(0);
        println!("  {label:<14} {scope:<6} {connected:<13} tracked={tracked} indexed={idx}");
    }

    // Detector + hook.
    let style = cfg["detector"]["styleGuide"]
        .as_str()
        .unwrap_or("microsoft");
    let hook_installed = std::fs::read_to_string(root.join(".claude").join("settings.json"))
        .map(|s| s.contains("mari hook"))
        .unwrap_or(false);
    println!(
        "detector: style={style}  hook={}",
        if hook_installed {
            "installed"
        } else {
            "not installed"
        }
    );

    // Tag counts by status.
    let counts = combined_tag_counts(&cfg, &catalog_status);
    if counts.is_empty() {
        println!("tags: none");
    } else {
        let line: Vec<String> = counts.iter().map(|(k, v)| format!("{k}={v}")).collect();
        println!("tags: {}", line.join(" "));
    }
    Ok(0)
}

fn status_catalog_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = vec![
        workspace::workspace_dir(root).join(index::CATALOG_FILE),
        workspace::global_workspace_dir().join(index::CATALOG_FILE),
    ];
    paths.sort();
    paths.dedup();
    // Keep both well-known catalog paths; the reader resolves each to its Iceberg
    // warehouse and skips any that is unpublished (there is no catalog.duckdb file
    // to stat anymore, §8.8).
    paths
}

fn catalog_status_from_paths(paths: &[PathBuf]) -> CatalogStatus {
    let mut status = CatalogStatus::default();
    for path in paths {
        // Read-only over the published Iceberg snapshot; None when nothing has
        // been synced for this scope yet.
        let Ok(Some(conn)) = index::open_readonly_path(path) else {
            continue;
        };
        if let Ok(mut stmt) =
            conn.prepare("SELECT source_id, COUNT(*) FROM documents GROUP BY source_id")
        {
            if let Ok(rows) =
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            {
                for row in rows.flatten() {
                    *status.indexed.entry(row.0).or_default() += row.1;
                }
            }
        }
        if let Some(last_sync) = schema_meta_value(&conn, "last_sync") {
            if status
                .last_sync
                .as_deref()
                .map(|current| last_sync.as_str() > current)
                .unwrap_or(true)
            {
                status.last_sync = Some(last_sync);
            }
        }
        if let Some(model) = schema_meta_value(&conn, "embedding.model") {
            status.embedding_models.insert(model);
        }
        if let Ok(mut stmt) = conn.prepare("SELECT DISTINCT model_id FROM embeddings") {
            if let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) {
                for model in rows.flatten() {
                    status.embedding_models.insert(model);
                }
            }
        }
        if let Ok(mut stmt) = conn.prepare("SELECT status, metadata_json FROM tags") {
            if let Ok(rows) =
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            {
                for (tag_status, metadata_json) in rows.flatten() {
                    *status.tag_counts.entry(tag_status).or_default() += 1;
                    if let Some(target) = mirrored_tag_target(&metadata_json) {
                        status.mirrored_tag_entry_targets.insert(target);
                    }
                }
            }
        }
    }
    status
}

fn schema_meta_value(conn: &duckdb::Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM schema_meta WHERE key = ?1", [key], |r| {
        r.get::<_, String>(0)
    })
    .ok()
}

fn mirrored_tag_target(metadata_json: &str) -> Option<String> {
    let value: Value = serde_json::from_str(metadata_json).ok()?;
    if value["source"].as_str()? != "tags.entries" {
        return None;
    }
    normalize_tag_target(value["target"].as_str()?)
}

fn normalize_tag_target(target: &str) -> Option<String> {
    let normalized = target.strip_prefix("./").unwrap_or(target).trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

fn combined_tag_counts(cfg: &Value, catalog_status: &CatalogStatus) -> BTreeMap<String, usize> {
    let mut counts = catalog_status.tag_counts.clone();
    if let Some(entries) = cfg["tags"]["entries"].as_object() {
        for (target, entry) in entries {
            let Some(target) = normalize_tag_target(target) else {
                continue;
            };
            if catalog_status.mirrored_tag_entry_targets.contains(&target) {
                continue;
            }
            if let Some(status) = entry["status"].as_str() {
                *counts.entry(status.to_string()).or_default() += 1;
            }
        }
    }
    counts
}

fn embedding_line(status: &CatalogStatus) -> Option<String> {
    if status.embedding_models.is_empty() {
        return None;
    }
    Some(
        status
            .embedding_models
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(", "),
    )
}

#[allow(clippy::type_complexity)]
fn source_table() -> Vec<(
    &'static str,
    &'static str,
    Option<&'static str>,
    Vec<&'static str>,
)> {
    vec![
        ("slack", "Slack", Some("slack"), vec!["slack.channels"]),
        (
            "gdocs",
            "Google Drive",
            Some("google"),
            vec!["google.docs", "google.folders"],
        ),
        ("github", "GitHub", Some("github"), vec!["github.repos"]),
        (
            "confluence",
            "Confluence",
            Some("confluence"),
            vec!["confluence.spaces", "confluence.pages"],
        ),
        ("jira", "Jira", Some("jira"), vec!["jira.projects"]),
        (
            "zendesk",
            "Zendesk",
            Some("zendesk"),
            vec!["zendesk.include"],
        ),
        (
            "salesforce",
            "Salesforce",
            Some("salesforce"),
            vec!["salesforce.objects"],
        ),
        (
            "hubspot",
            "HubSpot",
            Some("hubspot"),
            vec!["hubspot.include"],
        ),
        (
            "microsoft",
            "Microsoft 365",
            Some("microsoft"),
            vec!["microsoft.drives", "microsoft.mail", "microsoft.teams"],
        ),
        (
            "discord",
            "Discord",
            Some("discord"),
            vec!["discord.channels", "discord.guilds"],
        ),
        (
            "linear",
            "Linear",
            Some("linear"),
            vec!["linear.teams", "linear.projects"],
        ),
        ("git", "Git history", None, vec!["git.repos"]),
        ("localfiles", "Local files", None, vec!["localfiles.paths"]),
    ]
}

/// `mari doctor` — report which optional external tools and models are present
/// and which features they gate (SPEC §22 / portability). Never fails; it is a
/// diagnostic.
pub fn doctor() -> Result<i32> {
    println!("mari doctor — optional dependencies and models\n");

    println!("external tools:");
    for (tool, gates) in [
        (
            "git",
            "git-history connector, commit-association hook, humanizer, curator identity",
        ),
        (
            "gcloud",
            "Google Drive connector (rides your gcloud session)",
        ),
        ("aws", "S3 cloud-sharing backend"),
        (
            "python3",
            "optional Unlimited-OCR model tiers (ocr.backend=auto|ocr-model)",
        ),
    ] {
        let present = which(tool);
        println!(
            "  [{}] {:<8} {}",
            if present { "x" } else { " " },
            tool,
            gates
        );
    }

    println!("\nmodels (~/.mari/models):");
    for spec in [
        crate::index::vector::model_spec(),
        crate::attn::model_spec(),
    ] {
        let path = crate::models::model_path(spec.file);
        let state = if path.exists() {
            format!(
                "present ({} MB)",
                std::fs::metadata(&path).map(|m| m.len() >> 20).unwrap_or(0)
            )
        } else {
            format!("missing — `mari model pull {}`", spec.kind)
        };
        println!("  {:<10} {:<40} {state}", spec.kind, spec.file);
    }

    println!("\nfeatures:");
    println!("  detector, factcheck, curation, connectors  — always available");
    println!("  semantic search                            — needs the embedding model");
    println!("  --deep / --focus / i18n coverage           — needs the attention model");
    println!("  --grammar                                  — Harper, compiled in");
    println!("  ocr.backend=auto|ocr-model                 — needs python3 + explicit opt-in");
    Ok(0)
}

fn which(tool: &str) -> bool {
    std::process::Command::new(tool)
        .arg("--version")
        .output()
        .map(|o| o.status.success() || !o.stdout.is_empty())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{catalog_status_from_paths, combined_tag_counts, embedding_line, CatalogStatus};
    use crate::index;
    use serde_json::json;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn catalog_status_aggregates_repo_and_global_catalogs() {
        let dir = tempfile::tempdir().unwrap();
        let local = dir.path().join("local").join("catalog.duckdb");
        let global = dir.path().join("global").join("catalog.duckdb");
        write_catalog(
            &local,
            &[
                ("git/doc1", "git", "git:one"),
                ("slack/doc1", "slack", "slack:one"),
            ],
            "2026-01-01T00:00:00Z",
            index::EMBEDDING_MODEL,
        );
        write_catalog(
            &global,
            &[
                ("slack/doc2", "slack", "slack:two"),
                ("gdocs/doc1", "gdocs", "gdocs:one"),
            ],
            "2026-02-01T00:00:00Z",
            "other-model",
        );

        let status = catalog_status_from_paths(&[local, global]);

        assert_eq!(status.indexed.get("git"), Some(&1));
        assert_eq!(status.indexed.get("slack"), Some(&2));
        assert_eq!(status.indexed.get("gdocs"), Some(&1));
        assert_eq!(status.last_sync.as_deref(), Some("2026-02-01T00:00:00Z"));
        assert_eq!(
            embedding_line(&status).as_deref(),
            Some("other-model, qwen3-embedding-0.6b")
        );
        assert_eq!(status.tag_counts.get("canonical"), Some(&2));
        assert_eq!(status.tag_counts.get("stale"), Some(&2));
    }

    #[test]
    fn catalog_status_collects_mirrored_tag_entry_targets() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = dir.path().join("catalog.duckdb");
        write_catalog(
            &catalog,
            &[("git/doc1", "git", "docs/api.md")],
            "2026-01-01T00:00:00Z",
            index::EMBEDDING_MODEL,
        );
        let conn = index::open_catalog_at(&catalog).unwrap();
        conn.execute(
            "UPDATE tags SET metadata_json = ?1 WHERE target_id = 'docs/api.md'",
            duckdb::params![
                json!({"source": "tags.entries", "target": "./docs/api.md"}).to_string()
            ],
        )
        .unwrap();
        index::publish_to_path(&conn, &catalog).unwrap();

        let status = catalog_status_from_paths(&[catalog]);

        assert!(status.mirrored_tag_entry_targets.contains("docs/api.md"));
    }

    #[test]
    fn embeddings_table_reads_model_rows() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = dir.path().join("catalog.duckdb");
        write_catalog(
            &catalog,
            &[("git/doc1", "git", "docs/api.md")],
            "2026-01-01T00:00:00Z",
            index::EMBEDDING_MODEL,
        );
        let conn = index::open_catalog_at(&catalog).unwrap();
        let err = conn
            .execute(
                "INSERT INTO embeddings (chunk_id, model_id, dims, vector_json, norm, embedded_at)
                 VALUES ('orphan-chunk', 'other-model', 3, '[0.1,0.2,0.3]', 1.0, 'now')",
                [],
            )
            .unwrap_err();

        assert!(err.to_string().contains("CHECK"));
        conn.execute(
            "INSERT INTO embeddings (chunk_id, model_id, dims, vector_json, norm, embedded_at)
             VALUES ('orphan-chunk', ?1, 768, '[0.1,0.2,0.3]', 1.0, 'now')",
            duckdb::params![index::EMBEDDING_MODEL],
        )
        .unwrap();
        index::publish_to_path(&conn, &catalog).unwrap();

        let status = catalog_status_from_paths(&[catalog]);

        assert_eq!(
            embedding_line(&status).as_deref(),
            Some("qwen3-embedding-0.6b")
        );
    }

    #[test]
    fn combined_tag_counts_do_not_double_count_mirrored_config_entries() {
        let cfg = json!({
            "tags": {
                "entries": {
                    "./docs/api.md": {"status": "canonical"},
                    "docs/guide.md": {"status": "needs-review"}
                }
            }
        });
        let mut tag_counts = BTreeMap::new();
        tag_counts.insert("canonical".to_string(), 1);
        let mut mirrored_tag_entry_targets = BTreeSet::new();
        mirrored_tag_entry_targets.insert("docs/api.md".to_string());
        let status = CatalogStatus {
            tag_counts,
            mirrored_tag_entry_targets,
            ..CatalogStatus::default()
        };

        let counts = combined_tag_counts(&cfg, &status);

        assert_eq!(counts.get("canonical"), Some(&1));
        assert_eq!(counts.get("needs-review"), Some(&1));
    }

    #[test]
    fn source_table_order_matches_registry_order() {
        let keys = super::source_table()
            .into_iter()
            .map(|row| row.0)
            .collect::<Vec<_>>();

        assert_eq!(
            keys,
            vec![
                "slack",
                "gdocs",
                "github",
                "confluence",
                "jira",
                "zendesk",
                "salesforce",
                "hubspot",
                "microsoft",
                "discord",
                "linear",
                "git",
                "localfiles"
            ]
        );
    }

    fn write_catalog(
        path: &std::path::Path,
        docs: &[(&str, &str, &str)],
        last_sync: &str,
        model: &str,
    ) {
        let conn = duckdb::Connection::open_in_memory().unwrap();
        index::ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO schema_meta (key, value) VALUES ('last_sync', ?1)",
            duckdb::params![last_sync],
        )
        .unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO schema_meta (key, value) VALUES ('embedding.model', ?1)",
            duckdb::params![model],
        )
        .unwrap();
        for (doc_id, source, canonical_ref) in docs {
            conn.execute(
                "INSERT INTO documents (
                    doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind,
                    author_id, author_name, created_at, updated_at, observed_at, version,
                    content_sha256, body, metadata_json
                ) VALUES (?1, ?2, ?1, ?3, ?3, '', ?3, 'text/markdown', 'doc', '', '', NULL, NULL, 'now', '1', ?1, 'body', '{}')",
                duckdb::params![doc_id, source, canonical_ref],
            )
            .unwrap();
        }
        for (idx, (_, _, canonical_ref)) in docs.iter().enumerate() {
            let status = if idx == 0 { "canonical" } else { "stale" };
            conn.execute(
                "INSERT INTO tags (target_type, target_id, status, note, \"by\", \"at\", metadata_json)
                 VALUES ('doc', ?1, ?2, '', 'test', 'now', '{}')",
                duckdb::params![canonical_ref, status],
            )
            .unwrap();
        }
        index::publish_to_path(&conn, path).unwrap();
    }
}
