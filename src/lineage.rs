//! Lineage curation CLI (SPEC §8.3): a lineage edge is a span↔span
//! maintenance promise with status proposed | confirmed | rejected and
//! provenance (`--by llm|human`). Confirmed edges power the post-edit
//! hook's ⛓ notices; nudges (§4.7) are the hand-declared counterpart.

use crate::index::{self, hash_hex, now};
use anyhow::{anyhow, Result};
use duckdb::{params, Connection};

pub fn run(args: &[String], json: bool, by: Option<&str>, note: Option<&str>) -> Result<i32> {
    match args.first().map(|s| s.as_str()) {
        None | Some("list") => list(json),
        Some("add") => {
            let (Some(src), Some(dst)) = (args.get(1), args.get(2)) else {
                eprintln!("usage: mari lineage add <file>[#symbol] <file>[#symbol] [--by human|llm] [--note \"…\"]");
                return Ok(2);
            };
            add(src, dst, by.unwrap_or("human"), note)
        }
        Some("confirm") => set_status(args.get(1), "confirmed"),
        Some("reject") => set_status(args.get(1), "rejected"),
        Some(other) => {
            eprintln!("unknown lineage subcommand: {other} (list | add | confirm | reject)");
            Ok(2)
        }
    }
}

fn open() -> Result<Connection> {
    let db = index::catalog_path(false);
    if !db.exists() {
        return Err(anyhow!("no catalog yet — run `mari sync` first"));
    }
    Ok(Connection::open(db)?)
}

/// Resolve `path[#symbol]` to a span in the catalog. Bare paths resolve to
/// the document's first span.
fn find_span(conn: &Connection, target: &str) -> Result<(String, String)> {
    let (path, symbol) = match target.split_once('#') {
        Some((p, s)) => (p, Some(s)),
        None => (target, None),
    };
    let doc: (String, String) = conn
        .query_row(
            "SELECT doc_id, COALESCE(path, canonical_ref) FROM documents \
             WHERE path = ?1 OR path LIKE '%' || ?1 OR canonical_ref = ?1 \
             ORDER BY LENGTH(path) ASC LIMIT 1",
            [path],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| anyhow!("no indexed document matches `{path}` — is it synced?"))?;
    let row: Result<(String, i64, i64), _> = match symbol {
        Some(sym) => conn.query_row(
            "SELECT span_id, start_line, end_line FROM spans \
             WHERE doc_id = ?1 AND (lower(label) = lower(?2) OR lower(label) LIKE '%' || lower(?2) || '%') \
             ORDER BY LENGTH(label) ASC LIMIT 1",
            params![doc.0, sym],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ),
        None => conn.query_row(
            "SELECT span_id, start_line, end_line FROM spans \
             WHERE doc_id = ?1 ORDER BY start_line ASC LIMIT 1",
            [&doc.0],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ),
    };
    let (span_id, start, end) = row.map_err(|_| match symbol {
        Some(sym) => anyhow!("symbol `{sym}` does not resolve in {}", doc.1),
        None => anyhow!("{} has no indexed spans — re-run `mari sync`", doc.1),
    })?;
    Ok((span_id, format!("{}:{start}-{end}", doc.1)))
}

fn curator() -> String {
    std::process::Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| std::env::var("USER").unwrap_or_else(|_| "unknown".into()))
}

fn add(src: &str, dst: &str, by: &str, note: Option<&str>) -> Result<i32> {
    if by != "human" && by != "llm" {
        eprintln!("--by must be `human` or `llm`");
        return Ok(2);
    }
    let conn = open()?;
    let (from_span, from_desc) = match find_span(&conn, src) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("✗ {e}");
            return Ok(1);
        }
    };
    let (to_span, to_desc) = match find_span(&conn, dst) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("✗ {e}");
            return Ok(1);
        }
    };
    // Hand-declared edges are confirmed; machine proposals start proposed (§8.3).
    let (status, confidence) = if by == "human" {
        ("confirmed", 1.0)
    } else {
        ("proposed", 0.6)
    };
    let lineage_id = hash_hex(&format!("lineage:{from_span}:{to_span}"));
    conn.execute(
        "DELETE FROM lineage_edges WHERE lineage_id = ?1",
        [&lineage_id],
    )?;
    conn.execute(
        "INSERT INTO lineage_edges (lineage_id, from_span_id, to_span_id, rel, status, confidence, confirmed_by, confirmed_at, last_checked_at, metadata_json)
         VALUES (?1, ?2, ?3, 'coupled', ?4, ?5, ?6, ?7, ?7, ?8)",
        params![
            lineage_id,
            from_span,
            to_span,
            status,
            confidence,
            if by == "human" { Some(curator()) } else { None },
            now(),
            serde_json::json!({"by": by, "note": note}).to_string(),
        ],
    )?;
    println!(
        "✓ lineage {} [{status}] {from_desc} ↔ {to_desc}",
        &lineage_id[..8]
    );
    Ok(0)
}

fn set_status(id_prefix: Option<&String>, status: &str) -> Result<i32> {
    let Some(prefix) = id_prefix else {
        eprintln!("usage: mari lineage {status} <id>");
        return Ok(2);
    };
    let conn = open()?;
    let n = conn.execute(
        "UPDATE lineage_edges SET status = ?1, confirmed_by = ?2, confirmed_at = ?3 \
         WHERE lineage_id LIKE ?4 || '%'",
        params![status, curator(), now(), prefix],
    )?;
    if n == 0 {
        eprintln!("✗ no lineage edge matches `{prefix}`");
        return Ok(1);
    }
    println!("✓ {n} edge(s) → {status}");
    Ok(0)
}

fn list(json: bool) -> Result<i32> {
    let conn = open()?;
    let mut stmt = conn.prepare(
        "SELECT le.lineage_id, le.status, le.rel, le.confidence, COALESCE(le.confirmed_by, ''), le.metadata_json,
                COALESCE(fd.path, fd.canonical_ref), fs.start_line, fs.end_line,
                COALESCE(td.path, td.canonical_ref), ts.start_line, ts.end_line
           FROM lineage_edges le
           JOIN spans fs ON fs.span_id = le.from_span_id
           JOIN documents fd ON fd.doc_id = fs.doc_id
           JOIN spans ts ON ts.span_id = le.to_span_id
           JOIN documents td ON td.doc_id = ts.doc_id
          ORDER BY le.status, fd.path",
    )?;
    let rows: Vec<serde_json::Value> = stmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "id": r.get::<_, String>(0)?,
                "status": r.get::<_, String>(1)?,
                "rel": r.get::<_, String>(2)?,
                "confidence": r.get::<_, f64>(3)?,
                "by": r.get::<_, String>(4)?,
                "metadata": r.get::<_, String>(5)?,
                "from": format!("{}:{}-{}", r.get::<_, String>(6)?, r.get::<_, i64>(7)?, r.get::<_, i64>(8)?),
                "to": format!("{}:{}-{}", r.get::<_, String>(9)?, r.get::<_, i64>(10)?, r.get::<_, i64>(11)?),
            }))
        })?
        .flatten()
        .collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else if rows.is_empty() {
        println!(
            "no lineage edges — add one with `mari lineage add <file>[#symbol] <file>[#symbol]`"
        );
    } else {
        for r in &rows {
            println!(
                "{}  [{}] {} ↔ {}  ({}, by {})",
                &r["id"].as_str().unwrap_or("")[..8],
                r["status"].as_str().unwrap_or(""),
                r["from"].as_str().unwrap_or(""),
                r["to"].as_str().unwrap_or(""),
                r["rel"].as_str().unwrap_or(""),
                r["by"].as_str().unwrap_or("")
            );
        }
    }
    Ok(0)
}
