//! Curation: tags, glossary, facts, extract, audit kb, humanize (SPEC §5.3/§5.4/§10).
//!
//! Tags live in the catalog `tags` table (§8.7), keyed `(target_type, target_id)`:
//! a ref that resolves to an indexed document is stored as a `doc` tag on its
//! `doc_id`; an unresolved repo path is stored as a `ref` tag on the normalized
//! path (and reconciled into a `doc` tag once it gets indexed, §index::sync).
//! Team sharing rides the shared warehouse like every other catalog table (§9).
//! The glossary is STYLE.md's Terminology table (Use / Not columns); FACTS.md
//! is the deterministic grounding ledger (one `- fact  (source)` per line).

use crate::{authcmd, config, index, workspace};
use anyhow::Result;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Normalize a repo path / doc ref key: strip a leading `./`.
fn norm_ref(r: &str) -> String {
    r.strip_prefix("./").unwrap_or(r).to_string()
}

/// Effective config for a root (defaults → global → repo → repo-local).
fn resolved(root: &Path) -> Value {
    config::resolve(Some(root))
}

/// Valid tag statuses from resolved config `tags.statuses`.
fn statuses_in(root: &Path) -> Vec<String> {
    resolved(root)["tags"]["statuses"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// `git config user.name` in root, else $USER, else "unknown".
fn author_in(root: &Path) -> String {
    if let Ok(out) = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["config", "user.name"])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    std::env::var("USER").unwrap_or_else(|_| "unknown".into())
}

fn today() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

/// Published catalog warehouses for this root (repo workspace + `_global`),
/// filtered to those already built. Tag storage lives here (§8.7).
fn published_catalog_paths(root: &Path) -> Vec<PathBuf> {
    catalog_paths(root)
        .into_iter()
        .filter(|p| index::warehouse_published_at(p))
        .collect()
}

// ---------------------------------------------------------------------------
// mari tag
// ---------------------------------------------------------------------------

pub fn tag(
    args: &[String],
    note: Option<&str>,
    status_filter: Option<&str>,
    json: bool,
    source: Option<&str>,
    superseded_by: Option<&str>,
) -> Result<i32> {
    tag_in(
        &workspace::work_root(),
        args,
        note,
        status_filter,
        json,
        source,
        superseded_by,
    )
}

fn tag_in(
    root: &Path,
    args: &[String],
    note: Option<&str>,
    status_filter: Option<&str>,
    json: bool,
    source: Option<&str>,
    superseded_by: Option<&str>,
) -> Result<i32> {
    match args.first().map(|s| s.as_str()) {
        None => {
            eprintln!("usage: mari tag <path-or-ref> <status> [--note \"…\"] [--superseded-by <ref>] | mari tag analyze [path…] [--status S] [--source <key>] [--json] | mari tag list [--status S] [--json] | mari tag remove <ref>");
            Ok(2)
        }
        Some("list") => tag_list(root, status_filter, json),
        Some("analyze") => tag_analyze(root, &args[1..], status_filter, source, json),
        Some("remove") => {
            let Some(r) = args.get(1) else {
                eprintln!("usage: mari tag remove <path-or-ref>");
                return Ok(2);
            };
            let key = norm_ref(r);
            let paths = published_catalog_paths(root);
            if paths.is_empty() {
                eprintln!("✗ no catalog yet — run `mari sync` first");
                return Ok(1);
            }
            if remove_tag(&paths, &key)? {
                println!("✓ removed tag from {key}");
                Ok(0)
            } else {
                eprintln!("✗ no tag on {key}");
                Ok(1)
            }
        }
        Some(r) => {
            let Some(status) = args.get(1) else {
                eprintln!("usage: mari tag <path-or-ref> <status> [--note \"…\"]");
                return Ok(2);
            };
            let valid = statuses_in(root);
            if !valid.iter().any(|s| s == status) {
                eprintln!(
                    "✗ unknown status '{status}' — valid statuses: {}",
                    valid.join(", ")
                );
                return Ok(2);
            }
            let key = norm_ref(r);
            let paths = published_catalog_paths(root);
            if paths.is_empty() {
                eprintln!("✗ no catalog yet — run `mari sync` first");
                return Ok(1);
            }
            // Resolve the successor up front so a bad `--superseded-by` errors
            // before we write the tag (the flag errors if it does not resolve).
            let successor = match superseded_by {
                Some(s) => match resolve_ref_span(&paths, s)? {
                    Some(found) => Some(found),
                    None => {
                        eprintln!("✗ --superseded-by `{s}` does not resolve to an indexed document — is it synced?");
                        return Ok(1);
                    }
                },
                None => None,
            };
            let mut entry = json!({ "status": status, "by": author_in(root), "at": today() });
            if let Some(n) = note {
                entry["note"] = json!(n);
            }
            apply_tag(&paths, &key, &entry)?;
            match note {
                Some(n) => println!("✓ tagged {key} {status} — {n}"),
                None => println!("✓ tagged {key} {status}"),
            }
            if let Some((succ_path, _)) = &successor {
                let pointer = record_supersession(&paths, superseded_by.unwrap(), &key)?;
                if let Some(p) = pointer {
                    println!("  ↳ replaced by {p}");
                }
                let _ = succ_path;
            }
            Ok(0)
        }
    }
}

/// A tag row for listing: normalized display ref + status/note/by/at.
struct TagRow {
    display: String,
    status: String,
    note: String,
    by: String,
    at: String,
}

fn tag_list(root: &Path, status_filter: Option<&str>, json_out: bool) -> Result<i32> {
    if let Some(status) = status_filter {
        let valid = statuses_in(root);
        if !valid.iter().any(|s| s == status) {
            eprintln!(
                "✗ unknown status filter '{status}' — valid statuses: {}",
                valid.join(", ")
            );
            return Ok(2);
        }
    }
    let mut rows: BTreeMap<String, TagRow> = BTreeMap::new();
    for path in published_catalog_paths(root) {
        let Some(conn) = index::open_readonly_path(&path)? else {
            continue;
        };
        let mut stmt = conn.prepare(
            "SELECT COALESCE(d.path, d.canonical_ref, t.target_id),
                    t.status, COALESCE(t.note, ''), COALESCE(t.\"by\", ''), COALESCE(t.\"at\", '')
               FROM tags t
               LEFT JOIN documents d ON t.target_type = 'doc' AND d.doc_id = t.target_id
              WHERE t.status IS NOT NULL",
        )?;
        let fetched = stmt.query_map([], |r| {
            Ok(TagRow {
                display: r.get::<_, String>(0)?,
                status: r.get::<_, String>(1)?,
                note: r.get::<_, String>(2)?,
                by: r.get::<_, String>(3)?,
                at: r.get::<_, String>(4)?,
            })
        })?;
        for row in fetched.flatten() {
            if let Some(s) = status_filter {
                if row.status != s {
                    continue;
                }
            }
            rows.entry(norm_ref(&row.display)).or_insert(row);
        }
    }
    if json_out {
        let out: Vec<Value> = rows
            .iter()
            .map(|(r, v)| {
                json!({
                    "ref": r, "status": v.status, "note": v.note, "by": v.by, "at": v.at
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(0);
    }
    if rows.is_empty() {
        println!(
            "no tags{}",
            status_filter
                .map(|s| format!(" with status {s}"))
                .unwrap_or_default()
        );
        return Ok(0);
    }
    for (r, v) in &rows {
        let by = if v.by.is_empty() { "?" } else { v.by.as_str() };
        let at = if v.at.is_empty() { "?" } else { v.at.as_str() };
        let note = if v.note.is_empty() {
            String::new()
        } else {
            format!("  — {}", v.note)
        };
        println!("{r}  [{}]  ({by}, {at}){note}", v.status);
    }
    println!("{} tag(s)", rows.len());
    Ok(0)
}

/// Curation tag for a repo-relative path or doc ref, if any (SPEC §10.1).
/// Reads the catalog `tags` table: a `ref` tag on the normalized path, or a
/// `doc` tag on any document the path resolves to.
pub fn tag_of(root: &Path, r: &str) -> Option<String> {
    tag_of_paths(&published_catalog_paths(root), r)
}

fn tag_of_paths(paths: &[PathBuf], r: &str) -> Option<String> {
    let key = norm_ref(r);
    let like = format!("%{key}");
    for path in paths {
        let Ok(Some(conn)) = index::open_readonly_path(path) else {
            continue;
        };
        let status: Option<String> = conn
            .query_row(
                "SELECT t.status
                   FROM tags t
                   LEFT JOIN documents d ON t.target_type = 'doc' AND d.doc_id = t.target_id
                  WHERE (t.target_type = 'ref' AND t.target_id = ?1)
                     OR (t.target_type = 'doc'
                         AND (d.path = ?1 OR d.canonical_ref = ?1 OR d.external_id = ?1
                              OR d.canonical_ref LIKE ?2))
                  LIMIT 1",
                duckdb::params![key, like],
                |row| row.get::<_, String>(0),
            )
            .ok();
        if status.is_some() {
            return status;
        }
    }
    None
}

/// Write a tag across every published catalog: a `doc` tag on each document the
/// ref resolves to, or a `ref` tag on the primary catalog when it resolves
/// nowhere. Any prior `ref`/`doc` tag on the same target is replaced first.
fn apply_tag(paths: &[PathBuf], key: &str, entry: &Value) -> Result<()> {
    let mut resolved = false;
    for path in paths {
        let conn = index::open_catalog_at(path)?;
        let doc_ids = catalog_doc_ids_for_target(&conn, key)?;
        conn.execute(
            "DELETE FROM tags WHERE target_type = 'ref' AND target_id = ?1",
            [key],
        )?;
        for doc_id in &doc_ids {
            conn.execute(
                "DELETE FROM tags WHERE target_type = 'doc' AND target_id = ?1",
                [doc_id],
            )?;
            insert_tag(&conn, "doc", doc_id, key, entry)?;
            resolved = true;
        }
        index::publish_to_path(&conn, path)?;
    }
    if !resolved {
        // Unresolved repo path: store a `ref` tag in the primary catalog.
        let conn = index::open_catalog_at(&paths[0])?;
        conn.execute(
            "DELETE FROM tags WHERE target_type = 'ref' AND target_id = ?1",
            [key],
        )?;
        insert_tag(&conn, "ref", key, key, entry)?;
        index::publish_to_path(&conn, &paths[0])?;
    }
    Ok(())
}

/// Remove a tag from every published catalog. Returns true if anything was
/// deleted.
fn remove_tag(paths: &[PathBuf], key: &str) -> Result<bool> {
    let mut removed = false;
    for path in paths {
        let conn = index::open_catalog_at(path)?;
        let mut n = conn.execute(
            "DELETE FROM tags WHERE target_type = 'ref' AND target_id = ?1",
            [key],
        )?;
        for doc_id in catalog_doc_ids_for_target(&conn, key)? {
            n += conn.execute(
                "DELETE FROM tags WHERE target_type = 'doc' AND target_id = ?1",
                [&doc_id],
            )?;
        }
        if n > 0 {
            index::publish_to_path(&conn, path)?;
            removed = true;
        }
    }
    Ok(removed)
}

fn insert_tag(
    conn: &duckdb::Connection,
    target_type: &str,
    target_id: &str,
    display: &str,
    entry: &Value,
) -> Result<()> {
    conn.execute(
        "INSERT INTO tags (target_type, target_id, status, note, \"by\", \"at\", metadata_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        duckdb::params![
            target_type,
            target_id,
            entry["status"].as_str().unwrap_or(""),
            entry["note"].as_str().unwrap_or(""),
            entry["by"].as_str().unwrap_or("unknown"),
            entry["at"].as_str().unwrap_or(""),
            json!({"source": "mari tag", "target": display}).to_string()
        ],
    )?;
    Ok(())
}

/// The whole-document span (`span_id`, display ref) for a ref that resolves to
/// an indexed document, searching each published catalog in turn.
fn resolve_ref_span(paths: &[PathBuf], target: &str) -> Result<Option<(PathBuf, String)>> {
    let key = norm_ref(target);
    for path in paths {
        let Some(conn) = index::open_readonly_path(path)? else {
            continue;
        };
        if let Some(span) = first_span_for_ref(&conn, &key)? {
            return Ok(Some((path.clone(), span)));
        }
    }
    Ok(None)
}

/// First (whole-document) span id of the first document a ref resolves to.
fn first_span_for_ref(conn: &duckdb::Connection, key: &str) -> Result<Option<String>> {
    let Some(doc_id) = catalog_doc_ids_for_target(conn, key)?.into_iter().next() else {
        return Ok(None);
    };
    let span: Option<String> = conn
        .query_row(
            "SELECT span_id FROM spans WHERE doc_id = ?1 ORDER BY start_line ASC LIMIT 1",
            [&doc_id],
            |r| r.get::<_, String>(0),
        )
        .ok();
    Ok(span)
}

/// Record a confirmed `replaces` lineage edge from the successor's whole-doc
/// span to the deprecated doc's, provenance `human` (§8.3). Written into every
/// catalog where both refs resolve. Returns a display pointer to the successor.
fn record_supersession(
    paths: &[PathBuf],
    successor_ref: &str,
    deprecated_ref: &str,
) -> Result<Option<String>> {
    let successor = norm_ref(successor_ref);
    let deprecated = norm_ref(deprecated_ref);
    let mut pointer = None;
    for path in paths {
        let conn = index::open_catalog_at(path)?;
        let (Some(from_span), Some(to_span)) = (
            first_span_for_ref(&conn, &successor)?,
            first_span_for_ref(&conn, &deprecated)?,
        ) else {
            continue;
        };
        let lineage_id = index::hash_hex(&format!("lineage:{from_span}:{to_span}"));
        conn.execute(
            "DELETE FROM lineage_edges WHERE lineage_id = ?1",
            [&lineage_id],
        )?;
        conn.execute(
            "INSERT INTO lineage_edges (lineage_id, from_span_id, to_span_id, rel, status, confidence, confirmed_by, confirmed_at, last_checked_at, metadata_json)
             VALUES (?1, ?2, ?3, 'replaces', 'confirmed', 1.0, ?4, ?5, ?5, ?6)",
            duckdb::params![
                lineage_id,
                from_span,
                to_span,
                author_in(&workspace::work_root()),
                index::now(),
                json!({"by": "human", "note": "tag --superseded-by"}).to_string(),
            ],
        )?;
        index::publish_to_path(&conn, path)?;
        if pointer.is_none() {
            pointer = Some(successor.clone());
        }
    }
    Ok(pointer)
}

fn catalog_doc_ids_for_target(conn: &duckdb::Connection, target: &str) -> Result<Vec<String>> {
    let norm = target.strip_prefix("./").unwrap_or(target);
    let like = format!("%{norm}");
    let mut stmt = conn.prepare(
        "SELECT doc_id FROM documents
          WHERE canonical_ref = ?1 OR path = ?1 OR external_id = ?1 OR canonical_ref LIKE ?2",
    )?;
    let rows = stmt.query_map(duckdb::params![norm, like], |r| r.get::<_, String>(0))?;
    Ok(rows.flatten().collect())
}

// ---------------------------------------------------------------------------
// mari tag analyze (SPEC §10.4)
// ---------------------------------------------------------------------------

/// One document as seen by the catalog, used to build context cards.
struct AnalyzeDoc {
    doc_id: String,
    reference: String,
    title: String,
    path: String,
    source: String,
    updated_at: Option<String>,
    body: String,
}

#[derive(serde::Serialize)]
struct VersionMarker {
    marker: String,
    #[serde(rename = "where")]
    where_found: &'static str,
}

#[derive(serde::Serialize)]
struct Sibling {
    path: String,
    version_marker: String,
}

#[derive(serde::Serialize)]
struct NearDup {
    #[serde(rename = "ref")]
    reference: String,
    cosine: f64,
    relation: &'static str, // newer | older | unknown
}

#[derive(serde::Serialize, Default)]
struct DraftMarkers {
    todo_count: usize,
    wip: bool,
}

#[derive(serde::Serialize)]
struct ContextCard {
    #[serde(rename = "ref")]
    reference: String,
    title: String,
    path: String,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_tag: Option<String>,
    outline: Vec<String>,
    version_markers: Vec<VersionMarker>,
    siblings: Vec<Sibling>,
    #[serde(skip_serializing_if = "Option::is_none")]
    near_dup: Option<NearDup>,
    inbound_links: i64,
    draft_markers: DraftMarkers,
    #[serde(skip_serializing_if = "Option::is_none")]
    modified_at: Option<String>,
}

#[derive(serde::Serialize)]
struct RepoContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    project_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    git_tag: Option<String>,
    version_conventions: Vec<String>,
}

/// `mari tag analyze` — deterministic context extraction for the `/mari tag
/// analyze` skill flow (§10.4). Emits a repo-context block plus one bounded
/// context card per doc in scope. The CLI extracts; it never calls an LLM.
fn tag_analyze(
    root: &Path,
    path_args: &[String],
    status_filter: Option<&str>,
    source: Option<&str>,
    json: bool,
) -> Result<i32> {
    if let Some(s) = source {
        if !authcmd::SOURCES.contains(&s) {
            eprintln!("✗ unknown source: {s}");
            return Ok(2);
        }
    }
    if let Some(status) = status_filter {
        let valid = statuses_in(root);
        if !valid.iter().any(|s| s == status) {
            eprintln!(
                "✗ unknown status filter '{status}' — valid statuses: {}",
                valid.join(", ")
            );
            return Ok(2);
        }
    }

    let repo_ctx = repo_context(root);
    let docs = analyze_docs(root)?;
    if docs.is_empty() {
        // No catalog / no indexed docs: fall back to repo-path cards for the
        // explicit paths the user named (siblings/near-dup/inbound need an index).
        if !path_args.is_empty() {
            return tag_analyze_repo_only(root, path_args, &repo_ctx, json);
        }
        eprintln!("note: no indexed documents — run `mari sync` first, or pass repo paths to analyze uncataloged files");
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({ "repo_context": repo_ctx, "cards": [] }))?
            );
        }
        return Ok(0);
    }

    let tags = analyze_current_tags(root)?;
    let inbound = analyze_inbound_counts(root)?;
    let neighbours = analyze_neighbours();

    // Version-marker conventions observed across every doc ref/title.
    let conventions = observe_version_conventions(&docs);
    let repo_ctx = RepoContext {
        version_conventions: conventions,
        ..repo_ctx
    };

    let cards: Vec<ContextCard> = docs
        .iter()
        .filter(|d| in_scope(d, path_args, source, status_filter, &tags))
        .map(|d| build_card(d, &docs, &tags, &inbound, &neighbours))
        .collect();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "repo_context": repo_ctx,
                "cards": cards,
            }))?
        );
        return Ok(0);
    }
    print_repo_context(&repo_ctx);
    if cards.is_empty() {
        println!("\nno documents in scope — every doc in range is already handled.");
        return Ok(0);
    }
    println!("\n{} document(s) in scope:\n", cards.len());
    for c in &cards {
        print_card(c);
    }
    println!(
        "extracted context only — the skill judges from these cards and applies tags via `mari tag`."
    );
    Ok(0)
}

/// Whether a doc is in the analyze scope: matching path/source filters and,
/// by default, currently untagged (or matching `--status` when given).
fn in_scope(
    d: &AnalyzeDoc,
    path_args: &[String],
    source: Option<&str>,
    status_filter: Option<&str>,
    tags: &HashMap<String, String>,
) -> bool {
    if let Some(s) = source {
        if d.source != s {
            return false;
        }
    }
    if !path_args.is_empty() && !path_args.iter().any(|p| path_matches(p, &d.path, &d.reference)) {
        return false;
    }
    match status_filter {
        Some(s) => tags.get(&d.doc_id).map(String::as_str) == Some(s),
        None => !tags.contains_key(&d.doc_id),
    }
}

/// Match a repo path or glob (`*` wildcard) against a doc path/ref.
fn path_matches(pattern: &str, path: &str, reference: &str) -> bool {
    let pat = pattern.strip_prefix("./").unwrap_or(pattern);
    if pat.contains('*') {
        let mut re = String::from("(?i)");
        for ch in pat.chars() {
            match ch {
                '*' => re.push_str(".*"),
                c if c.is_ascii_alphanumeric() => re.push(c),
                c => {
                    re.push('\\');
                    re.push(c);
                }
            }
        }
        if let Ok(rx) = Regex::new(&re) {
            return rx.is_match(path) || rx.is_match(reference);
        }
    }
    let lp = pat.to_lowercase();
    path.to_lowercase().contains(&lp) || reference.to_lowercase().contains(&lp)
}

/// All indexed documents across published catalogs (deduped by normalized path).
fn analyze_docs(root: &Path) -> Result<Vec<AnalyzeDoc>> {
    let mut docs = Vec::new();
    let mut seen = HashSet::new();
    for path in published_catalog_paths(root) {
        let Some(conn) = index::open_readonly_path(&path)? else {
            continue;
        };
        // Curation tags apply to prose docs, not test fixtures or data files:
        // restrict to markdown.
        let mut stmt = conn.prepare(
            "SELECT doc_id, canonical_ref, COALESCE(title, ''), COALESCE(path, ''), source_id,
                    COALESCE(updated_at, ''), COALESCE(body, '')
               FROM documents
              WHERE lower(path) LIKE '%.md' OR lower(path) LIKE '%.markdown'
                 OR lower(canonical_ref) LIKE '%.md' OR lower(canonical_ref) LIKE '%.markdown'",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(AnalyzeDoc {
                doc_id: r.get::<_, String>(0)?,
                reference: r.get::<_, String>(1)?,
                title: r.get::<_, String>(2)?,
                path: r.get::<_, String>(3)?,
                source: r.get::<_, String>(4)?,
                updated_at: {
                    let u: String = r.get::<_, String>(5)?;
                    if u.is_empty() {
                        None
                    } else {
                        Some(u)
                    }
                },
                body: r.get::<_, String>(6)?,
            })
        })?;
        for d in rows.flatten() {
            let dedup = norm_ref(if d.path.is_empty() {
                &d.reference
            } else {
                &d.path
            });
            if seen.insert(dedup) {
                docs.push(d);
            }
        }
    }
    Ok(docs)
}

/// doc_id → current tag status, across published catalogs.
fn analyze_current_tags(root: &Path) -> Result<HashMap<String, String>> {
    let mut tags = HashMap::new();
    for path in published_catalog_paths(root) {
        let Some(conn) = index::open_readonly_path(&path)? else {
            continue;
        };
        let mut stmt =
            conn.prepare("SELECT target_id, status FROM tags WHERE target_type = 'doc'")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        for (id, status) in rows.flatten() {
            tags.entry(id).or_insert(status);
        }
    }
    Ok(tags)
}

/// doc_id → inbound doc→doc edge count (§8.4), across published catalogs.
fn analyze_inbound_counts(root: &Path) -> Result<HashMap<String, i64>> {
    let mut counts: HashMap<String, i64> = HashMap::new();
    for path in published_catalog_paths(root) {
        let Some(conn) = index::open_readonly_path(&path)? else {
            continue;
        };
        let mut stmt = conn.prepare(
            "SELECT to_id, COUNT(DISTINCT from_id) FROM edges
              WHERE to_type = 'doc' AND from_type = 'doc' GROUP BY to_id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?;
        for (id, n) in rows.flatten() {
            let e = counts.entry(id).or_default();
            *e = (*e).max(n);
        }
    }
    Ok(counts)
}

/// doc_id → nearest neighbour (doc_id, cosine), merged across repo + global
/// vector stores.
fn analyze_neighbours() -> HashMap<String, (String, f64)> {
    let mut best: HashMap<String, (String, f64)> = HashMap::new();
    for global in [false, true] {
        if let Some(map) = index::vector::doc_neighbours(global, 1) {
            for (doc, neighbours) in map {
                if let Some((other, score)) = neighbours.into_iter().next() {
                    let entry = best.entry(doc).or_insert((other.clone(), score));
                    if score > entry.1 {
                        *entry = (other, score);
                    }
                }
            }
        }
    }
    best
}

fn build_card(
    d: &AnalyzeDoc,
    all: &[AnalyzeDoc],
    tags: &HashMap<String, String>,
    inbound: &HashMap<String, i64>,
    neighbours: &HashMap<String, (String, f64)>,
) -> ContextCard {
    let near_dup = neighbours.get(&d.doc_id).and_then(|(other_id, cosine)| {
        all.iter().find(|o| &o.doc_id == other_id).map(|o| NearDup {
            reference: o.reference.clone(),
            cosine: (cosine * 1000.0).round() / 1000.0,
            relation: newer_older(d.updated_at.as_deref(), o.updated_at.as_deref()),
        })
    });
    let clean = strip_html_comments(&d.body);
    ContextCard {
        reference: d.reference.clone(),
        title: cap(&d.title, 120),
        path: d.path.clone(),
        source: d.source.clone(),
        current_tag: tags.get(&d.doc_id).cloned(),
        outline: outline(&clean),
        version_markers: version_markers(&d.path, &d.title, &clean),
        siblings: siblings(d, all),
        near_dup,
        inbound_links: inbound.get(&d.doc_id).copied().unwrap_or(0),
        draft_markers: draft_markers(d),
        modified_at: d.updated_at.clone(),
    }
}

/// "newer"/"older"/"unknown" for `d` relative to `other` by modified time.
fn newer_older(d: Option<&str>, other: Option<&str>) -> &'static str {
    match (d, other) {
        (Some(a), Some(b)) if a > b => "newer",
        (Some(a), Some(b)) if a < b => "older",
        _ => "unknown",
    }
}

fn cap(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

/// First H1 + up to 8 H2s, each capped at 80 chars.
fn outline(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut h1 = None;
    for line in body.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("## ") {
            if out.len() < 8 {
                out.push(format!("H2 {}", cap(rest, 80)));
            }
        } else if let Some(rest) = t.strip_prefix("# ") {
            if h1.is_none() {
                h1 = Some(format!("H1 {}", cap(rest, 80)));
            }
        }
    }
    let mut result = Vec::new();
    if let Some(h1) = h1 {
        result.push(h1);
    }
    result.extend(out);
    result
}

/// Remove `<!-- … -->` comment blocks (license headers, editor notes) so they
/// don't pollute the lede or masquerade as version markers.
fn strip_html_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start + 4..].find("-->") {
            Some(end) => rest = &rest[start + 4 + end + 3..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Version markers (`v2`, `1.15`) from the doc's path, title, and front matter —
/// the places a version *labels* a doc. Body prose is deliberately excluded: an
/// arbitrary decimal in a sentence or code sample is not a version marker.
/// Up to 4, path/title/front-matter order.
fn version_markers(path: &str, title: &str, body: &str) -> Vec<VersionMarker> {
    let re = Regex::new(r"(?i)\bv\d+(?:\.\d+)*\b|\b\d+\.\d+(?:\.\d+)?\b|\bas of \d+(?:\.\d+)*\b")
        .unwrap();
    let front_matter = body
        .strip_prefix("---")
        .and_then(|r| r.split("---").next())
        .unwrap_or("");
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for (text, where_found) in [
        (path, "path"),
        (title, "title"),
        (front_matter, "front-matter"),
    ] {
        for m in re.find_iter(text) {
            let marker = m.as_str().to_lowercase();
            if seen.insert(marker.clone()) {
                out.push(VersionMarker {
                    marker,
                    where_found,
                });
                if out.len() >= 4 {
                    return out;
                }
            }
        }
    }
    out
}

/// Same-stem versioned siblings: other docs whose basename stem (its version
/// marker stripped) matches this doc's and that carry a version marker. Up to 5.
fn siblings(d: &AnalyzeDoc, all: &[AnalyzeDoc]) -> Vec<Sibling> {
    let ver = Regex::new(r"(?i)v?\d+[._]\d+(?:[._]\d+)?").unwrap();
    let stem_of = |p: &str| -> Option<String> {
        let base = Path::new(p).file_name()?.to_str()?.to_lowercase();
        if !ver.is_match(&base) {
            return None;
        }
        Some(ver.replace_all(&base, "#").to_string())
    };
    let Some(stem) = stem_of(if d.path.is_empty() {
        &d.reference
    } else {
        &d.path
    }) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for o in all {
        if o.doc_id == d.doc_id {
            continue;
        }
        let op = if o.path.is_empty() {
            &o.reference
        } else {
            &o.path
        };
        if stem_of(op).as_deref() == Some(stem.as_str()) {
            let marker = ver
                .find(&Path::new(op).file_name().and_then(|b| b.to_str()).unwrap_or(op).to_lowercase())
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            out.push(Sibling {
                path: op.clone(),
                version_marker: marker,
            });
            if out.len() >= 5 {
                break;
            }
        }
    }
    out
}

fn draft_markers(d: &AnalyzeDoc) -> DraftMarkers {
    // Explicit body markers (whole-word) are real signal.
    let todo_re = Regex::new(r"\b(?:TODO|TBD|FIXME)\b").unwrap();
    let todo_count = todo_re.find_iter(&d.body).count();
    // A draft/WIP *flag* is a structural cue (path segment or title word), not a
    // substring of prose ("swiping" is not WIP).
    let label = format!("{} {}", d.path.to_lowercase(), d.title.to_lowercase());
    let wip = Regex::new(r"\bwip\b|\bdraft\b|/drafts?/")
        .unwrap()
        .is_match(&label);
    DraftMarkers { todo_count, wip }
}

fn observe_version_conventions(docs: &[AnalyzeDoc]) -> Vec<String> {
    let mut conventions: BTreeSet<String> = BTreeSet::new();
    let ver = Regex::new(r"(?i)v\d+\.\d+(?:\.\d+)?|v\d+|\d+\.\d+(?:\.\d+)?").unwrap();
    for d in docs {
        let src = if d.path.is_empty() {
            &d.reference
        } else {
            &d.path
        };
        if let Some(m) = ver.find(&src.to_lowercase()) {
            // Report the shape, not the specific version.
            let shape = generalize_version(m.as_str());
            let scope = if src.contains('/') && m.start() < src.rfind('/').unwrap_or(0) {
                "path"
            } else {
                "filename"
            };
            conventions.insert(format!("{shape} in {scope}"));
        }
        if d.title.to_lowercase().contains("as of ") {
            conventions.insert("\"as of <version>\" in title".to_string());
        }
    }
    conventions.into_iter().take(6).collect()
}

fn generalize_version(m: &str) -> String {
    let dots = m.matches('.').count();
    let v = if m.to_lowercase().starts_with('v') { "v" } else { "" };
    match dots {
        0 => format!("{v}N"),
        1 => format!("{v}N.N"),
        _ => format!("{v}N.N.N"),
    }
}

/// Repo-level context: project version (manifest + latest semver git tag).
fn repo_context(root: &Path) -> RepoContext {
    RepoContext {
        project_version: manifest_version(root),
        git_tag: latest_semver_tag(root),
        version_conventions: Vec::new(),
    }
}

fn manifest_version(root: &Path) -> Option<String> {
    // Cargo.toml / pyproject.toml: first `version = "…"` under a package table.
    for name in ["Cargo.toml", "pyproject.toml"] {
        if let Ok(text) = std::fs::read_to_string(root.join(name)) {
            for line in text.lines() {
                let t = line.trim();
                if let Some(rest) = t.strip_prefix("version") {
                    let rest = rest.trim_start().trim_start_matches('=').trim();
                    let v = rest.trim_matches(['"', '\'']).trim();
                    if !v.is_empty() && v.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                        return Some(v.to_string());
                    }
                }
            }
        }
    }
    // package.json: "version": "…".
    if let Ok(text) = std::fs::read_to_string(root.join("package.json")) {
        if let Ok(v) = serde_json::from_str::<Value>(&text) {
            if let Some(ver) = v["version"].as_str() {
                return Some(ver.to_string());
            }
        }
    }
    None
}

fn latest_semver_tag(root: &Path) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["tag", "--list", "--sort=-v:refname"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let semver = Regex::new(r"^v?\d+\.\d+(?:\.\d+)?$").unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .find(|t| semver.is_match(t))
        .map(String::from)
}

/// Repo-only fallback when there is no catalog: build reduced cards (no
/// siblings/near-dup/inbound, which need the index) for the named paths.
fn tag_analyze_repo_only(
    root: &Path,
    path_args: &[String],
    repo_ctx: &RepoContext,
    json: bool,
) -> Result<i32> {
    let files = crate::detector::runner::collect_files(path_args);
    let mut cards = Vec::new();
    for f in &files {
        let rel = f
            .strip_prefix(root)
            .unwrap_or(f)
            .to_string_lossy()
            .to_string();
        let Ok(body) = std::fs::read_to_string(f) else {
            continue;
        };
        let title = body
            .lines()
            .find_map(|l| l.trim().strip_prefix("# ").map(|s| s.to_string()))
            .unwrap_or_else(|| rel.clone());
        let d = AnalyzeDoc {
            doc_id: rel.clone(),
            reference: rel.clone(),
            title,
            path: rel.clone(),
            source: "localfiles".into(),
            updated_at: std::fs::metadata(f)
                .and_then(|m| m.modified())
                .ok()
                .map(|_| String::new())
                .filter(|s| !s.is_empty()),
            body,
        };
        let clean = strip_html_comments(&d.body);
        cards.push(ContextCard {
            reference: d.reference.clone(),
            title: cap(&d.title, 120),
            path: d.path.clone(),
            source: d.source.clone(),
            current_tag: None,
            outline: outline(&clean),
            version_markers: version_markers(&d.path, &d.title, &clean),
            siblings: Vec::new(),
            near_dup: None,
            inbound_links: 0,
            draft_markers: draft_markers(&d),
            modified_at: None,
        });
    }
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "repo_context": repo_ctx,
                "cards": cards,
            }))?
        );
        return Ok(0);
    }
    print_repo_context(repo_ctx);
    println!(
        "\n{} uncataloged file(s) (no index — siblings/near-dup/inbound unavailable):\n",
        cards.len()
    );
    for c in &cards {
        print_card(c);
    }
    Ok(0)
}

fn print_repo_context(ctx: &RepoContext) {
    println!("repo context:");
    let same_version = |a: &str, b: &str| a.trim_start_matches('v') == b.trim_start_matches('v');
    match (&ctx.project_version, &ctx.git_tag) {
        (Some(v), Some(t)) if !same_version(v, t) => {
            println!("  project version: {v} (manifest), {t} (latest git tag) — they disagree")
        }
        (Some(v), _) => println!("  project version: {v}"),
        (None, Some(t)) => println!("  project version: {t} (latest git tag)"),
        (None, None) => println!("  project version: unknown"),
    }
    if ctx.version_conventions.is_empty() {
        println!("  version conventions: none observed");
    } else {
        println!(
            "  version conventions: {}",
            ctx.version_conventions.join("; ")
        );
    }
}

fn print_card(c: &ContextCard) {
    let tag = c
        .current_tag
        .as_deref()
        .map(|t| format!("  [{t}]"))
        .unwrap_or_default();
    println!("• {}{tag}", c.reference);
    if !c.title.is_empty() {
        println!("    title: {}", c.title);
    }
    println!("    path: {}  source: {}", c.path, c.source);
    if !c.outline.is_empty() {
        println!("    outline: {}", c.outline.join(" · "));
    }
    if !c.version_markers.is_empty() {
        let vs: Vec<String> = c
            .version_markers
            .iter()
            .map(|v| format!("{} ({})", v.marker, v.where_found))
            .collect();
        println!("    version markers: {}", vs.join(", "));
    }
    if !c.siblings.is_empty() {
        let ss: Vec<String> = c
            .siblings
            .iter()
            .map(|s| format!("{} [{}]", s.path, s.version_marker))
            .collect();
        println!("    versioned siblings: {}", ss.join(", "));
    }
    if let Some(nd) = &c.near_dup {
        println!(
            "    near-dup: {} (cosine {:.3}, {})",
            nd.reference, nd.cosine, nd.relation
        );
    }
    if c.inbound_links > 0 {
        println!("    inbound links: {}", c.inbound_links);
    }
    if c.draft_markers.todo_count > 0 || c.draft_markers.wip {
        println!(
            "    draft markers: {} TODO/TBD/FIXME{}",
            c.draft_markers.todo_count,
            if c.draft_markers.wip {
                ", draft/WIP flag"
            } else {
                ""
            }
        );
    }
    if let Some(m) = &c.modified_at {
        println!("    modified: {m}");
    }
    println!();
}

// ---------------------------------------------------------------------------
// mari glossary
// ---------------------------------------------------------------------------

fn glossary_path(root: &Path, cfg: &Value) -> PathBuf {
    root.join(cfg["glossary"]["file"].as_str().unwrap_or("STYLE.md"))
}

pub fn glossary(args: &[String], use_: Option<&str>, not_: Option<&str>) -> Result<i32> {
    glossary_in(&workspace::work_root(), args, use_, not_)
}

fn glossary_in(
    root: &Path,
    args: &[String],
    use_: Option<&str>,
    not_: Option<&str>,
) -> Result<i32> {
    let cfg = resolved(root);
    match args.first().map(|s| s.as_str()) {
        None | Some("list") => {
            let groups = glossary_groups(root, &cfg);
            if groups.is_empty() {
                println!("no glossary terms — add with: mari glossary add <term> --use \"<canonical>\" --not \"<variants,…>\"");
                return Ok(0);
            }
            for g in &groups {
                println!("use: {}  not: {}", g[0], g[1..].join(", "));
            }
            println!("{} term(s)", groups.len());
            Ok(0)
        }
        Some("add") => {
            let Some(term) = args.get(1) else {
                eprintln!(
                    "usage: mari glossary add <term> --use \"<canonical>\" --not \"<variants,…>\""
                );
                return Ok(2);
            };
            let canonical = use_.unwrap_or(term).trim().to_string();
            let variants: Vec<String> = not_
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            glossary_add(root, &cfg, &canonical, &variants)?;
            println!(
                "✓ added glossary term: use \"{canonical}\" not \"{}\"",
                variants.join(", ")
            );
            Ok(0)
        }
        Some("harvest") => glossary_harvest(root, &cfg),
        Some(other) => {
            eprintln!("✗ unknown glossary subcommand '{other}' — expected harvest | list | add");
            Ok(2)
        }
    }
}

/// Append a Use/Not row to the Terminology table, creating the section
/// (and the file) when absent.
fn glossary_add(root: &Path, cfg: &Value, canonical: &str, variants: &[String]) -> Result<()> {
    let path = glossary_path(root, cfg);
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    let row = format!("| {} | {} |\n", canonical, variants.join(", "));
    if let Some(section) = terminology_section(&text) {
        // Insert after the last table row of the section.
        let (start, end) = section;
        let seg = &text[start..end];
        let insert_at = seg
            .lines()
            .scan(0usize, |off, l| {
                let line_start = *off;
                *off += l.len() + 1;
                Some((line_start, l))
            })
            .filter(|(_, l)| l.trim_start().starts_with('|'))
            .last()
            .map(|(off, l)| start + off + l.len() + 1)
            .unwrap_or(end);
        let at = insert_at.min(text.len());
        text.insert_str(at, &row);
    } else {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str("\n## Terminology\n\n| Use | Not |\n|---|---|\n");
        text.push_str(&row);
    }
    std::fs::write(&path, text)?;
    Ok(())
}

/// Byte range of the `## Terminology` section body (after the heading line,
/// up to the next heading or EOF).
fn terminology_section(text: &str) -> Option<(usize, usize)> {
    let mut off = 0usize;
    let mut start = None;
    for line in text.lines() {
        let next = off + line.len() + 1;
        let t = line.trim();
        match start {
            None => {
                if t.starts_with('#')
                    && t.trim_start_matches('#')
                        .trim()
                        .eq_ignore_ascii_case("terminology")
                {
                    start = Some(next.min(text.len()));
                }
            }
            Some(s) if t.starts_with('#') => return Some((s, off)),
            Some(_) => {}
        }
        off = next;
    }
    start.map(|s| (s, text.len()))
}

/// STYLE.md Terminology table rows as variant groups `[use, not…]` for the
/// terminology-consistency rule (SPEC §10.2).
pub fn glossary_groups(root: &Path, cfg: &Value) -> Vec<Vec<String>> {
    let path = glossary_path(root, cfg);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Some((start, end)) = terminology_section(&text) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text[start..end].lines() {
        let t = line.trim();
        if !t.starts_with('|') {
            continue;
        }
        let cells: Vec<String> = t
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim().to_string())
            .collect();
        if cells.is_empty() || cells[0].is_empty() {
            continue;
        }
        // Skip header and separator rows.
        if cells[0].eq_ignore_ascii_case("use")
            || cells[0].chars().all(|c| matches!(c, '-' | ':' | ' '))
        {
            continue;
        }
        let mut group = vec![cells[0].clone()];
        if let Some(not) = cells.get(1) {
            for v in not.split(',') {
                let v = v.trim();
                if !v.is_empty() {
                    group.push(v.to_string());
                }
            }
        }
        out.push(group);
    }
    out
}

/// Built-in variant families used by the deterministic harvest scan.
const HARVEST_PAIRS: &[&[&str]] = &[
    &["login", "log in", "log-in"],
    &["signin", "sign in", "sign-in"],
    &["signup", "sign up", "sign-up"],
    &["setup", "set up", "set-up"],
    &["email", "e-mail"],
    &["backend", "back end", "back-end"],
    &["frontend", "front end", "front-end"],
    &["website", "web site"],
    &["filename", "file name"],
    &["dataset", "data set"],
    &["codebase", "code base"],
    &["username", "user name"],
    &["timeout", "time out", "time-out"],
    &["wifi", "wi-fi"],
    &["realtime", "real time", "real-time"],
    &["opensource", "open source", "open-source"],
];

fn glossary_harvest(root: &Path, cfg: &Value) -> Result<i32> {
    println!("glossary harvest is agent-driven: mine canonical terms and observed variants");
    println!("from the repo and knowledge base, then propose Use/Not rows and confirm them");
    println!("with: mari glossary add <term> --use \"<canonical>\" --not \"<variants,…>\"");
    println!();
    // Deterministic assist: scan repo markdown for known variant families with
    // two or more spellings present, so the agent has concrete candidates.
    let mut seen = repo_glossary_harvest_seen(root);
    merge_harvest_seen(
        &mut seen,
        catalog_glossary_harvest_seen(&catalog_paths(root))?,
    );
    let existing: HashSet<String> = glossary_groups(root, cfg)
        .into_iter()
        .flatten()
        .map(|s| s.to_lowercase())
        .collect();
    let mut proposed = 0;
    for group in HARVEST_PAIRS {
        if let Some(found) = seen.get(group[0]) {
            if found.len() >= 2 && !group.iter().any(|t| existing.contains(&t.to_lowercase())) {
                let mut variants: Vec<&str> = found.iter().copied().collect();
                variants.sort();
                println!(
                    "candidate: {} — variants seen: {}",
                    group[0],
                    variants.join(", ")
                );
                proposed += 1;
            }
        }
    }
    if proposed == 0 {
        println!("no candidate variant pairs found in repo markdown.");
    } else {
        println!("{proposed} candidate(s) — review and add the ones your team approves.");
    }
    Ok(0)
}

fn repo_glossary_harvest_seen(root: &Path) -> HashMap<&'static str, HashSet<&'static str>> {
    let files = crate::detector::runner::collect_files(&[root.to_string_lossy().to_string()]);
    let mut seen = HashMap::new();
    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else {
            continue;
        };
        collect_glossary_harvest_terms(&text, &mut seen);
    }
    seen
}

fn catalog_glossary_harvest_seen(
    paths: &[PathBuf],
) -> Result<HashMap<&'static str, HashSet<&'static str>>> {
    let mut seen = HashMap::new();
    for path in paths {
        let Some(conn) = index::open_readonly_path(path)? else {
            continue;
        };
        let mut stmt = conn.prepare("SELECT COALESCE(body, '') FROM documents")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for text in rows.flatten() {
            collect_glossary_harvest_terms(&text, &mut seen);
        }
    }
    Ok(seen)
}

fn collect_glossary_harvest_terms(
    text: &str,
    seen: &mut HashMap<&'static str, HashSet<&'static str>>,
) {
    let lower = text.to_lowercase();
    for group in HARVEST_PAIRS {
        for term in *group {
            if word_present(&lower, term) {
                seen.entry(group[0]).or_default().insert(term);
            }
        }
    }
}

fn merge_harvest_seen(
    left: &mut HashMap<&'static str, HashSet<&'static str>>,
    right: HashMap<&'static str, HashSet<&'static str>>,
) {
    for (key, values) in right {
        left.entry(key).or_default().extend(values);
    }
}

/// Case-insensitive whole-word presence (haystack must already be lowercase).
fn word_present(lower: &str, term: &str) -> bool {
    let term = term.to_lowercase();
    let bytes = lower.as_bytes();
    let mut from = 0;
    while let Some(pos) = lower[from..].find(&term) {
        let start = from + pos;
        let end = start + term.len();
        let before_ok = start == 0 || !(bytes[start - 1] as char).is_ascii_alphanumeric();
        let after_ok = end >= lower.len() || !(bytes[end] as char).is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

// ---------------------------------------------------------------------------
// mari facts
// ---------------------------------------------------------------------------

fn facts_path(root: &Path, cfg: &Value) -> PathBuf {
    root.join(cfg["facts"]["file"].as_str().unwrap_or("FACTS.md"))
}

pub fn facts(args: &[String], source: Option<&str>) -> Result<i32> {
    facts_in(&workspace::work_root(), args, source)
}

fn facts_in(root: &Path, args: &[String], source: Option<&str>) -> Result<i32> {
    let cfg = resolved(root);
    let path = facts_path(root, &cfg);
    match args.first().map(|s| s.as_str()) {
        None | Some("list") => {
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    let mut n = 0;
                    for line in text.lines() {
                        if !line.trim().is_empty() {
                            println!("{line}");
                            n += 1;
                        }
                    }
                    println!("{n} fact(s) in {}", path.display());
                }
                Err(_) => println!(
                    "no facts yet — add with: mari facts add \"<fact>\" [--source \"<ref>\"]"
                ),
            }
            Ok(0)
        }
        Some("add") => {
            let Some(fact) = args.get(1) else {
                eprintln!("usage: mari facts add \"<fact>\" [--source \"<ref>\"]");
                return Ok(2);
            };
            let fact = fact.trim();
            if fact.is_empty() {
                eprintln!("usage: mari facts add \"<fact>\" [--source \"<ref>\"]");
                return Ok(2);
            }
            let line = match source {
                Some(s) => format!("- {}  ({})\n", fact, s.trim()),
                None => format!("- {}\n", fact),
            };
            let mut text = std::fs::read_to_string(&path).unwrap_or_default();
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&line);
            std::fs::write(&path, text)?;
            mirror_fact_to_catalogs(root, fact, source)?;
            println!("✓ added fact to {}", path.display());
            Ok(0)
        }
        Some(other) => {
            eprintln!("✗ unknown facts subcommand '{other}' — expected list | add");
            Ok(2)
        }
    }
}

fn mirror_fact_to_catalogs(root: &Path, claim: &str, source_ref: Option<&str>) -> Result<()> {
    mirror_fact_to_catalog_paths(
        &catalog_paths(root),
        claim,
        source_ref,
        &author_in(root),
        &chrono::Utc::now().to_rfc3339(),
    )
}

fn mirror_fact_to_catalog_paths(
    paths: &[PathBuf],
    claim: &str,
    source_ref: Option<&str>,
    created_by: &str,
    created_at: &str,
) -> Result<()> {
    let claim = claim.trim();
    if claim.is_empty() {
        return Ok(());
    }
    let source_ref = source_ref.map(str::trim).filter(|s| !s.is_empty());
    let fact_id = index::hash_hex(&format!("fact:{claim}:{}", source_ref.unwrap_or("")));
    let metadata = json!({"source": "FACTS.md"}).to_string();
    for path in paths {
        if !index::warehouse_published_at(path) {
            continue;
        }
        let conn = index::open_catalog_at(path)?;
        conn.execute("DELETE FROM facts WHERE fact_id = ?1", [&fact_id])?;
        conn.execute(
            "INSERT INTO facts (fact_id, claim, source_ref, source_span_id, status, created_by, created_at, metadata_json)
             VALUES (?1, ?2, ?3, NULL, 'accepted', ?4, ?5, ?6)",
            duckdb::params![fact_id, claim, source_ref, created_by, created_at, metadata],
        )?;
        index::publish_to_path(&conn, path)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// mari extract facts
// ---------------------------------------------------------------------------

/// True when a sentence carries a typed span worth grounding: number, date,
/// money, or percent.
fn has_typed_span(s: &str) -> bool {
    let chars = s.char_indices();
    for (i, c) in chars {
        match c {
            '$' | '€' | '£' => {
                if s[i + c.len_utf8()..]
                    .trim_start()
                    .starts_with(|d: char| d.is_ascii_digit())
                {
                    return true;
                }
            }
            '0'..='9' => {
                // percent, 4-digit year, or any standalone number
                let rest = &s[i..];
                let num_len = rest
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
                    .count();
                let after = rest[..].chars().nth(num_len);
                if after == Some('%') {
                    return true;
                }
                let digits: String = rest
                    .chars()
                    .take(num_len)
                    .filter(|c| c.is_ascii_digit())
                    .collect();
                if digits.len() == 4 && (digits.starts_with("19") || digits.starts_with("20")) {
                    return true;
                }
                if !digits.is_empty() {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// Crude sentence split for candidate mining.
fn sentences(text: &str) -> Vec<String> {
    sentence_candidates(text)
        .into_iter()
        .map(|(_, sentence)| sentence)
        .collect()
}

fn sentence_candidates(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with('|') || t.starts_with("```") {
            continue;
        }
        for s in t.split_inclusive(['.', '!', '?']) {
            let s = s.trim().trim_start_matches(['-', '*']).trim();
            if s.chars().filter(|c| c.is_alphabetic()).count() >= 10 {
                out.push((idx + 1, s.to_string()));
            }
        }
    }
    out
}

#[derive(Clone, Debug, serde::Serialize)]
struct CandidateFact {
    source: String,
    #[serde(rename = "ref")]
    reference: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
}

pub fn extract(
    args: &[String],
    source: Option<&str>,
    doc: Option<&str>,
    since: Option<i64>,
    json: bool,
) -> Result<i32> {
    extract_in(&workspace::work_root(), args, source, doc, since, json)
}

fn extract_in(
    root: &Path,
    args: &[String],
    source: Option<&str>,
    doc: Option<&str>,
    since: Option<i64>,
    json_out: bool,
) -> Result<i32> {
    if let Some(first) = args.first() {
        if first != "facts" {
            eprintln!(
                "usage: mari extract facts [--source <key>] [--doc <substr>] [--since D] [--json]"
            );
            return Ok(2);
        }
    }
    if let Some(source) = source {
        if !authcmd::SOURCES.contains(&source) {
            eprintln!("✗ unknown source: {source}");
            return Ok(2);
        }
    }
    let cutoff = since.map(cutoff_rfc3339);
    let candidates =
        if let Some(candidates) = extract_catalog_candidates(source, doc, cutoff.as_deref())? {
            candidates
        } else {
            if source.is_some() {
                eprintln!("note: no catalog yet — --source ignored; scanning repo markdown.");
            }
            extract_repo_candidates(root, doc, since)
        };
    print_candidate_facts(candidates, json_out)
}

fn cutoff_rfc3339(days: i64) -> String {
    (chrono::Utc::now() - chrono::Duration::days(days.max(0))).to_rfc3339()
}

fn extract_catalog_candidates(
    source: Option<&str>,
    doc: Option<&str>,
    cutoff: Option<&str>,
) -> Result<Option<Vec<CandidateFact>>> {
    let mut paths = vec![
        index::catalog_path(false),
        workspace::global_workspace_dir().join(index::CATALOG_FILE),
    ];
    paths.sort();
    paths.dedup();
    let paths: Vec<PathBuf> = paths
        .into_iter()
        .filter(|p| index::warehouse_published_at(p))
        .collect();
    if paths.is_empty() {
        return Ok(None);
    }
    Ok(Some(extract_catalog_candidates_from_paths(
        &paths, source, doc, cutoff,
    )?))
}

fn extract_catalog_candidates_from_paths(
    paths: &[PathBuf],
    source: Option<&str>,
    doc: Option<&str>,
    cutoff: Option<&str>,
) -> Result<Vec<CandidateFact>> {
    let doc_filter = doc.map(|d| d.to_lowercase());
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for path in paths {
        let Some(conn) = index::open_readonly_path(path)? else {
            continue;
        };
        let mut stmt = conn.prepare(
            "SELECT source_id, canonical_ref, COALESCE(title, ''), COALESCE(path, ''), COALESCE(updated_at, ''), body FROM documents",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })?;
        for row in rows.flatten() {
            let (source_id, reference, title, path, updated_at, body) = row;
            if source.is_some_and(|s| s != source_id) {
                continue;
            }
            if let Some(cutoff) = cutoff {
                if !updated_at.is_empty() && updated_at.as_str() < cutoff {
                    continue;
                }
            }
            if let Some(doc_filter) = &doc_filter {
                let haystack = format!("{reference}\n{title}\n{path}").to_lowercase();
                if !haystack.contains(doc_filter) {
                    continue;
                }
            }
            for (line, sentence) in sentence_candidates(&body) {
                if !has_typed_span(&sentence) {
                    continue;
                }
                let key = format!("{source_id}\0{reference}\0{sentence}");
                if seen.insert(key) {
                    candidates.push(CandidateFact {
                        source: source_id.clone(),
                        reference: reference.clone(),
                        text: sentence,
                        line: Some(line),
                        updated_at: if updated_at.is_empty() {
                            None
                        } else {
                            Some(updated_at.clone())
                        },
                    });
                }
            }
        }
    }
    Ok(candidates)
}

fn extract_repo_candidates(
    root: &Path,
    doc: Option<&str>,
    since: Option<i64>,
) -> Vec<CandidateFact> {
    let files = crate::detector::runner::collect_files(&[root.to_string_lossy().to_string()]);
    let cutoff = since.map(|d| {
        std::time::SystemTime::now() - std::time::Duration::from_secs(d.max(0) as u64 * 86_400)
    });
    let mut candidates = Vec::new();
    for f in &files {
        let rel = f
            .strip_prefix(root)
            .unwrap_or(f)
            .to_string_lossy()
            .to_string();
        if let Some(d) = doc {
            if !rel.to_lowercase().contains(&d.to_lowercase()) {
                continue;
            }
        }
        if let Some(cut) = cutoff {
            if let Ok(meta) = std::fs::metadata(f) {
                if meta.modified().map(|m| m < cut).unwrap_or(false) {
                    continue;
                }
            }
        }
        let Ok(text) = std::fs::read_to_string(f) else {
            continue;
        };
        for (line, s) in sentence_candidates(&text) {
            if has_typed_span(&s) {
                candidates.push(CandidateFact {
                    source: "localfiles".into(),
                    reference: rel.clone(),
                    text: s,
                    line: Some(line),
                    updated_at: None,
                });
            }
        }
    }
    candidates
}

fn print_candidate_facts(candidates: Vec<CandidateFact>, json_out: bool) -> Result<i32> {
    if json_out {
        println!("{}", serde_json::to_string_pretty(&candidates)?);
        return Ok(0);
    }
    if candidates.is_empty() {
        println!("no candidate facts found.");
        return Ok(0);
    }
    println!(
        "candidate facts — review, then accept with: mari facts add \"<fact>\" --source \"<ref>\""
    );
    for c in &candidates {
        let line = c.line.map(|line| format!(":L{line}")).unwrap_or_default();
        println!("- {}  ({}{})", c.text, c.reference, line);
    }
    println!("{} candidate(s)", candidates.len());
    Ok(0)
}

// ---------------------------------------------------------------------------
// mari audit kb
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
struct KbFinding {
    severity: &'static str, // error | warn | advisory
    rule: &'static str,
    file: String,
    message: String,
}

const STALE_DAYS: u64 = 90;

pub fn audit_kb(paths: &[String], json: bool, strict: bool) -> Result<i32> {
    audit_kb_in(&workspace::work_root(), paths, json, strict)
}

fn audit_kb_in(root: &Path, paths: &[String], json_out: bool, strict: bool) -> Result<i32> {
    let cfg = resolved(root);
    let roots: Vec<String> = if paths.is_empty() {
        vec![root.to_string_lossy().to_string()]
    } else {
        paths.to_vec()
    };
    let files = crate::detector::runner::collect_files(&roots);
    let mut findings: Vec<KbFinding> = Vec::new();
    let rel_of = |f: &Path| {
        f.strip_prefix(root)
            .unwrap_or(f)
            .to_string_lossy()
            .to_string()
    };

    findings.extend(catalog_contradiction_findings(root)?);

    // Per-file text cache.
    let texts: Vec<(PathBuf, String)> = files
        .iter()
        .filter_map(|f| std::fs::read_to_string(f).ok().map(|t| (f.clone(), t)))
        .collect();

    // 1. Stale pages (mtime older than the threshold). OPT-IN: filesystem
    // mtime is the checkout/clone time on a fresh git working tree, not the
    // real last-edit time, so this fires on every file and is off by default.
    // Enable with `audit.stale_pages = true`; tune with `audit.stale_days`.
    if cfg["audit"]["stale_pages"].as_bool().unwrap_or(false) {
        let stale_days = cfg["audit"]["stale_days"].as_u64().unwrap_or(STALE_DAYS);
        let now = std::time::SystemTime::now();
        for (f, _) in &texts {
            if let Ok(modified) = std::fs::metadata(f).and_then(|m| m.modified()) {
                if let Ok(age) = now.duration_since(modified) {
                    let days = age.as_secs() / 86_400;
                    if days > stale_days {
                        findings.push(KbFinding {
                            severity: "warn",
                            rule: "stale-page",
                            file: rel_of(f),
                            message: format!("not updated in {days} days (threshold {stale_days})"),
                        });
                    }
                }
            }
        }
    }

    // 2. needs-review backlog from the catalog `tags` table.
    let mut needs_review_seen = HashSet::new();
    findings.extend(catalog_needs_review_findings(root, &mut needs_review_seen)?);

    // 3. Inconsistent terminology: >=2 spellings of a glossary group in one file.
    let groups = glossary_groups(root, &cfg);
    let gpath = glossary_path(root, &cfg);
    for (f, text) in &texts {
        if *f == gpath || f.file_name() == gpath.file_name() {
            continue; // the glossary itself legitimately lists the variants
        }
        let lower = text.to_lowercase();
        for g in &groups {
            let present: Vec<&String> = g.iter().filter(|t| word_present(&lower, t)).collect();
            if present.len() >= 2 {
                findings.push(KbFinding {
                    severity: "warn",
                    rule: "inconsistent-terminology",
                    file: rel_of(f),
                    message: format!(
                        "mixes {} — glossary says use \"{}\"",
                        present
                            .iter()
                            .map(|s| format!("\"{s}\""))
                            .collect::<Vec<_>>()
                            .join(" and "),
                        g[0]
                    ),
                });
            }
        }
    }

    // 4. Duplicated content: identical normalized paragraphs across files.
    let mut paras: HashMap<String, HashSet<String>> = HashMap::new();
    for (f, text) in &texts {
        for para in text.split("\n\n") {
            let norm = para
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase();
            if norm.len() >= 80 && !norm.starts_with('#') && !norm.starts_with('|') {
                paras.entry(norm).or_default().insert(rel_of(f));
            }
        }
    }
    let mut dup_pairs: Vec<Vec<String>> = paras
        .into_values()
        .filter(|files| files.len() >= 2)
        .map(|files| {
            let mut v: Vec<String> = files.into_iter().collect();
            v.sort();
            v
        })
        .collect();
    dup_pairs.sort();
    dup_pairs.dedup();
    for files in dup_pairs {
        findings.push(KbFinding {
            severity: "warn",
            rule: "duplicate-content",
            file: files[0].clone(),
            message: format!("identical paragraph also in {}", files[1..].join(", ")),
        });
    }

    // 5. Missing links: no outbound markdown links and no inbound references.
    let mut inbound: HashSet<String> = HashSet::new();
    for (_, text) in &texts {
        let mut rest = text.as_str();
        while let Some(pos) = rest.find("](") {
            let after = &rest[pos + 2..];
            let end = after.find(')').unwrap_or(after.len());
            let target = after[..end].split('#').next().unwrap_or("").trim();
            if !target.is_empty() {
                if let Some(base) = Path::new(target).file_name().and_then(|b| b.to_str()) {
                    inbound.insert(base.to_lowercase());
                }
            }
            rest = &after[end.min(after.len())..];
        }
    }
    for (f, text) in &texts {
        let has_outbound = text.contains("](");
        let base = f
            .file_name()
            .and_then(|b| b.to_str())
            .unwrap_or("")
            .to_lowercase();
        let has_inbound = inbound.contains(&base);
        if !has_outbound && !has_inbound {
            findings.push(KbFinding {
                severity: "advisory",
                rule: "missing-links",
                file: rel_of(f),
                message: "no inbound or outbound markdown links (orphan page)".into(),
            });
        }
    }

    // 6. Unsupported claims: typed-span sentences with no citation nearby.
    for (f, text) in &texts {
        let mut n = 0;
        let mut example = String::new();
        for s in sentences(text) {
            let cited = s.contains("](")
                || s.contains("(source")
                || s.contains("(see ")
                || s.contains("[^");
            if has_typed_span(&s) && !cited {
                n += 1;
                if example.is_empty() {
                    example = s;
                }
            }
        }
        if n > 0 {
            let mut ex = example;
            if ex.len() > 100 {
                ex.truncate(100);
                ex.push('…');
            }
            findings.push(KbFinding {
                severity: "advisory",
                rule: "unsupported-claim",
                file: rel_of(f),
                message: format!(
                    "{n} sentence(s) with numbers/dates/money and no citation, e.g. \"{ex}\""
                ),
            });
        }
    }

    // 7. PRODUCT.md divergence: explicit banned/forbidden terms only.
    let banned = product_banned_terms(root);
    if !banned.is_empty() {
        let product_path = root.join("PRODUCT.md");
        for (f, text) in &texts {
            if *f == product_path || f.file_name() == Some(std::ffi::OsStr::new("PRODUCT.md")) {
                continue;
            }
            let lower = text.to_lowercase();
            let hits: Vec<&String> = banned
                .iter()
                .filter(|term| word_present(&lower, term))
                .collect();
            if !hits.is_empty() {
                findings.push(KbFinding {
                    severity: "warn",
                    rule: "product-divergence",
                    file: rel_of(f),
                    message: format!(
                        "uses PRODUCT.md banned term(s): {}",
                        hits.iter()
                            .map(|s| format!("\"{s}\""))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                });
            }
        }
    }

    // Prioritize errors → warns → advisories.
    let rank = |s: &str| match s {
        "error" => 0,
        "warn" => 1,
        _ => 2,
    };
    findings.sort_by(|a, b| {
        rank(a.severity)
            .cmp(&rank(b.severity))
            .then(a.file.cmp(&b.file))
            .then(a.rule.cmp(b.rule))
    });
    let errors = findings.iter().filter(|f| f.severity == "error").count();
    let warns = findings.iter().filter(|f| f.severity == "warn").count();
    let advisories = findings.iter().filter(|f| f.severity == "advisory").count();

    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "findings": findings,
                "summary": { "errors": errors, "warns": warns, "advisories": advisories, "files": texts.len() }
            }))?
        );
    } else if findings.is_empty() {
        println!(
            "✓ knowledge base audit clean — {} file(s) checked.",
            texts.len()
        );
    } else {
        println!("knowledge base audit — {} file(s), {errors} error(s), {warns} warn(s), {advisories} advisor{}:", texts.len(), if advisories == 1 { "y" } else { "ies" });
        for f in &findings {
            println!("  [{}] {} — {}: {}", f.severity, f.file, f.rule, f.message);
        }
        println!("report only — mari does not edit.");
    }

    if errors > 0 || (strict && warns > 0) {
        Ok(1)
    } else {
        Ok(0)
    }
}

fn product_banned_terms(root: &Path) -> Vec<String> {
    let path = root.join("PRODUCT.md");
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut terms = Vec::new();
    let mut in_section = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let heading = trimmed.trim_start_matches('#').trim().to_ascii_lowercase();
            in_section = heading.contains("banned") || heading.contains("forbidden");
            continue;
        }
        if !in_section || trimmed.is_empty() || trimmed.starts_with('|') {
            continue;
        }
        let cleaned = trimmed
            .trim_start_matches(['-', '*'])
            .trim()
            .trim_matches('`');
        for term in cleaned.split([',', ';']) {
            let term = term
                .trim()
                .trim_matches(['`', '"', '\'', '.'])
                .to_ascii_lowercase();
            if term.len() >= 3 && !terms.contains(&term) {
                terms.push(term);
            }
        }
    }
    terms
}

#[derive(Debug)]
struct CatalogClaim {
    reference: String,
    spans: BTreeSet<String>,
    terms: BTreeSet<String>,
}

fn catalog_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = vec![
        workspace::workspace_dir(root).join(index::CATALOG_FILE),
        workspace::global_workspace_dir().join(index::CATALOG_FILE),
    ];
    paths.sort();
    paths.dedup();
    // Each path names an Iceberg warehouse (sibling `iceberg/` dir), not a file
    // on disk — there is no catalog.duckdb anymore (§8.8). The mirror helpers
    // resolve each path to its warehouse and skip any that is unpublished.
    paths
}

fn catalog_contradiction_findings(root: &Path) -> Result<Vec<KbFinding>> {
    catalog_contradiction_findings_from_paths(&catalog_paths(root))
}

fn catalog_needs_review_findings(
    root: &Path,
    seen: &mut HashSet<String>,
) -> Result<Vec<KbFinding>> {
    catalog_needs_review_findings_from_paths(&catalog_paths(root), seen)
}

fn catalog_needs_review_findings_from_paths(
    paths: &[PathBuf],
    seen: &mut HashSet<String>,
) -> Result<Vec<KbFinding>> {
    let mut findings = Vec::new();
    for path in paths {
        let Some(conn) = index::open_readonly_path(path)? else {
            continue;
        };
        let mut stmt = conn.prepare(
            "SELECT t.target_id,
                    COALESCE(d.path, d.canonical_ref, t.target_id),
                    COALESCE(t.\"by\", ''),
                    COALESCE(t.\"at\", ''),
                    COALESCE(t.note, ''),
                    t.metadata_json
               FROM tags t
               LEFT JOIN documents d ON t.target_type = 'doc' AND d.doc_id = t.target_id
              WHERE t.status = 'needs-review'",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;
        for row in rows.flatten() {
            let (target_id, display, by, at, note, metadata_json) = row;
            let target = mirrored_tag_target(&metadata_json).unwrap_or(display);
            let key = norm_ref(&target);
            if !seen.insert(key.clone()) {
                continue;
            }
            let mut message = format!(
                "flagged needs-review by {} on {}",
                empty_as_unknown(&by),
                empty_as_unknown(&at)
            );
            if !note.trim().is_empty() {
                message.push_str(&format!(" — {}", note.trim()));
            }
            findings.push(KbFinding {
                severity: "warn",
                rule: "needs-review",
                file: if key.is_empty() { target_id } else { key },
                message,
            });
        }
    }
    Ok(findings)
}

fn mirrored_tag_target(metadata_json: &str) -> Option<String> {
    serde_json::from_str::<Value>(metadata_json)
        .ok()
        .and_then(|v| v["target"].as_str().map(norm_ref))
}

fn empty_as_unknown(s: &str) -> &str {
    if s.trim().is_empty() {
        "?"
    } else {
        s
    }
}

/// The kind of a typed span for contradiction comparison: money, percent,
/// year, or count. Only same-kind spans are comparable.
fn span_kind(s: &str) -> &'static str {
    let t = s.trim();
    if t.starts_with('$') {
        "money"
    } else if t.ends_with('%') || t.contains("percent") {
        "percent"
    } else {
        let digits: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() == 4 && (digits.starts_with("19") || digits.starts_with("20")) {
            "year"
        } else {
            "count"
        }
    }
}

/// Numeric magnitude of a span (commas/currency stripped), for agreement check.
fn span_value(s: &str) -> Option<f64> {
    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    cleaned.parse::<f64>().ok()
}

/// If the two span sets have a same-kind pair with *different* values, return
/// that pair (a, b). Returns None when every shared kind agrees or no kind is
/// shared — i.e. no genuine contradiction.
fn conflicting_span_pair(a: &BTreeSet<String>, b: &BTreeSet<String>) -> Option<(String, String)> {
    use std::collections::BTreeMap;
    let by_kind = |set: &BTreeSet<String>| -> BTreeMap<&'static str, Vec<String>> {
        let mut m: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
        for v in set {
            m.entry(span_kind(v)).or_default().push(v.clone());
        }
        m
    };
    let am = by_kind(a);
    let bm = by_kind(b);
    for (kind, avals) in &am {
        // High-precision kinds only: money and percent conflicts are almost
        // always genuine. Bare counts (30 days vs 7 hours vs 500 members) and
        // years are too ambiguous without unit/NLI awareness — comparing them
        // produces noise, so they don't raise a deterministic contradiction.
        if *kind != "money" && *kind != "percent" {
            continue;
        }
        let Some(bvals) = bm.get(kind) else { continue };
        for av in avals {
            for bv in bvals {
                let agree = match (span_value(av), span_value(bv)) {
                    (Some(x), Some(y)) => (x - y).abs() < f64::EPSILON,
                    _ => av == bv,
                };
                if !agree {
                    return Some((av.clone(), bv.clone()));
                }
            }
        }
    }
    None
}

/// A claim can only take part in a contradiction if it carries a span of a
/// high-precision kind (money or percent) — `conflicting_span_pair` ignores
/// every other kind. Filtering to these before the pairwise scan is what keeps
/// `audit kb` from being O(claims²) over the whole catalog: doc corpora are
/// dominated by bare numbers, version identifiers, and dates, none of which can
/// ever conflict.
fn claim_has_precision_kind(spans: &BTreeSet<String>) -> bool {
    spans
        .iter()
        .any(|v| matches!(span_kind(v), "money" | "percent"))
}

fn catalog_contradiction_findings_from_paths(paths: &[PathBuf]) -> Result<Vec<KbFinding>> {
    let claims: Vec<CatalogClaim> = catalog_claims_from_paths(paths)?
        .into_iter()
        .filter(|c| claim_has_precision_kind(&c.spans))
        .collect();
    let mut findings = Vec::new();
    let mut seen = HashSet::new();
    for i in 0..claims.len() {
        for j in (i + 1)..claims.len() {
            let a = &claims[i];
            let b = &claims[j];
            if a.reference == b.reference || a.spans == b.spans {
                continue;
            }
            let overlap = a.terms.intersection(&b.terms).count();
            if overlap < 2 {
                continue;
            }
            // A genuine contradiction needs a same-KIND span with different
            // values: money vs money, percent vs percent, year vs year. A
            // price ($49) and a customer count (6625) are different kinds and
            // never contradict, even if both docs mention "seat". And if the
            // two claims agree on a value within a kind, that kind isn't a
            // conflict. This is the precision gate.
            let Some((a_val, b_val)) = conflicting_span_pair(&a.spans, &b.spans) else {
                continue;
            };
            let _ = (&a_val, &b_val);
            let key = if a.reference <= b.reference {
                format!(
                    "{}\0{}\0{:?}\0{:?}",
                    a.reference, b.reference, a.spans, b.spans
                )
            } else {
                format!(
                    "{}\0{}\0{:?}\0{:?}",
                    b.reference, a.reference, b.spans, a.spans
                )
            };
            if !seen.insert(key) {
                continue;
            }
            findings.push(KbFinding {
                severity: "warn",
                rule: "contradiction-candidate",
                file: a.reference.clone(),
                message: format!(
                    "conflicting values vs {}: {} vs {} (same kind, different value)",
                    b.reference, a_val, b_val
                ),
            });
            if findings.len() >= 50 {
                return Ok(findings);
            }
        }
    }
    Ok(findings)
}

fn catalog_claims_from_paths(paths: &[PathBuf]) -> Result<Vec<CatalogClaim>> {
    let mut claims = Vec::new();
    let mut seen = HashSet::new();
    for path in paths {
        let Some(conn) = index::open_readonly_path(path)? else {
            continue;
        };
        let mut stmt = conn.prepare("SELECT canonical_ref, body FROM documents")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows.flatten() {
            let (reference, body) = row;
            for sentence in sentences(&body) {
                let spans = typed_spans_for_audit(&sentence);
                if spans.is_empty() {
                    continue;
                }
                let key = format!("{reference}\0{sentence}");
                if seen.insert(key) {
                    claims.push(CatalogClaim {
                        reference: reference.clone(),
                        terms: salient_terms_for_audit(&sentence),
                        spans,
                    });
                }
            }
        }
    }
    Ok(claims)
}

fn typed_spans_for_audit(text: &str) -> BTreeSet<String> {
    Regex::new(
        r"(?x)
        (?:[$€£]\s?\d[\d,]*(?:\.\d+)?)
        |(?:\b\d[\d,]*(?:\.\d+)?\s?%)
        |(?:\b(?:19|20)\d{2}\b)
        |(?:\b\d{1,2}[/-]\d{1,2}[/-]\d{2,4}\b)
        |(?:\b(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Sept|Oct|Nov|Dec)[a-z]*\.?\s+\d{1,2},?\s+(?:19|20)\d{2}\b)
        |(?:\b\d[\d,]*(?:\.\d+)?\b)
        ",
    )
    .unwrap()
    .find_iter(text)
    .map(|m| {
        m.as_str()
            .trim()
            .trim_end_matches(['.', ',', ';', ':'])
            .replace(' ', "")
            .to_ascii_lowercase()
    })
    .collect()
}

fn salient_terms_for_audit(text: &str) -> BTreeSet<String> {
    const STOP: &[&str] = &[
        "the", "a", "an", "and", "or", "to", "of", "in", "on", "for", "with", "by", "is", "are",
        "was", "were", "be", "been", "this", "that", "it", "we", "our", "from", "after", "before",
        "per", "as", "at",
    ];
    let stop: BTreeSet<&str> = STOP.iter().copied().collect();
    Regex::new(r"[A-Za-z][A-Za-z0-9_-]{2,}")
        .unwrap()
        .find_iter(text)
        .map(|m| m.as_str().to_ascii_lowercase())
        .filter(|w| !stop.contains(w.as_str()))
        .collect()
}

// ---------------------------------------------------------------------------
// mari humanize — vendored skill management (SPEC §5.4)
// ---------------------------------------------------------------------------

/// Default upstream for the vendored humanizer skill. Overridable via the
/// `humanizer.repo` config key when a team hosts their own. Set to empty to
/// disable the `humanize` command's clone/update.
const HUMANIZER_REPO_DEFAULT: &str = "";

fn humanizer_repo() -> String {
    let cfg = config::resolve(Some(&workspace::work_root()));
    cfg["humanizer"]["repo"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(HUMANIZER_REPO_DEFAULT)
        .to_string()
}

fn humanizer_dir() -> PathBuf {
    config::mari_home().join("skills").join("humanizer")
}

fn git_in(dir: &Path, args: &[&str]) -> Result<(bool, String, String)> {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output()?;
    Ok((
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
        String::from_utf8_lossy(&out.stderr).trim().to_string(),
    ))
}

pub fn humanize(action: Option<&str>, json: bool) -> Result<i32> {
    let dir = humanizer_dir();
    let skill = dir.join("SKILL.md");
    match action.unwrap_or("ensure") {
        "ensure" => {
            if !dir.join(".git").exists() {
                let repo = humanizer_repo();
                if repo.is_empty() {
                    eprintln!(
                        "✗ no humanizer upstream configured — set one with \
                         `mari config set humanizer.repo <git-url>` (the vendored \
                         humanizer skill has no default upstream in this build)"
                    );
                    if json {
                        println!(
                            "{}",
                            json!({ "ok": false, "error": "humanizer.repo not set" })
                        );
                    }
                    return Ok(1);
                }
                std::fs::create_dir_all(dir.parent().unwrap())?;
                let out = Command::new("git")
                    .args(["clone", "--depth", "1", &repo])
                    .arg(&dir)
                    .output()?;
                if !out.status.success() {
                    let err = String::from_utf8_lossy(&out.stderr);
                    eprintln!("✗ clone failed: {}", err.trim());
                    if json {
                        println!("{}", json!({ "ok": false, "error": err.trim() }));
                    }
                    return Ok(1);
                }
            }
            if json {
                println!("{}", json!({ "ok": true, "path": skill.to_string_lossy() }));
            } else {
                println!("{}", skill.display());
            }
            Ok(0)
        }
        "update" => {
            if !dir.join(".git").exists() {
                eprintln!("✗ humanizer skill not installed — run: mari humanize ensure");
                return Ok(1);
            }
            let (ok_f, _, err_f) = git_in(&dir, &["fetch", "--depth", "1", "origin"])?;
            if !ok_f {
                eprintln!("✗ fetch failed: {err_f}");
                return Ok(1);
            }
            let (ok_r, _, err_r) = git_in(&dir, &["reset", "--hard", "origin/HEAD"])?;
            if !ok_r {
                eprintln!("✗ reset failed: {err_r}");
                return Ok(1);
            }
            let (_, rev, _) = git_in(&dir, &["rev-parse", "HEAD"])?;
            if json {
                println!("{}", json!({ "ok": true, "revision": rev }));
            } else {
                println!("✓ humanizer updated to {rev}");
            }
            Ok(0)
        }
        "status" => {
            if !dir.join(".git").exists() {
                if json {
                    println!("{}", json!({ "installed": false }));
                } else {
                    println!("humanizer skill not installed — run: mari humanize ensure");
                }
                return Ok(1);
            }
            let (ok, rev, err) = git_in(&dir, &["rev-parse", "HEAD"])?;
            if !ok {
                eprintln!("✗ {err}");
                return Ok(1);
            }
            if json {
                println!(
                    "{}",
                    json!({ "installed": true, "revision": rev, "path": dir.to_string_lossy() })
                );
            } else {
                println!("{rev}");
            }
            Ok(0)
        }
        other => {
            eprintln!("✗ unknown humanize action '{other}' — expected ensure | update | status");
            Ok(2)
        }
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    /// Publish a catalog at `path` holding one doc (`doc1`, path `docs/api.md`).
    fn catalog_with_api_doc(path: &Path) {
        let conn = duckdb::Connection::open_in_memory().unwrap();
        index::ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO documents (
                doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind,
                author_id, author_name, created_at, updated_at, observed_at, version,
                content_sha256, body, metadata_json
            ) VALUES ('doc1', 'git', 'docs/api.md', 'git:docs/api.md', 'API', '', 'docs/api.md',
                'text/markdown', 'doc', '', '', NULL, NULL, 'now', '1', 'sha', 'body', '{}')",
            [],
        )
        .unwrap();
        index::publish_to_path(&conn, path).unwrap();
    }

    fn tag_status_in(path: &Path, target_type: &str, target_id: &str) -> Option<String> {
        let conn = index::open_readonly_path(path).unwrap().unwrap();
        conn.query_row(
            "SELECT status FROM tags WHERE target_type = ?1 AND target_id = ?2",
            duckdb::params![target_type, target_id],
            |r| r.get::<_, String>(0),
        )
        .ok()
    }

    #[test]
    fn tag_apply_resolves_to_doc_and_removes() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = dir.path().join("catalog.duckdb");
        catalog_with_api_doc(&catalog);
        let paths = std::slice::from_ref(&catalog);

        let entry = json!({ "status": "canonical", "by": "tester", "at": "2026-07-06", "note": "primary" });
        apply_tag(paths, "docs/api.md", &entry).unwrap();

        assert_eq!(
            tag_status_in(&catalog, "doc", "doc1").as_deref(),
            Some("canonical")
        );
        // tag_of resolves, including ./ normalization.
        assert_eq!(tag_of_paths(paths, "docs/api.md").as_deref(), Some("canonical"));
        assert_eq!(tag_of_paths(paths, "./docs/api.md").as_deref(), Some("canonical"));
        assert_eq!(tag_of_paths(paths, "other.md"), None);

        // remove clears it; removing again reports nothing removed.
        assert!(remove_tag(paths, "docs/api.md").unwrap());
        assert_eq!(tag_status_in(&catalog, "doc", "doc1"), None);
        assert!(!remove_tag(paths, "docs/api.md").unwrap());
    }

    #[test]
    fn tag_unknown_status_exits_2_without_catalog() {
        let dir = tempfile::tempdir().unwrap();
        // Bad status is rejected before any catalog is required.
        assert_eq!(
            tag_in(
                dir.path(),
                &s(&["README.md", "totally-bogus-status"]),
                None,
                None,
                false,
                None,
                None,
            )
            .unwrap(),
            2
        );
        assert_eq!(
            tag_in(
                dir.path(),
                &s(&["list"]),
                None,
                Some("totally-bogus-status"),
                false,
                None,
                None,
            )
            .unwrap(),
            2
        );
    }

    #[test]
    fn tag_unresolved_ref_stored_as_ref_tag() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = dir.path().join("catalog.duckdb");
        // Published but empty catalog: the ref resolves nowhere.
        let conn = duckdb::Connection::open_in_memory().unwrap();
        index::ensure_schema(&conn).unwrap();
        index::publish_to_path(&conn, &catalog).unwrap();
        drop(conn);

        let entry = json!({ "status": "draft", "by": "tester", "at": "2026-07-06" });
        apply_tag(std::slice::from_ref(&catalog), "gdocs:launch.plan.v2", &entry).unwrap();

        // Dots in the ref are preserved verbatim as the ref-tag target_id.
        assert_eq!(
            tag_status_in(&catalog, "ref", "gdocs:launch.plan.v2").as_deref(),
            Some("draft")
        );
    }

    #[test]
    fn superseded_by_records_replaces_lineage_edge() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = dir.path().join("catalog.duckdb");
        let conn = duckdb::Connection::open_in_memory().unwrap();
        index::ensure_schema(&conn).unwrap();
        for (doc, path) in [("old", "docs/v1.md"), ("new", "docs/v2.md")] {
            conn.execute(
                "INSERT INTO documents (
                    doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind,
                    author_id, author_name, created_at, updated_at, observed_at, version,
                    content_sha256, body, metadata_json
                ) VALUES (?1, 'git', ?2, ?3, ?2, '', ?2, 'text/markdown', 'doc',
                    '', '', NULL, NULL, 'now', '1', 'sha', 'body', '{}')",
                duckdb::params![doc, path, format!("git:{path}")],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO spans (span_id, doc_id, chunk_id, span_kind, label, start_byte, end_byte, start_line, end_line, stable_hash, metadata_json)
                 VALUES (?1, ?2, NULL, 'section', 'root', 0, 100, 1, 10, 'h', '{}')",
                duckdb::params![format!("span-{doc}"), doc],
            )
            .unwrap();
        }
        index::publish_to_path(&conn, &catalog).unwrap();
        drop(conn);
        let paths = std::slice::from_ref(&catalog);

        let pointer = record_supersession(paths, "docs/v2.md", "docs/v1.md").unwrap();
        assert_eq!(pointer.as_deref(), Some("docs/v2.md"));

        let conn = index::open_readonly_path(&catalog).unwrap().unwrap();
        let (rel, status, from_span, to_span): (String, String, String, String) = conn
            .query_row(
                "SELECT rel, status, from_span_id, to_span_id FROM lineage_edges",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(rel, "replaces");
        assert_eq!(status, "confirmed");
        assert_eq!(from_span, "span-new"); // successor → deprecated
        assert_eq!(to_span, "span-old");
    }

    #[test]
    fn facts_mirror_writes_accepted_catalog_fact() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = dir.path().join("catalog.duckdb");
        let conn = duckdb::Connection::open_in_memory().unwrap();
        index::ensure_schema(&conn).unwrap();
        index::publish_to_path(&conn, &catalog).unwrap();
        drop(conn);

        mirror_fact_to_catalog_paths(
            std::slice::from_ref(&catalog),
            "Latency dropped 40%.",
            Some("git:docs/postmortem.md"),
            "tester",
            "2026-07-06T00:00:00Z",
        )
        .unwrap();

        let conn = index::open_readonly_path(&catalog).unwrap().unwrap();
        let row: (String, String, String, String) = conn
            .query_row(
                "SELECT claim, source_ref, status, created_by FROM facts",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(row.0, "Latency dropped 40%.");
        assert_eq!(row.1, "git:docs/postmortem.md");
        assert_eq!(row.2, "accepted");
        assert_eq!(row.3, "tester");
    }

    #[test]
    fn audit_kb_reads_needs_review_from_catalog_tags() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = dir.path().join("catalog.duckdb");
        let conn = duckdb::Connection::open_in_memory().unwrap();
        index::ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO documents (
                doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind,
                author_id, author_name, created_at, updated_at, observed_at, version,
                content_sha256, body, metadata_json
            ) VALUES ('doc1', 'git', 'docs/review.md', 'git:docs/review.md', 'Review', '', 'docs/review.md',
                'text/markdown', 'doc', '', '', NULL, NULL, 'now', '1', 'sha', 'body', '{}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tags (target_type, target_id, status, note, \"by\", \"at\", metadata_json)
             VALUES ('doc', 'doc1', 'needs-review', 'check claims', 'tester', '2026-07-06', ?1)",
            duckdb::params![
                json!({"source": "tags.entries", "target": "./docs/review.md"}).to_string()
            ],
        )
        .unwrap();
        index::publish_to_path(&conn, &catalog).unwrap();
        drop(conn);

        let mut seen = HashSet::new();
        let findings =
            catalog_needs_review_findings_from_paths(std::slice::from_ref(&catalog), &mut seen)
                .unwrap();

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "needs-review");
        assert_eq!(findings[0].file, "docs/review.md");
        assert!(findings[0].message.contains("check claims"));
    }

    #[test]
    fn audit_kb_dedupes_catalog_needs_review_against_config_target() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = dir.path().join("catalog.duckdb");
        let conn = duckdb::Connection::open(&catalog).unwrap();
        index::ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO tags (target_type, target_id, status, note, \"by\", \"at\", metadata_json)
             VALUES ('doc', 'doc1', 'needs-review', '', 'tester', '2026-07-06', ?1)",
            duckdb::params![
                json!({"source": "tags.entries", "target": "docs/review.md"}).to_string()
            ],
        )
        .unwrap();
        drop(conn);

        let mut seen = HashSet::new();
        seen.insert("docs/review.md".to_string());
        let findings =
            catalog_needs_review_findings_from_paths(std::slice::from_ref(&catalog), &mut seen)
                .unwrap();

        assert!(findings.is_empty());
    }

    #[test]
    fn glossary_add_list_and_groups() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let code = glossary_in(
            root,
            &s(&["add", "login"]),
            Some("sign-in"),
            Some("login, log in, log-in"),
        )
        .unwrap();
        assert_eq!(code, 0);
        let code = glossary_in(root, &s(&["add", "email"]), Some("email"), Some("e-mail")).unwrap();
        assert_eq!(code, 0);

        let style = std::fs::read_to_string(root.join("STYLE.md")).unwrap();
        assert!(style.contains("## Terminology"));
        assert!(style.contains("| Use | Not |"));
        assert!(style.contains("| sign-in | login, log in, log-in |"));
        assert!(style.contains("| email | e-mail |"));

        let cfg = resolved(root);
        let groups = glossary_groups(root, &cfg);
        assert_eq!(
            groups,
            vec![
                vec![
                    "sign-in".to_string(),
                    "login".into(),
                    "log in".into(),
                    "log-in".into()
                ],
                vec!["email".to_string(), "e-mail".into()],
            ]
        );

        // list exits 0
        assert_eq!(glossary_in(root, &s(&["list"]), None, None).unwrap(), 0);
    }

    #[test]
    fn glossary_groups_parses_existing_table() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("STYLE.md"),
            "# Style\n\nIntro prose.\n\n## Terminology\n\n| Use | Not |\n|---|---|\n| repository | repo |\n| data set | dataset, data-set |\n\n## Voice\n\n| Use | Not |\n| ignored | row |\n",
        )
        .unwrap();
        let cfg = resolved(root);
        let groups = glossary_groups(root, &cfg);
        assert_eq!(
            groups,
            vec![
                vec!["repository".to_string(), "repo".into()],
                vec!["data set".to_string(), "dataset".into(), "data-set".into()],
            ]
        );
    }

    #[test]
    fn glossary_groups_empty_without_file() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = resolved(dir.path());
        assert!(glossary_groups(dir.path(), &cfg).is_empty());
    }

    #[test]
    fn glossary_harvest_seen_merges_repo_and_catalog_terms() {
        let mut repo = HashMap::new();
        collect_glossary_harvest_terms("Users can login from the app.", &mut repo);

        let mut catalog = HashMap::new();
        collect_glossary_harvest_terms("The docs say to log in before setup.", &mut catalog);

        merge_harvest_seen(&mut repo, catalog);

        let login = repo.get("login").unwrap();
        assert!(login.contains("login"));
        assert!(login.contains("log in"));
    }

    #[test]
    fn glossary_harvest_reads_catalog_document_bodies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.duckdb");
        let conn = duckdb::Connection::open_in_memory().unwrap();
        crate::index::ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO documents (
                doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind,
                author_id, author_name, created_at, updated_at, observed_at, version,
                content_sha256, body, metadata_json
            ) VALUES ('doc1', 'slack', 'C123', 'slack:C123', 'Thread', '', '',
                'text/markdown', 'doc', '', '', NULL, NULL, 'now', '1', 'hash',
                'Some teams write email and others write e-mail.', '{}')",
            [],
        )
        .unwrap();
        index::publish_to_path(&conn, &path).unwrap();
        drop(conn);

        let seen = catalog_glossary_harvest_seen(&[path]).unwrap();
        let email = seen.get("email").unwrap();
        assert!(email.contains("email"));
        assert!(email.contains("e-mail"));
    }

    #[test]
    fn facts_add_and_list() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        assert_eq!(
            facts_in(
                root,
                &s(&["add", "Uptime SLA is 99.9%"]),
                Some("PRODUCT.md")
            )
            .unwrap(),
            0
        );
        assert_eq!(
            facts_in(root, &s(&["add", "Launched in 2024"]), None).unwrap(),
            0
        );

        let text = std::fs::read_to_string(root.join("FACTS.md")).unwrap();
        assert_eq!(
            text,
            "- Uptime SLA is 99.9%  (PRODUCT.md)\n- Launched in 2024\n"
        );

        assert_eq!(facts_in(root, &s(&["list"]), None).unwrap(), 0);
        assert_eq!(facts_in(root, &s(&["bogus"]), None).unwrap(), 2);
    }

    #[test]
    fn facts_add_rejects_empty_fact() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        assert_eq!(facts_in(root, &s(&["add", "   "]), None).unwrap(), 2);
        assert!(!root.join("FACTS.md").exists());
    }

    #[test]
    fn typed_span_heuristic() {
        assert!(has_typed_span("Latency dropped 40% after the change."));
        assert!(has_typed_span("It costs $12 per seat."));
        assert!(has_typed_span("Shipped in 2024 to all regions."));
        assert!(!has_typed_span("No numbers here at all."));
    }

    #[test]
    fn catalog_fact_candidates_honor_filters() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.duckdb");
        let conn = duckdb::Connection::open_in_memory().unwrap();
        crate::index::ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO documents (
                doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind,
                author_id, author_name, created_at, updated_at, observed_at, version,
                content_sha256, body, metadata_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, '', ?6, 'text/markdown', 'doc', '', '', ?7, ?8, ?8, '1', ?9, ?10, '{}')",
            duckdb::params![
                "slack/doc1",
                "slack",
                "doc1",
                "slack:C123",
                "Pricing update",
                "pricing.md",
                "2024-01-01T00:00:00Z",
                "2026-01-02T00:00:00Z",
                "hash1",
                "# Pricing\n\nContext line.\nARR reached $12 million in 2026. This sentence has no typed fact."
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO documents (
                doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind,
                author_id, author_name, created_at, updated_at, observed_at, version,
                content_sha256, body, metadata_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, '', ?6, 'text/markdown', 'doc', '', '', ?7, ?8, ?8, '1', ?9, ?10, '{}')",
            duckdb::params![
                "github/doc2",
                "github",
                "doc2",
                "github:repo#1",
                "Pricing issue",
                "issue.md",
                "2024-01-01T00:00:00Z",
                "2020-01-02T00:00:00Z",
                "hash2",
                "Latency was 30 ms in 2020."
            ],
        )
        .unwrap();
        index::publish_to_path(&conn, &path).unwrap();
        drop(conn);

        let got = extract_catalog_candidates_from_paths(
            &[path],
            Some("slack"),
            Some("pricing"),
            Some("2025-01-01T00:00:00Z"),
        )
        .unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].source, "slack");
        assert_eq!(got[0].reference, "slack:C123");
        assert_eq!(got[0].line, Some(4));
        assert!(got[0].text.contains("$12 million"));
    }

    #[test]
    fn repo_fact_candidates_include_line_numbers() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("facts.md"),
            "# Facts\n\nIntro line.\nLatency dropped 40% after launch.\n",
        )
        .unwrap();

        let got = extract_repo_candidates(root, Some("facts"), None);

        assert_eq!(got.len(), 1);
        assert_eq!(got[0].reference, "facts.md");
        assert_eq!(got[0].line, Some(4));
    }

    #[test]
    fn extract_rejects_unknown_source_key() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("facts.md"),
            "# Facts\n\nLatency dropped 40% after launch.\n",
        )
        .unwrap();

        assert_eq!(
            extract_in(
                root,
                &s(&["facts"]),
                Some("totally-bogus-source"),
                None,
                None,
                false
            )
            .unwrap(),
            2
        );
    }

    #[test]
    fn audit_kb_flags_product_banned_terms() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("PRODUCT.md"),
            "# Product\n\n## Banned words\n\n- frictionless\n- magic, synergy\n",
        )
        .unwrap();
        std::fs::write(
            root.join("doc.md"),
            "# Doc\n\nThis flow is frictionless and useful.\n",
        )
        .unwrap();

        let code = audit_kb_in(root, &[], true, true).unwrap();
        assert_eq!(code, 1);
    }

    #[test]
    fn conflicting_span_pair_is_kind_aware() {
        use std::collections::BTreeSet;
        let set =
            |vals: &[&str]| -> BTreeSet<String> { vals.iter().map(|s| s.to_string()).collect() };
        // Money vs money, different → conflict.
        assert!(conflicting_span_pair(&set(&["$49"]), &set(&["$99"])).is_some());
        // Percent vs percent, different → conflict.
        assert!(conflicting_span_pair(&set(&["40%"]), &set(&["50%"])).is_some());
        // Money vs count (a price vs a customer count) → NOT a conflict.
        assert!(conflicting_span_pair(&set(&["$49"]), &set(&["6625"])).is_none());
        // Count vs count (30 days vs 7 hours) → not flagged (ambiguous units).
        assert!(conflicting_span_pair(&set(&["30"]), &set(&["7"])).is_none());
        // Agreeing money value with extra coverage → NOT a conflict.
        assert!(conflicting_span_pair(&set(&["$49"]), &set(&["$49", "500"])).is_none());
    }

    #[test]
    fn audit_kb_flags_catalog_contradiction_candidates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.duckdb");
        let conn = duckdb::Connection::open_in_memory().unwrap();
        crate::index::ensure_schema(&conn).unwrap();
        for (doc_id, canonical_ref, body) in [
            ("doc1", "git:docs/a.md", "Latency dropped 40% after launch."),
            ("doc2", "git:docs/b.md", "Latency dropped 50% after launch."),
        ] {
            conn.execute(
                "INSERT INTO documents (
                    doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind,
                    author_id, author_name, created_at, updated_at, observed_at, version,
                    content_sha256, body, metadata_json
                ) VALUES (?1, 'git', ?2, ?3, ?3, '', ?2, 'text/markdown', 'doc', '', '', NULL, NULL, 'now', '1', 'hash', ?4, '{}')",
                duckdb::params![doc_id, canonical_ref, canonical_ref, body],
            )
            .unwrap();
        }
        index::publish_to_path(&conn, &path).unwrap();
        drop(conn);

        let findings = catalog_contradiction_findings_from_paths(&[path]).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "contradiction-candidate");
        assert!(findings[0].message.contains("40%"));
        assert!(findings[0].message.contains("50%"));
    }

    #[test]
    fn product_banned_terms_parse_explicit_sections() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("PRODUCT.md"),
            "# Product\n\n## Forbidden phrasings\n\n- Seamless\n- leverage, robust.\n\n## Voice\n\n- allowed\n",
        )
        .unwrap();
        assert_eq!(
            product_banned_terms(dir.path()),
            vec![
                "seamless".to_string(),
                "leverage".to_string(),
                "robust".to_string()
            ]
        );
    }
}
