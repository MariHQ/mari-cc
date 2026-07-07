//! Post-edit hook (SPEC §15). Always exits 0 and never modifies files.

use crate::{config, curation, detector, i18n, index, workspace};
use duckdb::{params, Connection};
use globset::{Glob, GlobSetBuilder};
use regex::Regex;
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};

pub fn run(args: &[String]) -> i32 {
    if let Err(err) = run_inner(args) {
        if std::env::var("MARI_HOOK_DEBUG").is_ok() {
            eprintln!("mari hook internal error: {err:#}");
        }
    }
    0
}

fn run_inner(args: &[String]) -> anyhow::Result<()> {
    let root = workspace::work_root();
    let cfg = config::resolve(Some(&root));
    if args.first().map(|a| a.as_str()) == Some("commit") {
        return commit_association(&root, &cfg);
    }
    if cfg["hook"]["quiet"].as_bool().unwrap_or(false) {
        return Ok(());
    }
    let files = edited_files(args, &root)?;
    if files.is_empty() {
        return Ok(());
    }
    let detector_settings = detector_settings_for_hook(&cfg);

    let max = cfg["hook"]["maxFindings"].as_u64().unwrap_or(20) as usize;
    for file in files {
        if !file.exists() {
            continue;
        }
        prose_lint(&root, &file, &detector_settings, max);
        i18n_notice(&root, &file);
        rule_notices(&root, &cfg, &file);
        let nudge_fired = nudge_notices(&root, &cfg, &file);
        let lineage_fired = lineage_notices(&root, &file);
        if !nudge_fired && !lineage_fired {
            association_notice(&root, &file);
        }
        pending_impact_notice(&root, &cfg, &file);
        tag_notice(&root, &cfg, &file);
    }
    Ok(())
}

fn detector_settings_for_hook(cfg: &Value) -> detector::runner::DetectorSettings {
    let mut settings = detector::runner::settings(false, None);
    if cfg["hook"]["grammar"].as_bool().unwrap_or(false) {
        settings.grammar = true;
    }
    settings
}

fn edited_files(args: &[String], root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut out = BTreeSet::new();
    for arg in args {
        if arg == "run" {
            continue;
        }
        out.insert(resolve(root, arg));
    }

    let mut stdin = String::new();
    let _ = std::io::stdin().read_to_string(&mut stdin);
    if let Ok(json) = serde_json::from_str::<Value>(&stdin) {
        collect_paths(&json, &mut out, root);
    }
    Ok(out.into_iter().collect())
}

fn collect_paths(v: &Value, out: &mut BTreeSet<PathBuf>, root: &Path) {
    match v {
        Value::Object(map) => {
            for key in ["file_path", "filePath", "path", "uri"] {
                if let Some(s) = map.get(key).and_then(|v| v.as_str()) {
                    if looks_like_path(s) {
                        out.insert(resolve(root, s.strip_prefix("file://").unwrap_or(s)));
                    }
                }
            }
            for child in map.values() {
                collect_paths(child, out, root);
            }
        }
        Value::Array(arr) => {
            for child in arr {
                collect_paths(child, out, root);
            }
        }
        Value::String(s) if looks_like_path(s) => {
            out.insert(resolve(root, s.strip_prefix("file://").unwrap_or(s)));
        }
        _ => {}
    }
}

fn looks_like_path(s: &str) -> bool {
    s.ends_with(".md")
        || s.ends_with(".mdx")
        || s.ends_with(".mdc")
        || s.ends_with(".txt")
        || s.contains('/')
}

fn prose_lint(root: &Path, file: &Path, settings: &detector::runner::DetectorSettings, max: usize) {
    if !is_markdown(file) {
        return;
    }
    let rel = rel(root, file);
    if detector::runner::file_ignored(settings, &rel) {
        return;
    }
    let Ok(text) = std::fs::read_to_string(file) else {
        return;
    };
    if detector::runner::skip_file(file, &text) {
        return;
    }
    let result = detector::runner::detect_text(&rel, &text, settings);
    for finding in result.findings.iter().take(max) {
        println!(
            "{}:{}:{} {} {}: {}",
            rel,
            finding.line,
            finding.col,
            finding.severity.label(),
            finding.rule_id,
            finding.message
        );
    }
    if result.findings.len() > max {
        println!(
            "{}: {} more finding(s) suppressed by hook.maxFindings",
            rel,
            result.findings.len() - max
        );
    }
}

fn i18n_notice(root: &Path, file: &Path) {
    let siblings = i18n::source_language_siblings(file);
    if siblings.is_empty() {
        return;
    }
    let rels: Vec<String> = siblings.iter().map(|p| rel(root, p)).collect();
    println!(
        "i18n: {} changed; check translation sibling(s): {}",
        rel(root, file),
        rels.join(", ")
    );
}

fn rule_notices(root: &Path, cfg: &Value, file: &Path) {
    let rel = rel(root, file);
    for rule in cfg["rules"].as_array().into_iter().flatten() {
        if matches_any(&rel, rule["paths"].as_array())
            && !matches_any(&rel, rule["exclude"].as_array())
        {
            if let Some(msg) = rule["notify"].as_str() {
                println!(
                    "notify {}: {}",
                    rule["name"].as_str().unwrap_or("rule"),
                    msg
                );
            }
        }
    }
}

fn nudge_notices(root: &Path, cfg: &Value, file: &Path) -> bool {
    let rel = rel(root, file);
    let mut fired = false;
    for nudge in cfg["nudges"].as_array().into_iter().flatten() {
        let when = &nudge["when"];
        let Some(when_file) = endpoint_path(when) else {
            continue;
        };
        if !glob_match(when_file, &rel) {
            continue;
        }
        if matches_any(&rel, nudge["exclude"].as_array()) {
            continue;
        }
        if let Some(symbol) = when["symbol"].as_str() {
            let path = root.join(&rel);
            if !symbol_exists(&path, symbol) {
                println!(
                    "nudge warning: {}#{} no longer resolves; matching whole file",
                    when_file, symbol
                );
            }
        }
        let edits: Vec<String> = nudge["edit"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| {
                let file = endpoint_path(e)?;
                Some(match e["symbol"].as_str() {
                    Some(sym) => format!("{file}#{sym}"),
                    None => file.to_string(),
                })
            })
            .collect();
        println!(
            "✎ nudge {}: {} changed — edit {}{}",
            nudge["name"].as_str().unwrap_or("unnamed"),
            rel,
            edits.join(", "),
            nudge["message"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|s| format!(" — {s}"))
                .unwrap_or_default()
        );
        fired = true;
    }
    fired
}

fn endpoint_path(endpoint: &Value) -> Option<&str> {
    endpoint["path"]
        .as_str()
        .or_else(|| endpoint["file"].as_str())
}

fn lineage_notices(root: &Path, file: &Path) -> bool {
    let Some(conn) = open_existing_catalog() else {
        return false;
    };
    let rel = rel(root, file);
    let Ok(docs) = doc_ids_for_path(&conn, &rel) else {
        return false;
    };
    let mut fired = false;
    for doc_id in docs {
        if lineage_for_doc(&conn, &doc_id, &rel).unwrap_or(false) {
            fired = true;
        }
    }
    fired
}

fn lineage_for_doc(conn: &Connection, doc_id: &str, rel: &str) -> anyhow::Result<bool> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT COALESCE(other_doc.path, other_doc.canonical_ref), other_span.start_line, other_span.end_line, le.rel
           FROM lineage_edges le
           JOIN spans changed_span
             ON changed_span.span_id = le.from_span_id OR changed_span.span_id = le.to_span_id
           JOIN spans other_span
             ON (other_span.span_id = le.from_span_id OR other_span.span_id = le.to_span_id)
            AND other_span.span_id <> changed_span.span_id
           JOIN documents other_doc ON other_doc.doc_id = other_span.doc_id
          WHERE changed_span.doc_id = ?1
            AND le.status = 'confirmed'
          ORDER BY other_doc.path, other_span.start_line
          LIMIT 5",
    )?;
    let rows = stmt.query_map([doc_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    let mut fired = false;
    for row in rows.flatten() {
        println!(
            "⛓ lineage: {rel} changed; reconcile {}:{}-{} ({})",
            row.0, row.1, row.2, row.3
        );
        fired = true;
    }
    Ok(fired)
}

fn association_notice(root: &Path, file: &Path) -> bool {
    let Some(conn) = open_existing_catalog() else {
        return false;
    };
    let rel = rel(root, file);
    let Ok(docs) = doc_ids_for_path(&conn, &rel) else {
        return false;
    };
    let mut fired = false;
    for doc_id in docs {
        if associations_for_doc(&conn, &doc_id, &rel).unwrap_or(false) {
            fired = true;
        }
    }
    fired
}

fn associations_for_doc(conn: &Connection, doc_id: &str, rel: &str) -> anyhow::Result<bool> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT COALESCE(d.path, d.canonical_ref), e.rel, e.to_type, e.to_id
           FROM edges seed
           JOIN edges e ON e.rel = seed.rel AND e.to_type = seed.to_type AND e.to_id = seed.to_id
           JOIN documents d ON d.doc_id = e.from_id
          WHERE seed.from_type = 'doc'
            AND seed.from_id = ?1
            AND e.from_type = 'doc'
            AND e.from_id <> ?1
          ORDER BY d.path
          LIMIT 5",
    )?;
    let rows = stmt.query_map([doc_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    let related = rows.flatten().collect::<Vec<_>>();
    if related.is_empty() {
        return Ok(false);
    }
    let rendered = related
        .iter()
        .map(|row| format!("{} ({})", row.0, related_reason(&row.1, &row.2, &row.3)))
        .collect::<Vec<_>>()
        .join(", ");
    println!("assoc: {rel} changed; related file(s): {rendered}");
    Ok(true)
}

fn related_reason(rel: &str, to_type: &str, to_id: &str) -> String {
    match (rel, to_type) {
        ("in_repo", "container") => to_id
            .split_once(':')
            .map(|(_, v)| format!("same directory {v}"))
            .unwrap_or_else(|| "same directory".into()),
        _ => format!("{rel}:{to_type}"),
    }
}

fn doc_ids_for_path(conn: &Connection, rel: &str) -> anyhow::Result<Vec<String>> {
    // Sources record the same file in different path forms (repo-relative
    // for git, absolute for localfiles, symlinked /tmp vs /private/tmp on
    // macOS) — match on suffix so one edit resolves every sibling doc.
    let mut stmt = conn.prepare(
        "SELECT doc_id FROM documents
          WHERE path = ?1 OR external_id = ?1 OR canonical_ref = ?1
             OR path LIKE ?2 OR ?1 LIKE '%' || path OR canonical_ref LIKE ?2",
    )?;
    let like = format!("%{rel}");
    let rows = stmt.query_map(params![rel, like], |row| row.get::<_, String>(0))?;
    Ok(rows.flatten().collect())
}

fn open_existing_catalog() -> Option<Connection> {
    let path = index::catalog_path(false);
    if !path.exists() {
        return None;
    }
    Connection::open(path).ok()
}

fn tag_notice(root: &Path, cfg: &Value, file: &Path) {
    let rel = rel(root, file);
    if let Some(status) = curation::tag_of(root, cfg, &rel) {
        if matches!(status.as_str(), "stale" | "deprecated") {
            println!("tag: {rel} is marked {status}; update or avoid relying on it");
        }
        if status == "customer-facing" {
            internal_reference_notices(root, cfg, file, &rel);
        }
    }
}

fn internal_reference_notices(root: &Path, cfg: &Value, file: &Path, rel: &str) {
    let Ok(text) = std::fs::read_to_string(file) else {
        return;
    };
    for target in internal_references(root, cfg, file, &text) {
        println!("tag: {rel} is customer-facing but references internal content: {target}");
    }
}

fn internal_references(root: &Path, cfg: &Value, file: &Path, text: &str) -> Vec<String> {
    let internal = internal_tagged_refs(cfg);
    if internal.is_empty() {
        return Vec::new();
    }
    let base = file.parent().unwrap_or(root);
    let mut out = BTreeSet::new();
    for link in markdown_links(text) {
        if is_external_link(&link) {
            continue;
        }
        let (path, _) = split_link_target(&link);
        if path.is_empty() {
            continue;
        }
        let resolved = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            base.join(path)
        };
        let normalized = rel(root, &resolved);
        if internal.contains(&normalized) || internal.contains(path) {
            out.insert(normalized);
        }
    }
    out.into_iter().collect()
}

fn internal_tagged_refs(cfg: &Value) -> BTreeSet<String> {
    cfg["tags"]["entries"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|&(_, entry)| entry["status"].as_str() == Some("internal"))
        .map(|(target, _)| target.strip_prefix("./").unwrap_or(target).to_string())
        .collect()
}

fn markdown_links(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let mut rest = line;
        while let Some(close) = rest.find("](") {
            let after = &rest[close + 2..];
            let Some(end) = after.find(')') else {
                break;
            };
            let target = after[..end].trim();
            if !target.is_empty() {
                out.push(target.to_string());
            }
            rest = &after[end + 1..];
        }
    }
    out
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

fn symbol_exists(path: &Path, symbol: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "md" | "mdx" | "mdc" | "markdown" => markdown_heading_exists(&text, symbol),
        "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" => code_symbol_exists(&text, &ext, symbol),
        _ => false,
    }
}

fn markdown_heading_exists(text: &str, symbol: &str) -> bool {
    let target = normalize_symbol(symbol);
    text.lines().any(|line| {
        let t = line.trim();
        let level = t.chars().take_while(|c| *c == '#').count();
        (1..=6).contains(&level)
            && t.chars().nth(level) == Some(' ')
            && normalize_symbol(t[level..].trim().trim_matches('#').trim()) == target
    })
}

fn code_symbol_exists(text: &str, ext: &str, symbol: &str) -> bool {
    let patterns: &[&str] = match ext {
        "rs" => &[
            r"^\s*pub(?:\([^)]*\))?\s+(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)",
            r"^\s*pub(?:\([^)]*\))?\s+(?:struct|enum|trait|type|const|static|mod)\s+([A-Za-z_][A-Za-z0-9_]*)",
        ],
        "ts" | "tsx" | "js" | "jsx" => &[
            r"^\s*export\s+(?:default\s+)?(?:async\s+)?function\s+([A-Za-z_$][A-Za-z0-9_$]*)",
            r"^\s*export\s+(?:default\s+)?(?:const|let|var|class|interface|type|enum)\s+([A-Za-z_$][A-Za-z0-9_$]*)",
        ],
        "py" => &[
            r"^\s*(?:async\s+)?def\s+([A-Za-z_][A-Za-z0-9_]*)",
            r"^\s*class\s+([A-Za-z_][A-Za-z0-9_]*)",
        ],
        "go" => &[
            r"^\s*func\s+([A-Z][A-Za-z0-9_]*)",
            r"^\s*func\s+\([^)]*\)\s*([A-Z][A-Za-z0-9_]*)",
            r"^\s*type\s+([A-Z][A-Za-z0-9_]*)",
            r"^\s*(?:const|var)\s+([A-Z][A-Za-z0-9_]*)",
        ],
        _ => return false,
    };
    let regexes = patterns
        .iter()
        .map(|p| Regex::new(p).unwrap())
        .collect::<Vec<_>>();
    text.lines().any(|line| {
        regexes.iter().any(|re| {
            re.captures(line)
                .and_then(|caps| caps.get(1))
                .map(|m| m.as_str() == symbol)
                .unwrap_or(false)
        })
    })
}

fn normalize_symbol(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || c.is_whitespace() || *c == '-' || *c == '_')
        .flat_map(|c| c.to_lowercase())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn matches_any(rel: &str, arr: Option<&Vec<Value>>) -> bool {
    arr.into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .any(|g| glob_match(g, rel))
}

fn glob_match(pattern: &str, rel: &str) -> bool {
    let mut b = GlobSetBuilder::new();
    if let Ok(g) = Glob::new(pattern) {
        b.add(g);
    }
    b.build()
        .map(|set| {
            set.is_match(rel)
                || Path::new(rel)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|b| set.is_match(b))
                    .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn resolve(root: &Path, raw: &str) -> PathBuf {
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        p
    } else {
        root.join(p)
    }
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "md" | "mdx" | "mdc" | "markdown"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn collects_common_hook_paths() {
        let dir = tempdir().unwrap();
        let mut out = BTreeSet::new();
        collect_paths(
            &json!({"tool_input": {"file_path": "docs/a.md"}}),
            &mut out,
            dir.path(),
        );
        assert!(out.contains(&dir.path().join("docs/a.md")));
    }

    #[test]
    fn glob_matches_basename_and_relative_path() {
        assert!(glob_match("*.md", "docs/a.md"));
        assert!(glob_match("docs/**", "docs/a.md"));
        assert!(!glob_match("src/**", "docs/a.md"));
    }

    #[test]
    fn nudge_endpoint_path_prefers_spec_key_and_accepts_legacy_file() {
        let spec = json!({"path": "docs/api.md", "file": "legacy.md"});
        let legacy = json!({"file": "docs/legacy.md"});

        assert_eq!(endpoint_path(&spec), Some("docs/api.md"));
        assert_eq!(endpoint_path(&legacy), Some("docs/legacy.md"));
    }

    #[test]
    fn hook_grammar_config_enables_detector_grammar() {
        let cfg = json!({
            "hook": { "grammar": true },
            "detector": { "grammar": false }
        });

        assert!(detector_settings_for_hook(&cfg).grammar);
    }

    #[test]
    fn catalog_doc_lookup_matches_paths_and_refs() {
        let conn = Connection::open_in_memory().unwrap();
        index::ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO documents (doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind, author_id, author_name, created_at, updated_at, observed_at, version, content_sha256, body, metadata_json)
             VALUES ('d1', 'localfiles', 'docs/a.md', 'localfiles:docs/a.md', 'A', NULL, 'docs/a.md', 'text/markdown', 'file', NULL, NULL, NULL, NULL, 'now', 'v', 'sha', '# A', '{}')",
            [],
        )
        .unwrap();

        let docs = doc_ids_for_path(&conn, "docs/a.md").unwrap();
        assert_eq!(docs, vec!["d1"]);
    }

    #[test]
    fn association_query_reports_shared_edge_neighbor() {
        let conn = Connection::open_in_memory().unwrap();
        index::ensure_schema(&conn).unwrap();
        for (doc, path) in [("d1", "docs/a.md"), ("d2", "docs/b.md")] {
            conn.execute(
                "INSERT INTO documents (doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind, author_id, author_name, created_at, updated_at, observed_at, version, content_sha256, body, metadata_json)
                 VALUES (?1, 'localfiles', ?2, ?3, ?4, NULL, ?2, 'text/markdown', 'file', NULL, NULL, NULL, NULL, 'now', 'v', 'sha', '# Doc', '{}')",
                params![doc, path, format!("localfiles:{path}"), path],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO edges (edge_id, from_type, from_id, to_type, to_id, rel, confidence, evidence_span_id, created_by, created_at, metadata_json)
                 VALUES (?1, 'doc', ?2, 'container', 'localfiles:docs', 'in_repo', 1.0, NULL, 'test', 'now', '{}')",
                params![format!("e-{doc}"), doc],
            )
            .unwrap();
        }

        assert!(associations_for_doc(&conn, "d1", "docs/a.md").unwrap());
    }

    #[test]
    fn lineage_query_requires_confirmed_edges() {
        let conn = Connection::open_in_memory().unwrap();
        index::ensure_schema(&conn).unwrap();
        for (doc, path) in [("d1", "docs/a.md"), ("d2", "docs/b.md")] {
            conn.execute(
                "INSERT INTO documents (doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind, author_id, author_name, created_at, updated_at, observed_at, version, content_sha256, body, metadata_json)
                 VALUES (?1, 'localfiles', ?2, ?3, ?4, NULL, ?2, 'text/markdown', 'file', NULL, NULL, NULL, NULL, 'now', 'v', 'sha', '# Doc', '{}')",
                params![doc, path, format!("localfiles:{path}"), path],
            )
            .unwrap();
        }
        for (span, doc, line) in [("s1", "d1", 1), ("s2", "d2", 5)] {
            conn.execute(
                "INSERT INTO spans (span_id, doc_id, chunk_id, span_kind, label, start_byte, end_byte, start_line, end_line, stable_hash, metadata_json)
                 VALUES (?1, ?2, NULL, 'paragraph', NULL, 0, 10, ?3, ?3, ?4, '{}')",
                params![span, doc, line, format!("hash-{span}")],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO lineage_edges (lineage_id, from_span_id, to_span_id, rel, status, confidence, confirmed_by, confirmed_at, last_checked_at, metadata_json)
             VALUES ('l1', 's1', 's2', 'supports', 'draft', 0.9, NULL, NULL, NULL, '{}')",
            [],
        )
        .unwrap();
        assert!(!lineage_for_doc(&conn, "d1", "docs/a.md").unwrap());

        conn.execute("UPDATE lineage_edges SET status = 'confirmed'", [])
            .unwrap();
        assert!(lineage_for_doc(&conn, "d1", "docs/a.md").unwrap());
    }

    #[test]
    fn internal_reference_warning_uses_markdown_links_only() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let file = root.join("docs/public.md");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "[ok](guide.md)\n[secret](internal.md#notes)\n```md\n[ignored](internal.md)\n```\n[external](https://example.com/internal.md)\n").unwrap();
        let cfg = json!({
            "tags": {
                "entries": {
                    "docs/public.md": { "status": "customer-facing" },
                    "docs/internal.md": { "status": "internal" }
                }
            }
        });

        let refs = internal_references(root, &cfg, &file, &std::fs::read_to_string(&file).unwrap());
        assert_eq!(refs, vec!["docs/internal.md"]);
    }

    #[test]
    fn internal_tagged_refs_strip_dot_prefix() {
        let cfg = json!({
            "tags": {
                "entries": {
                    "./docs/internal.md": { "status": "internal" },
                    "docs/public.md": { "status": "customer-facing" }
                }
            }
        });
        assert!(internal_tagged_refs(&cfg).contains("docs/internal.md"));
        assert!(!internal_tagged_refs(&cfg).contains("docs/public.md"));
    }

    #[test]
    fn hook_symbol_resolver_matches_markdown_heading() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("api.md");
        std::fs::write(&file, "# API\n\n## Rate limits\n").unwrap();
        assert!(symbol_exists(&file, "Rate limits"));
        assert!(!symbol_exists(&file, "Missing"));
    }

    #[test]
    fn hook_symbol_resolver_requires_exported_code() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("lib.rs");
        std::fs::write(
            &file,
            "fn private() {}\npub(crate) mod api;\npub fn public_api() {}\n",
        )
        .unwrap();
        assert!(symbol_exists(&file, "api"));
        assert!(symbol_exists(&file, "public_api"));
        assert!(!symbol_exists(&file, "private"));
    }

    #[test]
    fn hook_symbol_resolver_matches_exported_ts_and_go_forms() {
        let dir = tempdir().unwrap();
        let ts = dir.path().join("api.ts");
        std::fs::write(
            &ts,
            "const hidden = 1;\nexport default function createApp() {}\nexport enum Mode {}\n",
        )
        .unwrap();
        assert!(symbol_exists(&ts, "createApp"));
        assert!(symbol_exists(&ts, "Mode"));
        assert!(!symbol_exists(&ts, "hidden"));

        let go = dir.path().join("main.go");
        std::fs::write(
            &go,
            "func helper() {}\nfunc (s *Server) Listen() {}\nconst DefaultPort = 8080\n",
        )
        .unwrap();
        assert!(symbol_exists(&go, "Listen"));
        assert!(symbol_exists(&go, "DefaultPort"));
        assert!(!symbol_exists(&go, "helper"));
    }
}

// ---------------------------------------------------------------------------
// §15.2 commit association (opt-in post-commit hook: `mari hook commit`)
// ---------------------------------------------------------------------------

fn commit_association(root: &Path, cfg: &Value) -> anyhow::Result<()> {
    let sha = git_out(root, &["rev-parse", "HEAD"])?;
    let message = git_out(root, &["log", "-1", "--pretty=%B"])?;
    let touched: Vec<String> = git_out(root, &["show", "--name-only", "--pretty=format:", "HEAD"])?
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect();
    if touched.is_empty() {
        return Ok(());
    }

    // 1. Coverage: a commit touched files covered by an edit-notify rule, or
    //    a nudge's `when`, without a matching change to the targets (§15.2).
    for rule in cfg["rules"].as_array().cloned().unwrap_or_default() {
        let name = rule["name"].as_str().unwrap_or("rule");
        let paths = glob_set(rule["paths"].as_array());
        let excludes = glob_set(rule["exclude"].as_array());
        if touched
            .iter()
            .any(|f| glob_matches(&paths, f) && !glob_matches(&excludes, f))
        {
            println!(
                "✎ commit {}: rule {name} — {}",
                &sha[..sha.len().min(8)],
                rule["notify"].as_str().unwrap_or("check the coupled docs")
            );
        }
    }
    for nudge in cfg["nudges"].as_array().cloned().unwrap_or_default() {
        let name = nudge["name"].as_str().unwrap_or("nudge");
        let when = endpoint_path(&nudge["when"]).unwrap_or("");
        let when_glob = glob_set(Some(&vec![Value::String(strip_symbol(when).to_string())]));
        let excludes = glob_set(nudge["exclude"].as_array());
        let trigger = touched
            .iter()
            .any(|f| glob_matches(&when_glob, f) && !glob_matches(&excludes, f));
        if !trigger {
            continue;
        }
        let targets: Vec<String> = nudge["edit"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(endpoint_path)
                    .map(|p| strip_symbol(p).to_string())
                    .collect()
            })
            .unwrap_or_default();
        let satisfied = targets
            .iter()
            .any(|t| touched.iter().any(|f| f == t || f.ends_with(t.as_str())));
        if !satisfied && !targets.is_empty() {
            println!(
                "✎ commit {}: nudge {name} — {} changed without a matching edit to {}",
                &sha[..sha.len().min(8)],
                when,
                targets.join(", ")
            );
        }
    }

    // 2. Associate the commit with relevant knowledge via keyword overlap
    //    against the catalog ("context is never lost").
    let terms: Vec<String> = commit_terms(&message);
    if terms.is_empty() {
        return Ok(());
    }
    for global in [false, true] {
        let db = index::catalog_path(global);
        if !db.exists() {
            continue;
        }
        let Ok(conn) = duckdb::Connection::open(&db) else {
            continue;
        };
        let like: Vec<String> = terms.iter().map(|t| format!("%{t}%")).collect();
        let sql = format!(
            "SELECT d.doc_id, d.source_id, d.canonical_ref, d.title, COUNT(*) AS score
             FROM chunks c JOIN documents d ON d.doc_id = c.doc_id
             WHERE {}
             GROUP BY d.doc_id, d.source_id, d.canonical_ref, d.title
             ORDER BY score DESC LIMIT 3",
            like.iter()
                .enumerate()
                .map(|(i, _)| format!("lower(c.text) LIKE ?{}", i + 1))
                .collect::<Vec<_>>()
                .join(" OR ")
        );
        let Ok(mut stmt) = conn.prepare(&sql) else {
            continue;
        };
        let params: Vec<&dyn duckdb::ToSql> =
            like.iter().map(|s| s as &dyn duckdb::ToSql).collect();
        let Ok(rows) = stmt.query_map(&params[..], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        }) else {
            continue;
        };
        let mut related = Vec::new();
        for (doc_id, source, cref, title) in rows.flatten() {
            // Skip the commit's own doc and other commits from this repo.
            if cref.contains(&sha[..sha.len().min(8)]) {
                continue;
            }
            related.push((doc_id, source, cref, title));
        }
        if related.is_empty() {
            continue;
        }
        println!("⛓ commit {} relates to:", &sha[..sha.len().min(8)]);
        for (doc_id, source, cref, title) in &related {
            println!("  {source}: {title}  ({cref})");
            // Persist the association edge in the catalog.
            let edge_id = index::hash_hex(&format!("commit:{sha}:{doc_id}"));
            let _ = conn.execute(
                "DELETE FROM edges WHERE edge_id = ?1",
                duckdb::params![edge_id],
            );
            let _ = conn.execute(
                "INSERT INTO edges (edge_id, from_type, from_id, to_type, to_id, rel, confidence, evidence_span_id, created_by, created_at, metadata_json)
                 VALUES (?1, 'commit', ?2, 'doc', ?3, 'associated_with', 0.5, NULL, 'commit-hook', ?4, '{}')",
                duckdb::params![edge_id, sha, doc_id, index::now()],
            );
        }
    }
    Ok(())
}

fn git_out(root: &Path, args: &[&str]) -> anyhow::Result<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn strip_symbol(p: &str) -> &str {
    p.split('#').next().unwrap_or(p)
}

fn glob_set(arr: Option<&Vec<Value>>) -> globset::GlobSet {
    let mut b = GlobSetBuilder::new();
    if let Some(arr) = arr {
        for g in arr.iter().filter_map(|v| v.as_str()) {
            if let Ok(glob) = Glob::new(g) {
                b.add(glob);
            }
        }
    }
    b.build().unwrap_or_else(|_| globset::GlobSet::empty())
}

fn glob_matches(set: &globset::GlobSet, path: &str) -> bool {
    if set.is_match(path) {
        return true;
    }
    Path::new(path)
        .file_name()
        .and_then(|b| b.to_str())
        .map(|b| set.is_match(b))
        .unwrap_or(false)
}

/// Content terms from a commit message: lowercase words ≥4 chars minus a
/// small stopword set, capped at 6.
fn commit_terms(message: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "this", "that", "with", "from", "into", "when", "then", "also", "adds", "added", "fixes",
        "fixed", "update", "updates", "updated", "remove", "removed", "change", "changed",
        "changes", "merge", "commit", "initial", "minor", "small", "some", "more",
    ];
    let re = Regex::new(r"[a-zA-Z][a-zA-Z0-9_-]{3,}").unwrap();
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for m in re.find_iter(message) {
        let t = m.as_str().to_lowercase();
        if STOP.contains(&t.as_str()) || !seen.insert(t.clone()) {
            continue;
        }
        out.push(t);
        if out.len() >= 6 {
            break;
        }
    }
    out
}

#[cfg(test)]
mod commit_tests {
    use super::commit_terms;

    #[test]
    fn commit_terms_filter_stopwords_and_cap() {
        let terms =
            commit_terms("Fixed pricing tiers: update docs and billing config with this change");
        assert!(terms.contains(&"pricing".to_string()));
        assert!(terms.contains(&"tiers".to_string()));
        assert!(!terms.contains(&"fixed".to_string()));
        assert!(!terms.contains(&"this".to_string()));
        assert!(terms.len() <= 6);
    }
}

/// §15.1 job 7 — knowledge pending-impact: note when scanned knowledge
/// (scan.* config, §4.9) affecting this file changed recently.
fn pending_impact_notice(root: &Path, cfg: &Value, file: &Path) {
    let scan = &cfg["scan"];
    let gdoc_refs: Vec<String> = ["docs", "folders"]
        .iter()
        .flat_map(|k| scan["google"][k].as_array().cloned().unwrap_or_default())
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    let channels: Vec<String> = scan["slack"]["channels"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.trim_start_matches('#').to_string()))
        .collect();
    if gdoc_refs.is_empty() && channels.is_empty() {
        return;
    }
    let lookback = scan["slack"]["lookbackDays"].as_i64().unwrap_or(14).max(1);
    let floor = (chrono::Utc::now() - chrono::Duration::days(lookback)).to_rfc3339();

    // Relevance: file-stem tokens (≥4 chars) appearing in the doc title.
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let tokens: Vec<String> = stem
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() >= 4)
        .map(String::from)
        .collect();
    if tokens.is_empty() {
        return;
    }
    let _ = root;
    for global in [false, true] {
        let db = index::catalog_path(global);
        if !db.exists() {
            continue;
        }
        let Ok(conn) = duckdb::Connection::open(&db) else {
            continue;
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT source_id, canonical_ref, title, updated_at FROM documents              WHERE source_id IN ('gdocs', 'slack') AND updated_at > ?1              ORDER BY updated_at DESC LIMIT 200",
        ) else {
            continue;
        };
        let Ok(rows) = stmt.query_map(params![floor], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        }) else {
            continue;
        };
        for (source, cref, title, updated) in rows.flatten() {
            // Scoped to the scanned refs (§4.9).
            let scanned = match source.as_str() {
                "gdocs" => gdoc_refs
                    .iter()
                    .any(|r| cref.contains(r.as_str()) || r.contains("docs.google.com")),
                "slack" => channels
                    .iter()
                    .any(|c| cref.contains(c.as_str()) || title.contains(&format!("#{c}"))),
                _ => false,
            };
            if !scanned {
                continue;
            }
            let lower = title.to_lowercase();
            if tokens.iter().any(|t| lower.contains(t.as_str())) {
                println!(
                    "⚠ scanned knowledge changed: {title}  ({cref}, updated {updated}) — review {}",
                    file.display()
                );
            }
        }
    }
}
