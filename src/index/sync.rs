//! Sync engine for v1 local sources (SPEC §5.2/§6.0).

use super::{hash_hex, is_text_path, now, open_catalog, repo_rel, set_meta};
use crate::{authcmd, cloud, config, workspace};
use anyhow::Result;
use duckdb::{params, Connection};
use ignore::WalkBuilder;
use regex::Regex;
use serde_json::json;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub fn run(source: Option<&str>, rebuild: bool, since: Option<i64>) -> Result<i32> {
    if let Some(s) = source {
        if !known_source(s) {
            eprintln!("✗ unknown source: {s}");
            return Ok(2);
        }
    }
    // One-writer rule (§9): consumers read the replica; only the writer syncs.
    if cloud::enabled() && cloud::role() == "consumer" {
        eprintln!(
            "✗ this machine is a cloud consumer — run `mari pull` to refresh the replica,              or `mari cloud role writer` if this machine should own the shared catalog"
        );
        return Ok(1);
    }
    if rebuild {
        if let Some(msg) = cloud::forbid_rebuild() {
            eprintln!("✗ {msg}");
            return Ok(1);
        }
    }
    let started = now();
    let cutoff = since_cutoff(since);
    let mut summary = SyncSummary::default();
    let mut had_errors = false;

    let sources = sync_sources(source);
    for s in sources {
        let conn = open_catalog(source_catalog_global(s))?;
        ensure_source(&conn, s)?;
        match s {
            "git" => {
                let repos = git_paths();
                let (seen, changed, deleted, chunks) =
                    sync_paths(&conn, "git", repos.clone(), rebuild, cutoff)?;
                // Commit history: one document per commit (§6.4).
                let hist = crate::connectors::gitlog::sync(&conn, &repos, rebuild)?;
                summary.updated += changed + hist.changed;
                summary.removed += deleted + hist.deleted;
                summary.chunks_embedded += chunks + hist.chunks;
                record_event(
                    &conn,
                    "git",
                    &started,
                    "success",
                    seen + hist.seen,
                    changed + hist.changed,
                    deleted + hist.deleted,
                    None,
                )?;
            }
            "localfiles" => {
                let paths = localfile_paths();
                if paths.is_empty() {
                    eprintln!("note: no localfiles paths tracked — add one with `mari track localfiles add <path>`");
                    continue;
                }
                let (seen, changed, deleted, chunks) =
                    sync_paths(&conn, "localfiles", paths, rebuild, cutoff)?;
                summary.updated += changed;
                summary.removed += deleted;
                summary.chunks_embedded += chunks;
                record_event(
                    &conn,
                    "localfiles",
                    &started,
                    "success",
                    seen,
                    changed,
                    deleted,
                    None,
                )?;
            }
            other => {
                match crate::connectors::cloud::sync_source(&conn, other, rebuild, since) {
                    Ok(stats) => {
                        summary.updated += stats.changed;
                        summary.removed += stats.deleted;
                        summary.chunks_embedded += stats.chunks;
                        record_event(
                            &conn,
                            other,
                            &started,
                            "success",
                            stats.seen,
                            stats.changed,
                            stats.deleted,
                            None,
                        )?;
                    }
                    Err(e) => {
                        // One source's failure never aborts others (§6.0).
                        eprintln!("✗ {other} sync failed: {e:#}");
                        had_errors = true;
                        record_event(
                            &conn,
                            other,
                            &started,
                            "error",
                            0,
                            0,
                            0,
                            Some(&e.to_string()),
                        )?;
                    }
                }
            }
        }
        mirror_tags(&conn)?;
        set_meta(&conn, "last_sync", &now())?;
    }
    // Vector embedding pass (§7.1): the local embedding model over every
    // chunk missing a vector; loud on failure, never silent.
    let mut catalogs_touched: Vec<bool> = Vec::new();
    for s in sync_sources(source) {
        let g = source_catalog_global(s);
        if !catalogs_touched.contains(&g) {
            catalogs_touched.push(g);
        }
    }
    let mut embedded_vectors = 0usize;
    for g in catalogs_touched {
        match open_catalog(g).and_then(|conn| super::vector::sync_vectors(&conn, g, rebuild)) {
            Ok(n) => embedded_vectors += n,
            Err(e) => {
                eprintln!("✗ vector embedding failed: {e:#}");
                had_errors = true;
            }
        }
    }
    println!(
        "✓ {} document(s) updated, {} removed — {} chunk(s) embedded ({} vector(s)).",
        summary.updated, summary.removed, summary.chunks_embedded, embedded_vectors
    );
    let cfg = config::resolve(Some(&workspace::work_root()));
    if should_print_git_cloud_commit_nudge(
        cfg["cloud"]["enabled"].as_bool().unwrap_or(false),
        &cloud::role(),
        cfg["cloud"]["backend"].as_str().unwrap_or("s3"),
    ) {
        println!("note: commit .mari so teammates can pull the updated git-backed catalog.");
    }
    Ok(sync_exit_code(had_errors))
}

#[derive(Default)]
struct SyncSummary {
    updated: usize,
    removed: usize,
    chunks_embedded: usize,
}

fn sync_exit_code(had_errors: bool) -> i32 {
    if had_errors {
        1
    } else {
        0
    }
}

fn should_print_git_cloud_commit_nudge(enabled: bool, role: &str, backend: &str) -> bool {
    enabled && role == "writer" && backend == "git"
}

fn sync_sources(source: Option<&str>) -> Vec<&str> {
    match source {
        Some(s) => vec![s],
        None => sync_sources_for_config(
            &config::resolve(Some(&workspace::work_root())),
            |provider| authcmd::credential(provider).is_some(),
        ),
    }
}

fn known_source(source: &str) -> bool {
    source == "git" || source == "localfiles" || CLOUD_SOURCE_ORDER.contains(&source)
}

fn sync_sources_for_config<F>(cfg: &serde_json::Value, connected: F) -> Vec<&'static str>
where
    F: Fn(&str) -> bool,
{
    let mut sources = Vec::new();
    for source in CLOUD_SOURCE_ORDER {
        if source_active(cfg, source, &connected) {
            sources.push(*source);
        }
    }
    sources.push("git");
    sources.push("localfiles");
    sources
}

const CLOUD_SOURCE_ORDER: &[&str] = &[
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
];

fn source_active<F>(cfg: &serde_json::Value, source: &str, connected: &F) -> bool
where
    F: Fn(&str) -> bool,
{
    tracked_ref_count(cfg, source) > 0
        || (always_when_connected(source) && connected(auth_provider(source)))
}

fn tracked_ref_count(cfg: &serde_json::Value, source: &str) -> usize {
    list_keys_for_source(source)
        .iter()
        .filter_map(|key| config::get_path(cfg, key))
        .filter_map(|v| v.as_array())
        .map(|a| a.len())
        .sum()
}

fn list_keys_for_source(source: &str) -> &'static [&'static str] {
    match source {
        "slack" => &["slack.channels"],
        "gdocs" => &["google.docs", "google.folders"],
        "github" => &["github.repos"],
        "confluence" => &["confluence.spaces", "confluence.pages"],
        "jira" => &["jira.projects"],
        "zendesk" => &["zendesk.include"],
        "salesforce" => &["salesforce.objects"],
        "hubspot" => &["hubspot.include"],
        "microsoft" => &["microsoft.drives", "microsoft.mail", "microsoft.teams"],
        "discord" => &["discord.channels", "discord.guilds"],
        "linear" => &["linear.teams", "linear.projects"],
        "git" => &["git.repos"],
        "localfiles" => &["localfiles.paths"],
        _ => &[],
    }
}

fn always_when_connected(source: &str) -> bool {
    matches!(
        source,
        "slack" | "gdocs" | "zendesk" | "salesforce" | "hubspot" | "git"
    )
}

fn auth_provider(source: &str) -> &str {
    if source == "gdocs" {
        "google"
    } else {
        source
    }
}

fn source_catalog_global(source_id: &str) -> bool {
    source_catalog_global_with(source_id, workspace::source_scope)
}

fn source_catalog_global_with<F>(source_id: &str, scope_of: F) -> bool
where
    F: Fn(&str) -> String,
{
    scope_of(source_id) == "global"
}

fn git_paths() -> Vec<PathBuf> {
    let root = workspace::work_root();
    let cfg = config::resolve(Some(&root));
    let mut out = vec![root];
    if let Some(repos) = cfg["git"]["repos"].as_array() {
        for r in repos.iter().filter_map(|v| v.as_str()) {
            out.push(PathBuf::from(r));
        }
    }
    out
}

fn localfile_paths() -> Vec<PathBuf> {
    let root = workspace::work_root();
    let cfg = config::resolve(Some(&root));
    cfg["localfiles"]["paths"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .map(|p| {
                    let path = PathBuf::from(p);
                    if path.is_absolute() {
                        path
                    } else {
                        root.join(path)
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn sync_paths(
    conn: &Connection,
    source_id: &str,
    roots: Vec<PathBuf>,
    rebuild: bool,
    cutoff: Option<SystemTime>,
) -> Result<(usize, usize, usize, usize)> {
    if rebuild && cutoff.is_none() {
        conn.execute("DELETE FROM embeddings WHERE chunk_id IN (SELECT chunk_id FROM chunks WHERE doc_id IN (SELECT doc_id FROM documents WHERE source_id = ?1))", [source_id])?;
        conn.execute(
            "DELETE FROM symbols WHERE doc_id IN (SELECT doc_id FROM documents WHERE source_id = ?1)",
            [source_id],
        )?;
        conn.execute(
            "DELETE FROM spans WHERE doc_id IN (SELECT doc_id FROM documents WHERE source_id = ?1)",
            [source_id],
        )?;
        conn.execute("DELETE FROM chunks WHERE doc_id IN (SELECT doc_id FROM documents WHERE source_id = ?1)", [source_id])?;
        conn.execute(
            "DELETE FROM edges WHERE (from_type = 'doc' AND from_id IN (SELECT doc_id FROM documents WHERE source_id = ?1)) OR (to_type = 'doc' AND to_id IN (SELECT doc_id FROM documents WHERE source_id = ?1))",
            [source_id],
        )?;
        conn.execute("DELETE FROM documents WHERE source_id = ?1", [source_id])?;
    }
    let files = collect_files(roots, source_id == "localfiles");
    let eligible_files = eligible_files(&files, cutoff);
    let current_external_ids = files
        .iter()
        .map(|p| external_id_for(source_id, p))
        .collect::<BTreeSet<_>>();
    let deleted = if rebuild {
        0
    } else {
        prune_vanished(conn, source_id, &current_external_ids)?
    };
    let mut changed = 0usize;
    let mut chunks = 0usize;
    for path in &eligible_files {
        if let Some(written_chunks) = ingest_file(conn, source_id, path)? {
            changed += 1;
            chunks += written_chunks;
        }
    }
    rebuild_link_edges(conn, source_id)?;
    Ok((files.len(), changed, deleted, chunks))
}

fn since_cutoff(days: Option<i64>) -> Option<SystemTime> {
    let days = days?;
    let days = days.max(0);
    let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
    Some(cutoff.into())
}

fn eligible_files(files: &[PathBuf], cutoff: Option<SystemTime>) -> Vec<PathBuf> {
    let Some(cutoff) = cutoff else {
        return files.to_vec();
    };
    files
        .iter()
        .filter(|path| {
            path.metadata()
                .and_then(|m| m.modified())
                .map(|modified| modified >= cutoff)
                .unwrap_or(true)
        })
        .cloned()
        .collect()
}

fn ensure_source(conn: &Connection, source_id: &str) -> Result<()> {
    let scope = workspace::source_scope(source_id);
    let list_keys = json!(list_keys_for_source(source_id));
    let auth_provider = source_auth_provider(source_id);
    let cfg_hash = hash_hex(&config::resolve(Some(&workspace::work_root())).to_string());
    conn.execute("DELETE FROM sources WHERE source_id = ?1", [source_id])?;
    conn.execute(
        "INSERT INTO sources (source_id, provider, scope, connector_version, auth_provider, list_keys_json, config_hash, last_sync_at, last_success_at, last_error)
         VALUES (?1, ?2, ?3, 'v1', ?4, ?5, ?6, ?7, ?7, NULL)",
        params![
            source_id,
            source_id,
            scope,
            auth_provider,
            list_keys.to_string(),
            cfg_hash,
            now()
        ],
    )?;
    Ok(())
}

fn source_auth_provider(source_id: &str) -> Option<&'static str> {
    match source_id {
        "git" | "localfiles" => None,
        "gdocs" => Some("google"),
        "github" => Some("github"),
        "slack" => Some("slack"),
        "confluence" => Some("confluence"),
        "jira" => Some("jira"),
        "zendesk" => Some("zendesk"),
        "salesforce" => Some("salesforce"),
        "hubspot" => Some("hubspot"),
        "microsoft" => Some("microsoft"),
        "discord" => Some("discord"),
        "linear" => Some("linear"),
        _ => None,
    }
}

fn is_pdf(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

fn collect_files(roots: Vec<PathBuf>, include_pdf: bool) -> Vec<PathBuf> {
    // localfiles also carries PDFs and Office formats (§6.12/§8.5).
    let keep = |p: &Path| {
        is_text_path(p) || (include_pdf && (is_pdf(p) || crate::office::is_office_path(p)))
    };
    let mut out = Vec::new();
    for root in roots {
        if root.is_file() {
            if keep(&root) {
                out.push(root);
            }
            continue;
        }
        let walker = WalkBuilder::new(root)
            .hidden(false)
            .git_ignore(true)
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !(e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                    && matches!(
                        name.as_ref(),
                        ".git"
                            | ".mari"
                            | "target"
                            | "node_modules"
                            | "dist"
                            | "build"
                            | ".next"
                            | "vendor"
                    ))
            })
            .build();
        for entry in walker.flatten() {
            let path = entry.path();
            if path.is_file() && keep(path) {
                out.push(path.to_path_buf());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn ingest_file(conn: &Connection, source_id: &str, path: &Path) -> Result<Option<usize>> {
    if is_pdf(path) {
        return ingest_pdf(conn, source_id, path);
    }
    if crate::office::is_office_path(path) {
        return ingest_binary_doc(conn, source_id, path);
    }
    let Ok(body) = std::fs::read_to_string(path) else {
        return Ok(None);
    };
    let rel = repo_rel(path);
    let external_id = external_id_for(source_id, path);
    let doc_id = hash_hex(&format!("{source_id}:{external_id}"));
    let content_sha = hash_hex(&body);
    let old_sha: Option<String> = conn
        .query_row(
            "SELECT content_sha256 FROM documents WHERE doc_id = ?1",
            [&doc_id],
            |r| r.get(0),
        )
        .ok();
    if old_sha.as_deref() == Some(content_sha.as_str()) {
        return Ok(None);
    }
    let meta = std::fs::metadata(path).ok();
    let updated = meta
        .and_then(|m| m.modified().ok())
        .map(chrono::DateTime::<chrono::Utc>::from)
        .map(|t| t.to_rfc3339())
        .unwrap_or_else(now);
    let title = title_for(path, &body);
    let mime = match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html",
        "txt" | "text" => "text/plain",
        _ => "text/markdown",
    };
    conn.execute(
        "DELETE FROM embeddings WHERE chunk_id IN (SELECT chunk_id FROM chunks WHERE doc_id = ?1)",
        [&doc_id],
    )?;
    conn.execute("DELETE FROM symbols WHERE doc_id = ?1", [&doc_id])?;
    conn.execute("DELETE FROM spans WHERE doc_id = ?1", [&doc_id])?;
    conn.execute("DELETE FROM chunks WHERE doc_id = ?1", [&doc_id])?;
    conn.execute(
        "DELETE FROM edges WHERE (from_type = 'doc' AND from_id = ?1) OR (to_type = 'doc' AND to_id = ?1)",
        [&doc_id],
    )?;
    conn.execute("DELETE FROM documents WHERE doc_id = ?1", [&doc_id])?;
    conn.execute(
        "INSERT INTO documents (doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind, author_id, author_name, created_at, updated_at, observed_at, version, content_sha256, body, metadata_json)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7, 'file', NULL, NULL, NULL, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            doc_id,
            source_id,
            external_id,
            format!("{source_id}:{rel}"),
            title,
            rel,
            mime,
            updated,
            now(),
            content_sha,
            content_sha,
            body,
            json!({"extractor": super::EXTRACTOR_VERSION}).to_string(),
        ],
    )?;
    let chunks = ingest_chunks(conn, source_id, &doc_id, &body)?;
    ingest_spans_and_symbols(conn, &doc_id, &rel, &body)?;
    ingest_edges(conn, &doc_id, source_id, path, &rel)?;
    Ok(Some(chunks))
}

/// Office ingestion (SPEC §8.5): docx/odt/rtf/pptx/xlsx extracted natively;
/// raw-byte hash is the re-extract authority.
fn ingest_binary_doc(conn: &Connection, source_id: &str, path: &Path) -> Result<Option<usize>> {
    ingest_extracted(conn, source_id, path, "application/vnd.office", |p| {
        crate::office::extract(p)
    })
}

/// PDF ingestion (SPEC §8.6): extraction runs through the configured
/// Unlimited-OCR toolchain — no fallback engines. The raw-byte hash is the
/// re-extract authority, so unchanged PDFs never re-run OCR.
fn ingest_pdf(conn: &Connection, source_id: &str, path: &Path) -> Result<Option<usize>> {
    ingest_extracted(conn, source_id, path, "application/pdf", |p| {
        crate::ocr::extract_pdf(p)
    })
}

fn ingest_extracted(
    conn: &Connection,
    source_id: &str,
    path: &Path,
    mime: &str,
    extractor: impl Fn(&Path) -> Result<String>,
) -> Result<Option<usize>> {
    let Ok(bytes) = std::fs::read(path) else {
        return Ok(None);
    };
    let raw_sha = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(&bytes);
        format!("{:x}", h.finalize())
    };
    let rel = repo_rel(path);
    let external_id = external_id_for(source_id, path);
    let doc_id = hash_hex(&format!("{source_id}:{external_id}"));
    let old_sha: Option<String> = conn
        .query_row(
            "SELECT version FROM documents WHERE doc_id = ?1",
            [&doc_id],
            |r| r.get(0),
        )
        .ok();
    if old_sha.as_deref() == Some(raw_sha.as_str()) {
        return Ok(None);
    }
    let body = match extractor(path) {
        Ok(text) => text,
        Err(e) => {
            // Loud, and the file is skipped — never a different engine (§6.0).
            eprintln!("✗ {e:#}");
            return Ok(None);
        }
    };
    let meta = std::fs::metadata(path).ok();
    let updated = meta
        .and_then(|m| m.modified().ok())
        .map(chrono::DateTime::<chrono::Utc>::from)
        .map(|t| t.to_rfc3339())
        .unwrap_or_else(now);
    let title = path
        .file_stem()
        .map(|t| t.to_string_lossy().to_string())
        .unwrap_or_else(|| rel.clone());
    conn.execute(
        "DELETE FROM embeddings WHERE chunk_id IN (SELECT chunk_id FROM chunks WHERE doc_id = ?1)",
        [&doc_id],
    )?;
    conn.execute("DELETE FROM symbols WHERE doc_id = ?1", [&doc_id])?;
    conn.execute("DELETE FROM spans WHERE doc_id = ?1", [&doc_id])?;
    conn.execute("DELETE FROM chunks WHERE doc_id = ?1", [&doc_id])?;
    conn.execute(
        "DELETE FROM edges WHERE (from_type = 'doc' AND from_id = ?1) OR (to_type = 'doc' AND to_id = ?1)",
        [&doc_id],
    )?;
    conn.execute("DELETE FROM documents WHERE doc_id = ?1", [&doc_id])?;
    conn.execute(
        "INSERT INTO documents (doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind, author_id, author_name, created_at, updated_at, observed_at, version, content_sha256, body, metadata_json)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?13, 'file', NULL, NULL, NULL, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            doc_id,
            source_id,
            external_id,
            format!("{source_id}:{rel}"),
            title,
            rel,
            updated,
            now(),
            raw_sha,
            hash_hex(&body),
            body,
            json!({"extractor": "unlimited-ocr"}).to_string(),
            mime,
        ],
    )?;
    let chunks = ingest_chunks(conn, source_id, &doc_id, &body)?;
    ingest_edges(conn, &doc_id, source_id, path, &rel)?;
    Ok(Some(chunks))
}

fn external_id_for(source_id: &str, path: &Path) -> String {
    if source_id == "git" {
        repo_rel(path)
    } else {
        path.to_string_lossy().to_string()
    }
}

fn prune_vanished(
    conn: &Connection,
    source_id: &str,
    current_external_ids: &BTreeSet<String>,
) -> Result<usize> {
    let mut stmt =
        conn.prepare("SELECT doc_id, external_id FROM documents WHERE source_id = ?1")?;
    let rows = stmt.query_map([source_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut deleted = 0usize;
    for row in rows.flatten() {
        if current_external_ids.contains(&row.1) {
            continue;
        }
        delete_doc(conn, &row.0)?;
        deleted += 1;
    }
    Ok(deleted)
}

pub(crate) fn delete_doc(conn: &Connection, doc_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM embeddings WHERE chunk_id IN (SELECT chunk_id FROM chunks WHERE doc_id = ?1)",
        [doc_id],
    )?;
    conn.execute("DELETE FROM symbols WHERE doc_id = ?1", [doc_id])?;
    conn.execute("DELETE FROM spans WHERE doc_id = ?1", [doc_id])?;
    conn.execute("DELETE FROM chunks WHERE doc_id = ?1", [doc_id])?;
    conn.execute(
        "DELETE FROM edges WHERE (from_type = 'doc' AND from_id = ?1) OR (to_type = 'doc' AND to_id = ?1)",
        [doc_id],
    )?;
    conn.execute(
        "DELETE FROM tags WHERE target_type = 'doc' AND target_id = ?1",
        [doc_id],
    )?;
    conn.execute("DELETE FROM documents WHERE doc_id = ?1", [doc_id])?;
    Ok(())
}

fn ingest_edges(
    conn: &Connection,
    doc_id: &str,
    source_id: &str,
    path: &Path,
    rel: &str,
) -> Result<()> {
    if source_id == "git" || source_id == "localfiles" {
        let container = path
            .parent()
            .map(|p| repo_rel(p))
            .filter(|p| !p.is_empty() && p != ".")
            .unwrap_or_else(|| "(repo-root)".into());
        insert_edge(
            conn,
            doc_id,
            "in_repo",
            "container",
            &format!("{source_id}:{container}"),
            &json!({"path": rel, "container": container}).to_string(),
        )?;
    }
    Ok(())
}

fn insert_edge(
    conn: &Connection,
    doc_id: &str,
    rel: &str,
    to_type: &str,
    to_id: &str,
    metadata_json: &str,
) -> Result<()> {
    let edge_id = hash_hex(&format!("doc:{doc_id}:{rel}:{to_type}:{to_id}"));
    conn.execute("DELETE FROM edges WHERE edge_id = ?1", [&edge_id])?;
    conn.execute(
        "INSERT INTO edges (edge_id, from_type, from_id, to_type, to_id, rel, confidence, evidence_span_id, created_by, created_at, metadata_json)
         VALUES (?1, 'doc', ?2, ?3, ?4, ?5, 1.0, NULL, 'sync', ?6, ?7)",
        params![edge_id, doc_id, to_type, to_id, rel, now(), metadata_json],
    )?;
    Ok(())
}

fn rebuild_link_edges(conn: &Connection, source_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM edges WHERE rel = 'links_to' AND from_type = 'doc' AND from_id IN (SELECT doc_id FROM documents WHERE source_id = ?1)",
        [source_id],
    )?;
    let mut stmt = conn.prepare(
        "SELECT doc_id, path, body FROM documents WHERE source_id = ?1 AND path IS NOT NULL",
    )?;
    let docs = stmt.query_map([source_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for doc in docs.flatten() {
        for target in local_markdown_links(&doc.1, &doc.2) {
            let targets = doc_ids_for_path(conn, &target)?;
            for target_doc_id in targets {
                insert_edge(
                    conn,
                    &doc.0,
                    "links_to",
                    "doc",
                    &target_doc_id,
                    &json!({"target": target}).to_string(),
                )?;
            }
        }
    }
    Ok(())
}

fn doc_ids_for_path(conn: &Connection, path: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT doc_id FROM documents WHERE path = ?1 OR external_id = ?1 OR canonical_ref = ?1 OR canonical_ref LIKE ?2")?;
    let like = format!("%{path}");
    let rows = stmt.query_map(params![path, like], |r| r.get::<_, String>(0))?;
    Ok(rows.flatten().collect())
}

fn title_for(path: &Path, body: &str) -> String {
    body.lines()
        .find_map(|l| l.strip_prefix("# ").map(|h| h.trim().to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "document".into())
        })
}

pub(crate) fn ingest_chunks(
    conn: &Connection,
    source_id: &str,
    doc_id: &str,
    body: &str,
) -> Result<usize> {
    let cfg = config::resolve(Some(&workspace::work_root()));
    ingest_chunks_with_config(conn, source_id, doc_id, body, &cfg)
}

fn ingest_chunks_with_config(
    conn: &Connection,
    source_id: &str,
    doc_id: &str,
    body: &str,
    cfg: &serde_json::Value,
) -> Result<usize> {
    let chunk_cfg = chunking_config(cfg, source_id);
    let lines_per = chunk_cfg["lines"].as_u64().unwrap_or(40).max(1) as usize;
    let overlap =
        (chunk_cfg["overlap"].as_u64().unwrap_or(8) as usize).min(lines_per.saturating_sub(1));
    let min_chars = chunk_cfg["min_chars"].as_u64().unwrap_or(40) as usize;
    let max_chars = chunk_cfg["max_chars"].as_u64().unwrap_or(2000).max(1) as usize;
    let write_large = chunk_cfg["large_chunks"].as_bool().unwrap_or(false);
    let large_ratio = chunk_cfg["large_chunk_ratio"].as_u64().unwrap_or(4).max(1) as usize;
    let starts = line_offsets(body);
    let line_count = starts.len();
    let mut idx = 0usize;
    let mut written = 0usize;
    let mut line = 0usize;
    let mut base_chunks = Vec::new();
    while line < line_count {
        let end_line_excl = (line + lines_per).min(line_count);
        let start_byte = starts[line];
        let window_end_byte = starts.get(end_line_excl).copied().unwrap_or(body.len());
        let end_byte = cap_utf8_boundary(body, start_byte, window_end_byte, max_chars);
        let text = body[start_byte..end_byte].to_string();
        if text.trim().len() < min_chars {
            if end_line_excl == line_count {
                break;
            }
            line = end_line_excl.saturating_sub(overlap);
            continue;
        }
        let chunk_id = stable_chunk_id(source_id, doc_id, line + 1);
        let heading = heading_for(body, start_byte);
        let end_line = byte_to_line(starts.as_slice(), end_byte);
        conn.execute(
            "INSERT INTO chunks (chunk_id, doc_id, chunk_index, heading_path, section_anchor, start_byte, end_byte, start_line, end_line, token_count, text, text_sha256, metadata_json)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                chunk_id,
                doc_id,
                idx as i64,
                heading,
                start_byte as i64,
                end_byte as i64,
                (line + 1) as i64,
                end_line as i64,
                crate::detector::ctx::count_words(&text) as i64,
                text,
                hash_hex(&body[start_byte..end_byte]),
                json!({"chunking": super::CHUNKING_VERSION, "large": false}).to_string(),
            ],
        )?;
        written += 1;
        base_chunks.push(BaseChunk {
            chunk_id: stable_chunk_id(source_id, doc_id, line + 1),
            heading,
            start_byte,
            end_byte,
            start_line: line + 1,
            end_line,
        });
        idx += 1;
        if end_line_excl == line_count {
            break;
        }
        line = end_line_excl.saturating_sub(overlap);
    }
    if write_large {
        written += write_large_chunks(
            conn,
            source_id,
            doc_id,
            body,
            &base_chunks,
            idx,
            large_ratio,
        )?;
    }
    Ok(written)
}

fn chunking_config(cfg: &serde_json::Value, source_id: &str) -> serde_json::Value {
    let mut chunk_cfg = cfg["chunking"].clone();
    if let Some(source_chunking) = cfg[source_id]["chunking"].as_object() {
        config::deep_merge(
            &mut chunk_cfg,
            &serde_json::Value::Object(source_chunking.clone()),
            false,
        );
    }
    chunk_cfg
}

#[derive(Debug, Clone)]
struct BaseChunk {
    chunk_id: String,
    heading: String,
    start_byte: usize,
    end_byte: usize,
    start_line: usize,
    end_line: usize,
}

fn write_large_chunks(
    conn: &Connection,
    source_id: &str,
    doc_id: &str,
    body: &str,
    base_chunks: &[BaseChunk],
    mut chunk_index: usize,
    large_ratio: usize,
) -> Result<usize> {
    let mut written = 0usize;
    for group in base_chunks.chunks(large_ratio) {
        let Some(first) = group.first() else {
            continue;
        };
        let Some(last) = group.last() else {
            continue;
        };
        if group.len() < 2 {
            continue;
        }
        let start_byte = first.start_byte;
        let end_byte = last.end_byte;
        let text = body[start_byte..end_byte].to_string();
        let chunk_id = format!(
            "{}+large{}",
            stable_chunk_id(source_id, doc_id, first.start_line),
            group.len()
        );
        let base_ids = group
            .iter()
            .map(|chunk| chunk.chunk_id.as_str())
            .collect::<Vec<_>>();
        conn.execute(
            "INSERT INTO chunks (chunk_id, doc_id, chunk_index, heading_path, section_anchor, start_byte, end_byte, start_line, end_line, token_count, text, text_sha256, metadata_json)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                chunk_id,
                doc_id,
                chunk_index as i64,
                first.heading,
                start_byte as i64,
                end_byte as i64,
                first.start_line as i64,
                last.end_line as i64,
                crate::detector::ctx::count_words(&text) as i64,
                text,
                hash_hex(&body[start_byte..end_byte]),
                json!({
                    "chunking": super::CHUNKING_VERSION,
                    "large": true,
                    "base_chunk_ids": base_ids
                })
                .to_string(),
            ],
        )?;
        written += 1;
        chunk_index += 1;
    }
    Ok(written)
}

fn ingest_spans_and_symbols(conn: &Connection, doc_id: &str, rel: &str, body: &str) -> Result<()> {
    let starts = line_offsets(body);
    for span in navigation_spans(rel, body) {
        let span_id = stable_span_id(
            doc_id,
            &span.kind,
            span.start_byte,
            span.end_byte,
            &span.label,
        );
        let chunk_id = chunk_for_span(conn, doc_id, span.start_byte, span.end_byte)?;
        conn.execute(
            "INSERT INTO spans (span_id, doc_id, chunk_id, span_kind, label, start_byte, end_byte, start_line, end_line, stable_hash, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                span_id,
                doc_id,
                chunk_id,
                span.kind,
                span.label,
                span.start_byte as i64,
                span.end_byte as i64,
                byte_to_line(&starts, span.start_byte) as i64,
                byte_to_line(&starts, span.end_byte) as i64,
                hash_hex(&body[span.start_byte..span.end_byte]),
                span.metadata_json,
            ],
        )?;
    }
    for symbol in navigation_symbols(rel, body) {
        let symbol_id = hash_hex(&format!(
            "{doc_id}:{}:{}:{}:{}",
            symbol.kind, symbol.name, symbol.start_byte, symbol.end_byte
        ));
        let span_id = matching_span_id(conn, doc_id, symbol.start_byte, symbol.end_byte)?;
        conn.execute(
            "INSERT INTO symbols (symbol_id, doc_id, span_id, language, symbol_kind, name, qualified_name, signature, start_byte, end_byte, start_line, end_line, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                symbol_id,
                doc_id,
                span_id,
                symbol.language,
                symbol.kind,
                symbol.name,
                symbol.qualified_name,
                symbol.signature,
                symbol.start_byte as i64,
                symbol.end_byte as i64,
                byte_to_line(&starts, symbol.start_byte) as i64,
                byte_to_line(&starts, symbol.end_byte) as i64,
                symbol.metadata_json,
            ],
        )?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct NavSpan {
    kind: String,
    label: Option<String>,
    start_byte: usize,
    end_byte: usize,
    metadata_json: String,
}

#[derive(Debug, Clone)]
struct NavSymbol {
    language: Option<String>,
    kind: String,
    name: String,
    qualified_name: String,
    signature: Option<String>,
    start_byte: usize,
    end_byte: usize,
    metadata_json: String,
}

fn navigation_spans(rel: &str, body: &str) -> Vec<NavSpan> {
    let mut spans = Vec::new();
    let mut paragraph_start: Option<usize> = None;
    for (line_start, line) in lines_with_offsets(body) {
        let line_end = line_start + line.len();
        let trimmed = line.trim();
        if let Some((level, label)) = markdown_heading(trimmed) {
            flush_paragraph(body, &mut spans, &mut paragraph_start, line_start);
            spans.push(NavSpan {
                kind: "heading".into(),
                label: Some(label.to_string()),
                start_byte: line_start,
                end_byte: line_end,
                metadata_json: json!({"level": level, "path": rel}).to_string(),
            });
        } else if trimmed.is_empty() {
            flush_paragraph(body, &mut spans, &mut paragraph_start, line_start);
        } else if paragraph_start.is_none() {
            paragraph_start = Some(line_start);
        }
        for link in markdown_links_in_line(line, line_start) {
            spans.push(NavSpan {
                kind: "link".into(),
                label: Some(link.target.clone()),
                start_byte: link.start_byte,
                end_byte: link.end_byte,
                metadata_json: json!({"target": link.target, "path": rel}).to_string(),
            });
        }
    }
    flush_paragraph(body, &mut spans, &mut paragraph_start, body.len());
    spans
}

fn navigation_symbols(rel: &str, body: &str) -> Vec<NavSymbol> {
    let ext = Path::new(rel)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "md" | "mdx" | "markdown" | "mdc" => markdown_symbols(rel, body),
        "toml" | "yaml" | "yml" | "json" => config_symbols(rel, body),
        "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" => code_symbols(rel, &ext, body),
        _ => Vec::new(),
    }
}

fn markdown_symbols(rel: &str, body: &str) -> Vec<NavSymbol> {
    let mut out = Vec::new();
    for (line_start, line) in lines_with_offsets(body) {
        let trimmed = line.trim();
        if let Some((_, label)) = markdown_heading(trimmed) {
            out.push(NavSymbol {
                language: Some("markdown".into()),
                kind: "heading".into(),
                name: label.to_string(),
                qualified_name: format!("{rel}#{label}"),
                signature: Some(trimmed.to_string()),
                start_byte: line_start,
                end_byte: line_start + line.len(),
                metadata_json: json!({"path": rel}).to_string(),
            });
        }
        for command in command_spans_in_line(line, line_start) {
            out.push(NavSymbol {
                language: Some("shell".into()),
                kind: "command".into(),
                name: command_name(&command.text).to_string(),
                qualified_name: format!("{rel}:{}", command.text),
                signature: Some(command.text),
                start_byte: command.start_byte,
                end_byte: command.end_byte,
                metadata_json: json!({"path": rel}).to_string(),
            });
        }
    }
    out
}

fn config_symbols(rel: &str, body: &str) -> Vec<NavSymbol> {
    let ext = Path::new(rel)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "toml" => toml_config_symbols(rel, body),
        "yaml" | "yml" => yaml_config_symbols(rel, body),
        "json" => json_config_symbols(rel, body),
        _ => flat_config_symbols(rel, body),
    }
}

fn flat_config_symbols(rel: &str, body: &str) -> Vec<NavSymbol> {
    let re = Regex::new(r#"^\s*["']?([A-Za-z_][A-Za-z0-9_.-]*)["']?\s*[:=]"#).unwrap();
    lines_with_offsets(body)
        .into_iter()
        .filter_map(|(line_start, line)| {
            let caps = re.captures(line)?;
            let name = caps.get(1)?.as_str();
            Some(config_symbol(rel, name, line_start, line))
        })
        .collect()
}

fn toml_config_symbols(rel: &str, body: &str) -> Vec<NavSymbol> {
    let section_re = Regex::new(r#"^\s*\[\[?\s*([A-Za-z0-9_.-]+)\s*\]?\]\s*$"#).unwrap();
    let key_re = Regex::new(r#"^\s*([A-Za-z_][A-Za-z0-9_-]*)\s*="#).unwrap();
    let mut section: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for (line_start, line) in lines_with_offsets(body) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(caps) = section_re.captures(trimmed) {
            section = caps
                .get(1)
                .unwrap()
                .as_str()
                .split('.')
                .map(str::to_string)
                .collect();
            out.push(config_symbol(rel, &section.join("."), line_start, line));
            continue;
        }
        if let Some(caps) = key_re.captures(trimmed) {
            let name = dotted_config_name(&section, caps.get(1).unwrap().as_str());
            out.push(config_symbol(rel, &name, line_start, line));
        }
    }
    out
}

fn yaml_config_symbols(rel: &str, body: &str) -> Vec<NavSymbol> {
    let key_re = Regex::new(r#"^(\s*)([A-Za-z_][A-Za-z0-9_.-]*)\s*:"#).unwrap();
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut out = Vec::new();
    for (line_start, line) in lines_with_offsets(body) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
            continue;
        }
        let Some(caps) = key_re.captures(line) else {
            continue;
        };
        let indent = caps.get(1).unwrap().as_str().chars().count();
        let key = caps.get(2).unwrap().as_str().to_string();
        while stack.last().map(|(i, _)| *i >= indent).unwrap_or(false) {
            stack.pop();
        }
        let mut parts: Vec<String> = stack.iter().map(|(_, key)| key.clone()).collect();
        parts.push(key.clone());
        out.push(config_symbol(rel, &parts.join("."), line_start, line));
        if config_value_after_colon(trimmed).is_empty() {
            stack.push((indent, key));
        }
    }
    out
}

fn json_config_symbols(rel: &str, body: &str) -> Vec<NavSymbol> {
    let key_re = Regex::new(r#"^(\s*)"([^"]+)"\s*:"#).unwrap();
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut out = Vec::new();
    for (line_start, line) in lines_with_offsets(body) {
        let Some(caps) = key_re.captures(line) else {
            continue;
        };
        let indent = caps.get(1).unwrap().as_str().chars().count();
        let key = caps.get(2).unwrap().as_str().to_string();
        while stack.last().map(|(i, _)| *i >= indent).unwrap_or(false) {
            stack.pop();
        }
        let mut parts: Vec<String> = stack.iter().map(|(_, key)| key.clone()).collect();
        parts.push(key.clone());
        out.push(config_symbol(rel, &parts.join("."), line_start, line));
        let value = config_value_after_colon(line.trim());
        if value.starts_with('{') || value.starts_with('[') {
            stack.push((indent, key));
        }
    }
    out
}

fn config_symbol(rel: &str, name: &str, line_start: usize, line: &str) -> NavSymbol {
    NavSymbol {
        language: None,
        kind: "config_key".into(),
        qualified_name: format!("{rel}:{name}"),
        name: name.to_string(),
        signature: Some(line.trim().to_string()),
        start_byte: line_start,
        end_byte: line_start + line.len(),
        metadata_json: json!({"path": rel}).to_string(),
    }
}

fn dotted_config_name(prefix: &[String], key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{}.{}", prefix.join("."), key)
    }
}

fn config_value_after_colon(line: &str) -> &str {
    line.split_once(':').map(|(_, v)| v.trim()).unwrap_or("")
}

fn code_symbols(rel: &str, ext: &str, body: &str) -> Vec<NavSymbol> {
    let (language, patterns): (&str, &[&str]) = match ext {
        "rs" => (
            "rust",
            &[
                r"^\s*pub(?:\([^)]*\))?\s+(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)",
                r"^\s*pub(?:\([^)]*\))?\s+(?:struct|enum|trait|type|const|static|mod)\s+([A-Za-z_][A-Za-z0-9_]*)",
            ],
        ),
        "ts" | "tsx" | "js" | "jsx" => (
            "js-ts",
            &[
                r"^\s*export\s+(?:default\s+)?(?:async\s+)?function\s+([A-Za-z_$][A-Za-z0-9_$]*)",
                r"^\s*export\s+(?:default\s+)?(?:const|let|var|class|interface|type|enum)\s+([A-Za-z_$][A-Za-z0-9_$]*)",
            ],
        ),
        "py" => (
            "python",
            &[
                r"^\s*(?:async\s+)?def\s+([A-Za-z_][A-Za-z0-9_]*)",
                r"^\s*class\s+([A-Za-z_][A-Za-z0-9_]*)",
            ],
        ),
        "go" => (
            "go",
            &[
                r"^\s*func\s+([A-Z][A-Za-z0-9_]*)",
                r"^\s*func\s+\([^)]*\)\s*([A-Z][A-Za-z0-9_]*)",
                r"^\s*type\s+([A-Z][A-Za-z0-9_]*)",
                r"^\s*(?:const|var)\s+([A-Z][A-Za-z0-9_]*)",
            ],
        ),
        _ => return Vec::new(),
    };
    let regexes = patterns
        .iter()
        .map(|p| Regex::new(p).unwrap())
        .collect::<Vec<_>>();
    let mut out = Vec::new();
    for (line_start, line) in lines_with_offsets(body) {
        for re in &regexes {
            if let Some(caps) = re.captures(line) {
                let name = caps.get(1).unwrap().as_str().to_string();
                out.push(NavSymbol {
                    language: Some(language.into()),
                    kind: "code_symbol".into(),
                    qualified_name: format!("{rel}:{name}"),
                    name,
                    signature: Some(line.trim().to_string()),
                    start_byte: line_start,
                    end_byte: line_start + line.len(),
                    metadata_json: json!({"path": rel}).to_string(),
                });
            }
        }
    }
    out
}

fn flush_paragraph(
    body: &str,
    spans: &mut Vec<NavSpan>,
    paragraph_start: &mut Option<usize>,
    end_byte: usize,
) {
    let Some(start) = paragraph_start.take() else {
        return;
    };
    let text = body[start..end_byte].trim();
    if text.is_empty() {
        return;
    }
    spans.push(NavSpan {
        kind: "paragraph".into(),
        label: None,
        start_byte: start,
        end_byte,
        metadata_json: "{}".into(),
    });
}

fn stable_span_id(
    doc_id: &str,
    kind: &str,
    start_byte: usize,
    end_byte: usize,
    label: &Option<String>,
) -> String {
    hash_hex(&format!(
        "{doc_id}:{kind}:{start_byte}:{end_byte}:{}",
        label.as_deref().unwrap_or("")
    ))
}

fn chunk_for_span(
    conn: &Connection,
    doc_id: &str,
    start_byte: usize,
    end_byte: usize,
) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT chunk_id FROM chunks
              WHERE doc_id = ?1 AND start_byte <= ?2 AND end_byte >= ?3
              ORDER BY chunk_index LIMIT 1",
            params![doc_id, start_byte as i64, end_byte as i64],
            |row| row.get(0),
        )
        .ok())
}

fn matching_span_id(
    conn: &Connection,
    doc_id: &str,
    start_byte: usize,
    end_byte: usize,
) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT span_id FROM spans
              WHERE doc_id = ?1 AND start_byte <= ?2 AND end_byte >= ?3
              ORDER BY (end_byte - start_byte) ASC LIMIT 1",
            params![doc_id, start_byte as i64, end_byte as i64],
            |row| row.get(0),
        )
        .ok())
}

fn stable_chunk_id(source_id: &str, doc_id: &str, start_line: usize) -> String {
    format!("{source_id}/{doc_id}#L{start_line}")
}

fn cap_utf8_boundary(body: &str, start: usize, window_end: usize, max_chars: usize) -> usize {
    let capped = start.saturating_add(max_chars).min(window_end);
    if capped == window_end || body.is_char_boundary(capped) {
        return capped;
    }
    let mut end = capped;
    while end > start && !body.is_char_boundary(end) {
        end -= 1;
    }
    end
}

fn byte_to_line(starts: &[usize], byte: usize) -> usize {
    match starts.binary_search(&byte) {
        Ok(idx) => idx.max(1),
        Err(idx) => idx.max(1),
    }
}

#[derive(Debug, Clone)]
struct LinkTarget {
    target: String,
    start_byte: usize,
    end_byte: usize,
}

#[derive(Debug, Clone)]
struct CommandSpan {
    text: String,
    start_byte: usize,
    end_byte: usize,
}

fn lines_with_offsets(body: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    for line in body.split_inclusive('\n') {
        let trimmed = line.strip_suffix('\n').unwrap_or(line);
        out.push((offset, trimmed));
        offset += line.len();
    }
    if body.is_empty() {
        out.push((0, ""));
    } else if !body.ends_with('\n') && out.is_empty() {
        out.push((0, body));
    }
    out
}

fn markdown_heading(trimmed: &str) -> Option<(usize, &str)> {
    let level = trimmed.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) || trimmed.chars().nth(level) != Some(' ') {
        return None;
    }
    let label = trimmed[level..].trim().trim_matches('#').trim();
    (!label.is_empty()).then_some((level, label))
}

fn markdown_links_in_line(line: &str, line_start: usize) -> Vec<LinkTarget> {
    let mut out = Vec::new();
    let mut search_from = 0usize;
    while let Some(close_rel) = line[search_from..].find("](") {
        let close = search_from + close_rel;
        let Some(open_rel) = line[..close].rfind('[') else {
            search_from = close + 2;
            continue;
        };
        if open_rel > 0 && line.as_bytes().get(open_rel - 1) == Some(&b'!') {
            search_from = close + 2;
            continue;
        }
        let target_start = close + 2;
        let Some(end_rel) = line[target_start..].find(')') else {
            break;
        };
        let target_end = target_start + end_rel;
        let target = line[target_start..target_end].trim();
        if !target.is_empty() {
            out.push(LinkTarget {
                target: target.to_string(),
                start_byte: line_start + open_rel,
                end_byte: line_start + target_end + 1,
            });
        }
        search_from = target_end + 1;
    }
    out
}

fn local_markdown_links(rel: &str, body: &str) -> Vec<String> {
    let base = Path::new(rel).parent().unwrap_or_else(|| Path::new(""));
    let mut out = Vec::new();
    for (_, line) in lines_with_offsets(body) {
        for link in markdown_links_in_line(line, 0) {
            if is_external_link(&link.target) {
                continue;
            }
            let (path, _) = split_link_target(&link.target);
            if path.is_empty() {
                continue;
            }
            let normalized = if Path::new(path).is_absolute() {
                path.trim_start_matches('/').to_string()
            } else {
                base.join(path).display().to_string()
            };
            out.push(normalized);
        }
    }
    out.sort();
    out.dedup();
    out
}

fn command_spans_in_line(line: &str, line_start: usize) -> Vec<CommandSpan> {
    let mut out = Vec::new();
    let mut rest = line;
    let mut base = 0usize;
    while let Some(start) = rest.find('`') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('`') else {
            break;
        };
        let raw = &after_start[..end];
        let text = raw.trim();
        if is_command_like(text) {
            let leading_ws = raw.len() - raw.trim_start().len();
            let span_start = line_start + base + start + 1 + leading_ws;
            out.push(CommandSpan {
                text: text.to_string(),
                start_byte: span_start,
                end_byte: span_start + text.len(),
            });
        }
        let consumed = start + 1 + end + 1;
        base += consumed;
        rest = &rest[consumed..];
    }
    let trimmed = line.trim_start();
    let leading = line.len() - trimmed.len();
    let command = trimmed.strip_prefix("$ ").unwrap_or(trimmed);
    if is_command_like(command) {
        let prefix = if trimmed.starts_with("$ ") { 2 } else { 0 };
        out.push(CommandSpan {
            text: command.to_string(),
            start_byte: line_start + leading + prefix,
            end_byte: line_start + leading + prefix + command.len(),
        });
    }
    out
}

fn command_name(command: &str) -> &str {
    command.split_whitespace().next().unwrap_or(command)
}

fn is_command_like(command: &str) -> bool {
    let raw_name = command_name(command.trim());
    let name = raw_name.strip_prefix("./").unwrap_or(raw_name);
    matches!(
        name,
        "mari"
            | "cargo"
            | "git"
            | "gh"
            | "npm"
            | "pnpm"
            | "yarn"
            | "make"
            | "docker"
            | "kubectl"
            | "python"
            | "python3"
            | "node"
    ) || raw_name.starts_with("./")
}

fn split_link_target(link: &str) -> (&str, Option<&str>) {
    let link = link.split('?').next().unwrap_or(link);
    match link.split_once('#') {
        Some((path, anchor)) => (path, Some(anchor)),
        None => (link, None),
    }
}

fn is_external_link(link: &str) -> bool {
    let lower = link.to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with("tel:")
}

fn line_offsets(body: &str) -> Vec<usize> {
    let mut out = vec![0usize];
    for (i, b) in body.bytes().enumerate() {
        if b == b'\n' {
            out.push(i + 1);
        }
    }
    if out.last().copied() == Some(body.len()) && body.ends_with('\n') {
        out.pop();
    }
    out
}

fn heading_for(body: &str, upto: usize) -> String {
    let mut headings = Vec::new();
    for line in body[..upto.min(body.len())].lines() {
        let trimmed = line.trim_start();
        let level = trimmed.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&level) && trimmed.chars().nth(level) == Some(' ') {
            headings.truncate(level - 1);
            headings.push(trimmed[level..].trim().trim_matches('#').trim().to_string());
        }
    }
    if headings.is_empty() {
        "(root)".into()
    } else {
        headings.join(" > ")
    }
}

fn record_event(
    conn: &Connection,
    source_id: &str,
    started: &str,
    status: &str,
    seen: usize,
    changed: usize,
    deleted: usize,
    error: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO sync_events (event_id, source_id, started_at, finished_at, status, docs_seen, docs_changed, docs_deleted, error, metadata_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '{}')",
        params![
            hash_hex(&format!("{source_id}:{started}:{}", now())),
            source_id,
            started,
            now(),
            status,
            seen as i64,
            changed as i64,
            deleted as i64,
            error,
        ],
    )?;
    Ok(())
}

fn mirror_tags(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM tags", [])?;
    let root = workspace::work_root();
    let cfg = config::resolve(Some(&root));
    let Some(entries) = cfg["tags"]["entries"].as_object() else {
        return Ok(());
    };
    for (target, entry) in entries {
        let Some(status) = entry["status"].as_str() else {
            continue;
        };
        let note = entry["note"].as_str().unwrap_or("");
        let by = entry["by"].as_str().unwrap_or("unknown");
        let at = entry["at"].as_str().unwrap_or("");
        for doc_id in doc_ids_for_tag_target(conn, target)? {
            conn.execute(
                "DELETE FROM tags WHERE target_type = 'doc' AND target_id = ?1",
                [&doc_id],
            )?;
            conn.execute(
                "INSERT INTO tags (target_type, target_id, status, note, \"by\", \"at\", metadata_json)
                 VALUES ('doc', ?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    doc_id,
                    status,
                    note,
                    by,
                    at,
                    json!({"source": "tags.entries", "target": target}).to_string()
                ],
            )?;
        }
    }
    Ok(())
}

fn doc_ids_for_tag_target(conn: &Connection, target: &str) -> Result<Vec<String>> {
    let norm = target.strip_prefix("./").unwrap_or(target);
    let mut stmt = conn.prepare(
        "SELECT doc_id FROM documents
          WHERE canonical_ref = ?1 OR path = ?1 OR external_id = ?1 OR canonical_ref LIKE ?2",
    )?;
    let like = format!("%{norm}");
    let rows = stmt.query_map(params![norm, like], |r| r.get::<_, String>(0))?;
    Ok(rows.flatten().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_chunk_ids_use_source_doc_and_start_line() {
        assert_eq!(
            stable_chunk_id("localfiles", "abc123", 41),
            "localfiles/abc123#L41"
        );
    }

    #[test]
    fn source_catalog_scope_selects_global_only_for_global_scope() {
        assert!(source_catalog_global_with("slack", |_| "global".into()));
        assert!(!source_catalog_global_with("git", |_| "local".into()));
    }

    #[test]
    fn default_sync_sources_include_tracked_connectors_before_git_and_localfiles() {
        let mut cfg = config::defaults();
        config::set_path(&mut cfg, "github.repos", json!(["acme/project"]));
        config::set_path(&mut cfg, "discord.channels", json!(["C123"]));

        let sources = sync_sources_for_config(&cfg, |_| false);

        assert_eq!(sources, vec!["github", "discord", "git", "localfiles"]);
    }

    #[test]
    fn always_connected_sources_are_active_when_authenticated() {
        let cfg = config::defaults();
        let sources = sync_sources_for_config(&cfg, |provider| provider == "google");

        assert_eq!(sources, vec!["gdocs", "git", "localfiles"]);
    }

    #[test]
    fn known_source_rejects_unknown_sync_source_keys() {
        assert!(known_source("gdocs"));
        assert!(known_source("git"));
        assert!(known_source("localfiles"));
        assert!(!known_source("sqlite"));
    }

    #[test]
    fn ensure_source_records_cloud_list_keys_and_auth_provider() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();

        ensure_source(&conn, "gdocs").unwrap();

        let row: (String, String) = conn
            .query_row(
                "SELECT COALESCE(auth_provider, ''), list_keys_json FROM sources WHERE source_id = 'gdocs'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(row.0, "google");
        assert_eq!(row.1, json!(["google.docs", "google.folders"]).to_string());
    }

    #[test]
    fn source_metadata_covers_multi_list_sources() {
        assert_eq!(
            list_keys_for_source("microsoft"),
            &["microsoft.drives", "microsoft.mail", "microsoft.teams"]
        );
        assert_eq!(source_auth_provider("localfiles"), None);
        assert_eq!(source_auth_provider("microsoft"), Some("microsoft"));
    }

    #[test]
    fn sync_exit_code_reflects_source_errors() {
        assert_eq!(sync_exit_code(false), 0);
        assert_eq!(sync_exit_code(true), 1);
    }

    #[test]
    fn git_cloud_writer_gets_commit_nudge() {
        assert!(should_print_git_cloud_commit_nudge(true, "writer", "git"));
        assert!(!should_print_git_cloud_commit_nudge(
            true, "consumer", "git"
        ));
        assert!(!should_print_git_cloud_commit_nudge(true, "writer", "s3"));
        assert!(!should_print_git_cloud_commit_nudge(false, "writer", "git"));
    }

    #[test]
    fn since_cutoff_treats_negative_days_as_today() {
        assert!(since_cutoff(Some(-4)).is_some());
    }

    #[test]
    fn eligible_files_respects_since_cutoff() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("fresh.md");
        std::fs::write(&file, "# Fresh\n").unwrap();

        let future_cutoff = SystemTime::now() + std::time::Duration::from_secs(60);
        let all = vec![file.clone()];
        assert!(eligible_files(&all, None).contains(&file));
        assert!(eligible_files(&all, Some(future_cutoff)).is_empty());
    }

    #[test]
    fn max_char_cap_preserves_utf8_boundaries() {
        let text = "abcéfg\nnext";
        let capped = cap_utf8_boundary(text, 0, text.len(), 4);
        assert_eq!(&text[..capped], "abc");
    }

    #[test]
    fn ingest_chunks_writes_stable_line_chunk_id() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO documents (doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind, author_id, author_name, created_at, updated_at, observed_at, version, content_sha256, body, metadata_json)
             VALUES ('doc1', 'localfiles', 'docs/a.md', 'localfiles:docs/a.md', 'A', NULL, 'docs/a.md', 'text/markdown', 'file', NULL, NULL, NULL, NULL, 'now', 'v', 'sha', '# A', '{}')",
            [],
        )
        .unwrap();
        let body = "# A\n\nThis paragraph is long enough to pass the default minimum chunk size.\n";
        let written = ingest_chunks(&conn, "localfiles", "doc1", body).unwrap();
        let chunk_id: String = conn
            .query_row("SELECT chunk_id FROM chunks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(written, 1);
        assert_eq!(chunk_id, "localfiles/doc1#L1");
    }

    #[test]
    fn ingest_chunks_can_write_large_vector_only_chunks() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO documents (doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind, author_id, author_name, created_at, updated_at, observed_at, version, content_sha256, body, metadata_json)
             VALUES ('doc1', 'localfiles', 'docs/a.md', 'localfiles:docs/a.md', 'A', NULL, 'docs/a.md', 'text/markdown', 'file', NULL, NULL, NULL, NULL, 'now', 'v', 'sha', '# A', '{}')",
            [],
        )
        .unwrap();
        let cfg = json!({
            "chunking": {
                "lines": 2,
                "overlap": 0,
                "min_chars": 1,
                "max_chars": 4000,
                "large_chunks": true,
                "large_chunk_ratio": 2
            }
        });
        let body = "one alpha\none beta\n\ntwo alpha\ntwo beta\n\nthree alpha\nthree beta\n";
        let written = ingest_chunks_with_config(&conn, "localfiles", "doc1", body, &cfg).unwrap();

        let base_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chunks WHERE metadata_json LIKE '%\"large\":false%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let large_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chunks WHERE metadata_json LIKE '%\"large\":true%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(base_count >= 3);
        assert!(large_count >= 1);
        assert_eq!(written as i64, base_count + large_count);
    }

    #[test]
    fn ingest_chunks_merges_source_specific_chunking() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO documents (doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind, author_id, author_name, created_at, updated_at, observed_at, version, content_sha256, body, metadata_json)
             VALUES ('doc1', 'git', 'docs/a.md', 'git:docs/a.md', 'A', NULL, 'docs/a.md', 'text/markdown', 'file', NULL, NULL, NULL, NULL, 'now', 'v', 'sha', '# A', '{}')",
            [],
        )
        .unwrap();
        let cfg = json!({
            "chunking": {
                "lines": 10,
                "overlap": 0,
                "min_chars": 1,
                "max_chars": 4000,
                "large_chunks": false
            },
            "git": {
                "chunking": {
                    "lines": 2
                }
            }
        });
        let body = "one\n\ntwo\n\nthree\n\nfour\n";
        let written = ingest_chunks_with_config(&conn, "git", "doc1", body, &cfg).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
            .unwrap();
        assert!(count > 1);
        assert_eq!(written as i64, count);
    }

    #[test]
    fn navigation_extracts_spans_and_symbols() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO documents (doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind, author_id, author_name, created_at, updated_at, observed_at, version, content_sha256, body, metadata_json)
             VALUES ('doc1', 'localfiles', 'docs/a.md', 'localfiles:docs/a.md', 'A', NULL, 'docs/a.md', 'text/markdown', 'file', NULL, NULL, NULL, NULL, 'now', 'v', 'sha', '# A', '{}')",
            [],
        )
        .unwrap();
        let body = "# Title\n\nParagraph with [link](b.md) and command `mari check --strict`.\n";
        ingest_chunks(&conn, "localfiles", "doc1", body).unwrap();
        ingest_spans_and_symbols(&conn, "doc1", "docs/a.md", body).unwrap();

        let headings: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM spans WHERE span_kind = 'heading'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let links: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM spans WHERE span_kind = 'link'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let commands: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM symbols WHERE symbol_kind = 'command'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(headings, 1);
        assert_eq!(links, 1);
        assert_eq!(commands, 1);
    }

    #[test]
    fn navigation_symbols_include_public_code_forms() {
        let rust = navigation_symbols("src/lib.rs", "pub(crate) mod api;\npub async fn run() {}\n");
        assert_eq!(
            rust.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["api", "run"]
        );

        let ts = navigation_symbols(
            "src/api.ts",
            "export default function createApp() {}\nexport enum Mode {}\n",
        );
        assert_eq!(
            ts.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["createApp", "Mode"]
        );

        let go = navigation_symbols(
            "main.go",
            "func (s *Server) Listen() {}\nvar DefaultPort = 8080\n",
        );
        assert_eq!(
            go.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["Listen", "DefaultPort"]
        );
    }

    #[test]
    fn navigation_symbols_include_dotted_config_keys() {
        let toml = navigation_symbols(
            "config.toml",
            "[tool.mari]\nstrict = true\n\n[[tool.mari.rules]]\nname = \"docs\"\n",
        );
        assert_eq!(
            toml.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec![
                "tool.mari",
                "tool.mari.strict",
                "tool.mari.rules",
                "tool.mari.rules.name"
            ]
        );

        let yaml = navigation_symbols(
            "mkdocs.yml",
            "theme:\n  features:\n    search: true\nnav:\n  - Home: index.md\n",
        );
        assert!(yaml.iter().any(|s| s.name == "theme.features.search"));

        let json = navigation_symbols(
            "package.json",
            "{\n  \"scripts\": {\n    \"test\": \"cargo test\"\n  }\n}\n",
        );
        assert!(json.iter().any(|s| s.name == "scripts.test"));
    }

    #[test]
    fn local_markdown_links_resolve_relative_to_document() {
        let links = local_markdown_links(
            "docs/guide/a.md",
            "[next](b.md#install)\n[root](/README.md)\n[web](https://example.com/x.md)\n",
        );
        assert_eq!(links, vec!["README.md", "docs/guide/b.md"]);
    }

    #[test]
    fn rebuild_link_edges_connects_indexed_docs() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO sources (source_id, provider, scope, connector_version, auth_provider, list_keys_json, config_hash, last_sync_at, last_success_at, last_error)
             VALUES ('localfiles', 'localfiles', 'local', 'v1', NULL, '[]', 'cfg', 'now', 'now', NULL)",
            [],
        )
        .unwrap();
        for (doc, path, body) in [
            ("d1", "docs/a.md", "[B](b.md)\n"),
            ("d2", "docs/b.md", "# B\n\nBody long enough for chunks.\n"),
        ] {
            conn.execute(
                "INSERT INTO documents (doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind, author_id, author_name, created_at, updated_at, observed_at, version, content_sha256, body, metadata_json)
                 VALUES (?1, 'localfiles', ?2, ?3, ?4, NULL, ?2, 'text/markdown', 'file', NULL, NULL, NULL, NULL, 'now', 'v', 'sha', ?5, '{}')",
                params![doc, path, format!("localfiles:{path}"), path, body],
            )
            .unwrap();
        }
        rebuild_link_edges(&conn, "localfiles").unwrap();
        let links: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM edges WHERE rel = 'links_to'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(links, 1);
    }

    #[test]
    fn prune_vanished_removes_navigation_rows() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO documents (doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind, author_id, author_name, created_at, updated_at, observed_at, version, content_sha256, body, metadata_json)
             VALUES ('keep', 'git', 'docs/keep.md', 'git:docs/keep.md', 'Keep', NULL, 'docs/keep.md', 'text/markdown', 'file', NULL, NULL, NULL, 'now', 'now', 'v', 'sha', '# Keep', '{}'),
                    ('gone', 'git', 'docs/gone.md', 'git:docs/gone.md', 'Gone', NULL, 'docs/gone.md', 'text/markdown', 'file', NULL, NULL, NULL, 'now', 'now', 'v', 'sha', '# Gone', '{}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO chunks (chunk_id, doc_id, chunk_index, heading_path, section_anchor, start_byte, end_byte, start_line, end_line, token_count, text, text_sha256, metadata_json)
             VALUES ('gone-c', 'gone', 0, '(root)', NULL, 0, 6, 1, 1, 1, '# Gone', 'sha', '{}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO spans (span_id, doc_id, chunk_id, span_kind, label, start_byte, end_byte, start_line, end_line, stable_hash, metadata_json)
             VALUES ('gone-s', 'gone', 'gone-c', 'heading', 'Gone', 0, 6, 1, 1, 'hash', '{}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO symbols (symbol_id, doc_id, span_id, language, symbol_kind, name, qualified_name, signature, start_byte, end_byte, start_line, end_line, metadata_json)
             VALUES ('gone-y', 'gone', 'gone-s', 'markdown', 'heading', 'Gone', 'docs/gone.md#Gone', '# Gone', 0, 6, 1, 1, '{}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO edges (edge_id, from_type, from_id, to_type, to_id, rel, confidence, evidence_span_id, created_by, created_at, metadata_json)
             VALUES ('gone-e', 'doc', 'gone', 'doc', 'keep', 'links_to', 1.0, NULL, 'test', 'now', '{}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tags (target_type, target_id, status, note, \"by\", \"at\", metadata_json)
             VALUES ('doc', 'gone', 'stale', '', 'test', 'now', '{}')",
            [],
        )
        .unwrap();

        let current = BTreeSet::from(["docs/keep.md".to_string()]);
        let deleted = prune_vanished(&conn, "git", &current).unwrap();
        assert_eq!(deleted, 1);
        for table in ["documents", "chunks", "spans", "symbols", "edges", "tags"] {
            let count: i64 = conn
                .query_row(
                    &format!(
                        "SELECT COUNT(*) FROM {table} WHERE {}",
                        match table {
                            "documents" | "chunks" | "spans" | "symbols" => "doc_id = 'gone'",
                            "edges" => "from_id = 'gone' OR to_id = 'gone'",
                            "tags" => "target_id = 'gone'",
                            _ => "1 = 0",
                        }
                    ),
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0, "{table}");
        }
    }

    #[test]
    fn record_event_stores_docs_deleted() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();
        record_event(&conn, "git", "start", "success", 3, 2, 1, None).unwrap();
        let deleted: i64 = conn
            .query_row("SELECT docs_deleted FROM sync_events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(deleted, 1);
    }
}
