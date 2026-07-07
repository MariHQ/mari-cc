//! Curation: tags, glossary, facts, extract, audit kb, humanize (SPEC §5.3/§5.4/§10).
//!
//! Tags live in the COMMITTED `<repo>/.mari/config.json` under `tags.entries`
//! (`{ref: {status, by, at, note}}`) so they are team-shared and versioned.
//! The glossary is STYLE.md's Terminology table (Use / Not columns); FACTS.md
//! is the deterministic grounding ledger (one `- fact  (source)` per line).

use crate::{authcmd, config, index, workspace};
use anyhow::Result;
use regex::Regex;
use serde_json::{json, Map, Value};
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

/// Committed tags.entries object from the repo config file.
fn tag_entries(root: &Path) -> Map<String, Value> {
    config::read_json(&config::repo_config_path(root))["tags"]["entries"]
        .as_object()
        .cloned()
        .unwrap_or_default()
}

fn write_tag_entries(root: &Path, entries: Map<String, Value>) -> Result<()> {
    config::set_in_file(
        &config::repo_config_path(root),
        "tags.entries",
        Value::Object(entries),
    )
}

// ---------------------------------------------------------------------------
// mari tag
// ---------------------------------------------------------------------------

pub fn tag(
    args: &[String],
    note: Option<&str>,
    status_filter: Option<&str>,
    json: bool,
) -> Result<i32> {
    tag_in(&workspace::work_root(), args, note, status_filter, json)
}

fn tag_in(
    root: &Path,
    args: &[String],
    note: Option<&str>,
    status_filter: Option<&str>,
    json: bool,
) -> Result<i32> {
    match args.first().map(|s| s.as_str()) {
        None => {
            eprintln!("usage: mari tag <path-or-ref> <status> [--note \"…\"] | mari tag list [--status S] [--json] | mari tag remove <ref>");
            Ok(2)
        }
        Some("list") => tag_list(root, status_filter, json),
        Some("remove") => {
            let Some(r) = args.get(1) else {
                eprintln!("usage: mari tag remove <path-or-ref>");
                return Ok(2);
            };
            let key = norm_ref(r);
            let mut entries = tag_entries(root);
            if entries.remove(&key).is_some() {
                write_tag_entries(root, entries)?;
                mirror_tag_to_catalogs(root, &key, None)?;
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
            let mut entry = json!({ "status": status, "by": author_in(root), "at": today() });
            if let Some(n) = note {
                entry["note"] = json!(n);
            }
            let mut entries = tag_entries(root);
            entries.insert(key.clone(), entry);
            let mirror_entry = entries.get(&key).cloned();
            write_tag_entries(root, entries)?;
            mirror_tag_to_catalogs(root, &key, mirror_entry.as_ref())?;
            match note {
                Some(n) => println!("✓ tagged {key} {status} — {n}"),
                None => println!("✓ tagged {key} {status}"),
            }
            Ok(0)
        }
    }
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
    // Show the resolved view (repo config layered on defaults/global).
    let cfg = resolved(root);
    let entries: BTreeMap<String, Value> = cfg["tags"]["entries"]
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(_, v)| match status_filter {
                    Some(s) => v["status"].as_str() == Some(s),
                    None => true,
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();
    if json_out {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(0);
    }
    if entries.is_empty() {
        println!(
            "no tags{}",
            status_filter
                .map(|s| format!(" with status {s}"))
                .unwrap_or_default()
        );
        return Ok(0);
    }
    for (r, v) in &entries {
        let status = v["status"].as_str().unwrap_or("?");
        let by = v["by"].as_str().unwrap_or("?");
        let at = v["at"].as_str().unwrap_or("?");
        let note = v["note"]
            .as_str()
            .map(|n| format!("  — {n}"))
            .unwrap_or_default();
        println!("{r}  [{status}]  ({by}, {at}){note}");
    }
    println!("{} tag(s)", entries.len());
    Ok(0)
}

/// Curation tag for a repo-relative path or doc ref, if any (SPEC §10.1).
pub fn tag_of(_root: &Path, cfg: &Value, r: &str) -> Option<String> {
    let entries = cfg["tags"]["entries"].as_object()?;
    let key = norm_ref(r);
    let entry = entries.get(&key).or_else(|| entries.get(r))?;
    entry["status"].as_str().map(String::from)
}

fn mirror_tag_to_catalogs(root: &Path, target: &str, entry: Option<&Value>) -> Result<()> {
    mirror_tag_to_catalog_paths(&catalog_paths(root), target, entry)
}

fn mirror_tag_to_catalog_paths(
    paths: &[PathBuf],
    target: &str,
    entry: Option<&Value>,
) -> Result<()> {
    for path in paths {
        if !path.exists() {
            continue;
        }
        let conn = duckdb::Connection::open(path)?;
        index::ensure_schema(&conn)?;
        for doc_id in catalog_doc_ids_for_target(&conn, target)? {
            conn.execute(
                "DELETE FROM tags WHERE target_type = 'doc' AND target_id = ?1",
                [&doc_id],
            )?;
            let Some(entry) = entry else {
                continue;
            };
            let Some(status) = entry["status"].as_str() else {
                continue;
            };
            conn.execute(
                "INSERT INTO tags (target_type, target_id, status, note, \"by\", \"at\", metadata_json)
                 VALUES ('doc', ?1, ?2, ?3, ?4, ?5, ?6)",
                duckdb::params![
                    doc_id,
                    status,
                    entry["note"].as_str().unwrap_or(""),
                    entry["by"].as_str().unwrap_or("unknown"),
                    entry["at"].as_str().unwrap_or(""),
                    json!({"source": "tags.entries", "target": target}).to_string()
                ],
            )?;
        }
    }
    Ok(())
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
        if !path.exists() {
            continue;
        }
        let conn = duckdb::Connection::open(path)?;
        index::ensure_schema(&conn)?;
        conn.execute("DELETE FROM facts WHERE fact_id = ?1", [&fact_id])?;
        conn.execute(
            "INSERT INTO facts (fact_id, claim, source_ref, source_span_id, status, created_by, created_at, metadata_json)
             VALUES (?1, ?2, ?3, NULL, 'accepted', ?4, ?5, ?6)",
            duckdb::params![fact_id, claim, source_ref, created_by, created_at, metadata],
        )?;
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
    let paths: Vec<PathBuf> = paths.into_iter().filter(|p| p.exists()).collect();
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

    // 1. Stale pages (mtime older than STALE_DAYS).
    let now = std::time::SystemTime::now();
    for (f, _) in &texts {
        if let Ok(modified) = std::fs::metadata(f).and_then(|m| m.modified()) {
            if let Ok(age) = now.duration_since(modified) {
                let days = age.as_secs() / 86_400;
                if days > STALE_DAYS {
                    findings.push(KbFinding {
                        severity: "warn",
                        rule: "stale-page",
                        file: rel_of(f),
                        message: format!("not updated in {days} days (threshold {STALE_DAYS})"),
                    });
                }
            }
        }
    }

    // 2. needs-review backlog from tags.entries and mirrored catalog tags.
    let mut needs_review_seen = HashSet::new();
    if let Some(entries) = cfg["tags"]["entries"].as_object() {
        for (r, v) in entries {
            if v["status"].as_str() == Some("needs-review") {
                let by = v["by"].as_str().unwrap_or("?");
                let at = v["at"].as_str().unwrap_or("?");
                needs_review_seen.insert(norm_ref(r));
                findings.push(KbFinding {
                    severity: "warn",
                    rule: "needs-review",
                    file: r.clone(),
                    message: format!("flagged needs-review by {by} on {at}"),
                });
            }
        }
    }
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
    paths.into_iter().filter(|p| p.exists()).collect()
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

fn catalog_contradiction_findings_from_paths(paths: &[PathBuf]) -> Result<Vec<KbFinding>> {
    let claims = catalog_claims_from_paths(paths)?;
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

    #[test]
    fn tag_add_list_remove_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // add
        let code = tag_in(
            root,
            &s(&["docs/api.md", "canonical"]),
            Some("primary reference"),
            None,
            false,
        )
        .unwrap();
        assert_eq!(code, 0);
        let cfg = config::read_json(&config::repo_config_path(root));
        let entry = &cfg["tags"]["entries"]["docs/api.md"];
        assert_eq!(entry["status"], "canonical");
        assert_eq!(entry["note"], "primary reference");
        assert!(entry["by"].as_str().is_some());
        assert!(entry["at"].as_str().unwrap().len() == 10); // YYYY-MM-DD

        // tag_of resolves, including ./ normalization
        assert_eq!(
            tag_of(root, &cfg, "docs/api.md").as_deref(),
            Some("canonical")
        );
        assert_eq!(
            tag_of(root, &cfg, "./docs/api.md").as_deref(),
            Some("canonical")
        );
        assert_eq!(tag_of(root, &cfg, "other.md"), None);

        // list (both plain and json paths just need to not fail)
        assert_eq!(tag_in(root, &s(&["list"]), None, None, false).unwrap(), 0);
        assert_eq!(
            tag_in(root, &s(&["list"]), None, Some("canonical"), true).unwrap(),
            0
        );

        // remove
        assert_eq!(
            tag_in(root, &s(&["remove", "./docs/api.md"]), None, None, false).unwrap(),
            0
        );
        let cfg = config::read_json(&config::repo_config_path(root));
        assert!(cfg["tags"]["entries"].get("docs/api.md").is_none());
        // removing again fails
        assert_eq!(
            tag_in(root, &s(&["remove", "docs/api.md"]), None, None, false).unwrap(),
            1
        );
    }

    #[test]
    fn tag_unknown_status_exits_2() {
        let dir = tempfile::tempdir().unwrap();
        let code = tag_in(
            dir.path(),
            &s(&["README.md", "totally-bogus-status"]),
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(code, 2);
        assert!(!config::repo_config_path(dir.path()).exists());
        assert_eq!(
            tag_in(
                dir.path(),
                &s(&["list"]),
                None,
                Some("totally-bogus-status"),
                false
            )
            .unwrap(),
            2
        );
    }

    #[test]
    fn tag_ref_with_dots_stays_flat() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        tag_in(
            root,
            &s(&["gdocs:launch.plan.v2", "draft"]),
            None,
            None,
            false,
        )
        .unwrap();
        let cfg = config::read_json(&config::repo_config_path(root));
        assert_eq!(
            cfg["tags"]["entries"]["gdocs:launch.plan.v2"]["status"],
            "draft"
        );
    }

    #[test]
    fn tag_mirror_updates_existing_catalog_docs() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = dir.path().join("catalog.duckdb");
        let conn = duckdb::Connection::open(&catalog).unwrap();
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
        drop(conn);

        let entry = json!({
            "status": "canonical",
            "by": "tester",
            "at": "2026-07-06",
            "note": "primary"
        });
        mirror_tag_to_catalog_paths(std::slice::from_ref(&catalog), "docs/api.md", Some(&entry))
            .unwrap();
        let conn = duckdb::Connection::open(&catalog).unwrap();
        let status: String = conn
            .query_row(
                "SELECT status FROM tags WHERE target_type = 'doc' AND target_id = 'doc1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "canonical");
        drop(conn);

        mirror_tag_to_catalog_paths(std::slice::from_ref(&catalog), "docs/api.md", None).unwrap();
        let conn = duckdb::Connection::open(&catalog).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn facts_mirror_writes_accepted_catalog_fact() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = dir.path().join("catalog.duckdb");
        let conn = duckdb::Connection::open(&catalog).unwrap();
        index::ensure_schema(&conn).unwrap();
        drop(conn);

        mirror_fact_to_catalog_paths(
            std::slice::from_ref(&catalog),
            "Latency dropped 40%.",
            Some("git:docs/postmortem.md"),
            "tester",
            "2026-07-06T00:00:00Z",
        )
        .unwrap();

        let conn = duckdb::Connection::open(&catalog).unwrap();
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
        let conn = duckdb::Connection::open(&catalog).unwrap();
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
        let conn = duckdb::Connection::open(&path).unwrap();
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
        let conn = duckdb::Connection::open(&path).unwrap();
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
        let conn = duckdb::Connection::open(&path).unwrap();
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
