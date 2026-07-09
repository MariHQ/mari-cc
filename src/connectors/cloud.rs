//! Cloud connector sync loops (SPEC §6.1–§6.13).
//!
//! Shared contract (§6.0): per-doc revision signal decides *fetch*; the
//! content hash is the final authority for *re-embed* (a revision bump with
//! identical text updates metadata only). HTTP retries 429/≥500 up to 4
//! attempts honoring Retry-After (else exponential backoff); 401 gets one
//! token-refresh attempt then an auth error; 60s timeout. One bad doc is
//! logged and skipped; a source failure never aborts other sources.

use crate::index::{hash_hex, now};
use crate::{authcmd, config, workspace};
use anyhow::{anyhow, Result};
use duckdb::{params, Connection};
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[derive(Default)]
pub struct SyncStats {
    pub seen: usize,
    pub changed: usize,
    pub deleted: usize,
    pub chunks: usize,
}

pub struct RemoteDoc {
    pub external_id: String,
    pub canonical_ref: String,
    pub title: String,
    pub url: Option<String>,
    pub author: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub mime: &'static str,
    pub kind: &'static str,
    /// (container id, edge rel: in_repo | in_project | in_channel)
    pub container: Option<(String, &'static str)>,
    pub body: String,
    pub revision: String,
}

/// Entry point used by the sync engine for every non-local source.
pub fn sync_source(
    conn: &Connection,
    source: &str,
    rebuild: bool,
    since_days: Option<i64>,
) -> Result<SyncStats> {
    let root = workspace::work_root();
    let cfg = config::resolve(Some(&root));
    match source {
        "slack" => slack::sync(conn, &cfg, rebuild, since_days),
        "gdocs" => gdocs::sync(conn, &cfg, rebuild, since_days),
        "github" => github::sync(conn, &cfg, rebuild),
        "confluence" => confluence::sync(conn, &cfg, rebuild),
        "jira" => jira::sync(conn, &cfg, rebuild),
        "zendesk" => zendesk::sync(conn, &cfg, rebuild),
        "salesforce" => salesforce::sync(conn, &cfg, rebuild),
        "hubspot" => hubspot::sync(conn, &cfg, rebuild),
        "microsoft" => microsoft::sync(conn, &cfg, rebuild),
        "discord" => discord::sync(conn, &cfg, rebuild, since_days),
        "linear" => linear::sync(conn, &cfg, rebuild),
        "granola" => granola::sync(conn, &cfg, rebuild),
        other => Err(anyhow!("unknown source: {other}")),
    }
}

/// A tracked-but-unconnected source is a nudge, not an error (§6.0).
pub struct NotConnected(pub String);

fn credential_or_nudge(provider: &str) -> std::result::Result<Value, NotConnected> {
    authcmd::credential(provider).ok_or_else(|| {
        NotConnected(format!(
            "{provider} is tracked but not connected — run `mari auth {provider} …`"
        ))
    })
}

// ---------------------------------------------------------------------------
// Catalog helpers
// ---------------------------------------------------------------------------

pub fn get_meta(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM schema_meta WHERE key = ?1", [key], |r| {
        r.get(0)
    })
    .ok()
}

fn stored_doc(conn: &Connection, doc_id: &str) -> Option<(String, String)> {
    conn.query_row(
        "SELECT content_sha256, COALESCE(version, '') FROM documents WHERE doc_id = ?1",
        [doc_id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    )
    .ok()
}

/// Upsert a remote document. Returns chunks embedded (None = unchanged).
pub fn ingest_remote_doc(
    conn: &Connection,
    source_id: &str,
    doc: &RemoteDoc,
) -> Result<Option<usize>> {
    let doc_id = hash_hex(&format!("{source_id}:{}", doc.external_id));
    let content_sha = hash_hex(&doc.body);
    if let Some((old_sha, old_rev)) = stored_doc(conn, &doc_id) {
        if old_sha == content_sha {
            // Revision bump with identical text: metadata only (§6.0).
            if old_rev != doc.revision {
                conn.execute(
                    "UPDATE documents SET version = ?2, updated_at = ?3, observed_at = ?4 WHERE doc_id = ?1",
                    params![doc_id, doc.revision, doc.updated_at.clone().unwrap_or_else(now), now()],
                )?;
            }
            return Ok(None);
        }
    }
    // Changed (or new): replace document + derived rows.
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
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, NULL, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            doc_id,
            source_id,
            doc.external_id,
            doc.canonical_ref,
            doc.title,
            doc.url,
            doc.mime,
            doc.kind,
            doc.author,
            doc.created_at,
            doc.updated_at.clone().unwrap_or_else(now),
            now(),
            doc.revision,
            content_sha,
            doc.body,
            json!({"extractor": crate::index::EXTRACTOR_VERSION}).to_string(),
        ],
    )?;
    let chunks = crate::index::sync::ingest_chunks(conn, source_id, &doc_id, &doc.body)?;
    // Deterministic edge graph (§8.4).
    if let Some(author) = &doc.author {
        insert_edge(conn, &doc_id, "authored_by", "person", author)?;
    }
    if let Some((container, rel)) = &doc.container {
        insert_edge(
            conn,
            &doc_id,
            rel,
            "container",
            &format!("{source_id}:{container}"),
        )?;
    }
    Ok(Some(chunks))
}

fn insert_edge(
    conn: &Connection,
    doc_id: &str,
    rel: &str,
    to_type: &str,
    to_id: &str,
) -> Result<()> {
    let edge_id = hash_hex(&format!("doc:{doc_id}:{rel}:{to_type}:{to_id}"));
    conn.execute("DELETE FROM edges WHERE edge_id = ?1", [&edge_id])?;
    conn.execute(
        "INSERT INTO edges (edge_id, from_type, from_id, to_type, to_id, rel, confidence, evidence_span_id, created_by, created_at, metadata_json)
         VALUES (?1, 'doc', ?2, ?3, ?4, ?5, 1.0, NULL, 'sync', ?6, '{}')",
        params![edge_id, doc_id, to_type, to_id, rel, now()],
    )?;
    Ok(())
}

/// Prune docs of this source whose external id is not in `keep` — for
/// item-tracked sources only (§6.0).
pub fn prune_source_except(
    conn: &Connection,
    source_id: &str,
    keep: &BTreeSet<String>,
) -> Result<usize> {
    let mut stmt =
        conn.prepare("SELECT doc_id, external_id FROM documents WHERE source_id = ?1")?;
    let rows = stmt.query_map([source_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut deleted = 0usize;
    for (doc_id, ext) in rows.flatten() {
        if !keep.contains(&ext) {
            crate::index::sync::delete_doc(conn, &doc_id)?;
            deleted += 1;
        }
    }
    Ok(deleted)
}

/// Prune docs whose external id starts with a prefix not in the tracked set
/// (e.g. GitHub `owner/repo#` prefixes for untracked repos).
pub fn prune_untracked_prefixes(
    conn: &Connection,
    source_id: &str,
    tracked_prefixes: &[String],
) -> Result<usize> {
    let mut stmt =
        conn.prepare("SELECT doc_id, external_id FROM documents WHERE source_id = ?1")?;
    let rows = stmt.query_map([source_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut deleted = 0usize;
    for (doc_id, ext) in rows.flatten() {
        if !tracked_prefixes.iter().any(|p| ext.starts_with(p.as_str())) {
            crate::index::sync::delete_doc(conn, &doc_id)?;
            deleted += 1;
        }
    }
    Ok(deleted)
}

// ---------------------------------------------------------------------------
// HTTP contract (§6.0)
// ---------------------------------------------------------------------------

pub struct Http {
    headers: Vec<(String, String)>,
    /// Called once on 401; returns a fresh Authorization header value.
    refresh: Option<Box<dyn Fn() -> Option<String>>>,
}

impl Http {
    pub fn new(headers: Vec<(String, String)>) -> Http {
        Http {
            headers,
            refresh: None,
        }
    }

    pub fn with_refresh(mut self, f: impl Fn() -> Option<String> + 'static) -> Http {
        self.refresh = Some(Box::new(f));
        self
    }

    fn call(&mut self, method: &str, url: &str, body: Option<&Value>) -> Result<Value> {
        let mut refreshed = false;
        let mut attempt = 0u32;
        loop {
            let mut req = match method {
                "POST" => ureq::post(url),
                _ => ureq::get(url),
            }
            .timeout(std::time::Duration::from_secs(60));
            for (k, v) in &self.headers {
                req = req.set(k, v);
            }
            let resp = match body {
                Some(b) => req.send_json(b.clone()),
                None => req.call(),
            };
            match resp {
                Ok(r) => {
                    let text = r.into_string()?;
                    if text.trim().is_empty() {
                        return Ok(Value::Null);
                    }
                    return serde_json::from_str(&text)
                        .map_err(|e| anyhow!("bad JSON from {url}: {e}"));
                }
                Err(ureq::Error::Status(401, _)) => {
                    if refreshed {
                        return Err(anyhow!("auth error: 401 from {url} after token refresh"));
                    }
                    refreshed = true;
                    let Some(f) = &self.refresh else {
                        return Err(anyhow!("auth error: 401 from {url}"));
                    };
                    let Some(new_auth) = f() else {
                        return Err(anyhow!("auth error: 401 from {url} (refresh failed)"));
                    };
                    self.headers
                        .retain(|(k, _)| !k.eq_ignore_ascii_case("authorization"));
                    self.headers.push(("Authorization".into(), new_auth));
                }
                Err(ureq::Error::Status(code, r)) if retryable(code) && attempt < 3 => {
                    let wait = r
                        .header("Retry-After")
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(backoff_secs(attempt));
                    std::thread::sleep(std::time::Duration::from_secs(wait.min(60)));
                    attempt += 1;
                }
                Err(ureq::Error::Status(code, r)) => {
                    let body = r.into_string().unwrap_or_default();
                    return Err(anyhow!(
                        "HTTP {code} from {url}: {}",
                        body.chars().take(200).collect::<String>()
                    ));
                }
                Err(e) if attempt < 3 => {
                    let _ = e;
                    std::thread::sleep(std::time::Duration::from_secs(backoff_secs(attempt)));
                    attempt += 1;
                }
                Err(e) => return Err(anyhow!("request failed: {e}")),
            }
        }
    }

    pub fn get(&mut self, url: &str) -> Result<Value> {
        self.call("GET", url, None)
    }

    pub fn post(&mut self, url: &str, body: &Value) -> Result<Value> {
        self.call("POST", url, Some(body))
    }

    /// GET returning raw bytes (binary downloads, e.g. PDFs).
    pub fn get_bytes(&mut self, url: &str) -> Result<Vec<u8>> {
        let mut attempt = 0u32;
        loop {
            let mut req = ureq::get(url).timeout(std::time::Duration::from_secs(60));
            for (k, v) in &self.headers {
                req = req.set(k, v);
            }
            match req.call() {
                Ok(r) => {
                    let mut buf = Vec::new();
                    use std::io::Read;
                    r.into_reader()
                        .take(64 * 1024 * 1024)
                        .read_to_end(&mut buf)?;
                    return Ok(buf);
                }
                Err(ureq::Error::Status(code, r)) if retryable(code) && attempt < 3 => {
                    let wait = r
                        .header("Retry-After")
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(backoff_secs(attempt));
                    std::thread::sleep(std::time::Duration::from_secs(wait.min(60)));
                    attempt += 1;
                }
                Err(e) => return Err(anyhow!("request failed: {e}")),
            }
        }
    }

    /// GET returning the raw body text (exports, file contents).
    pub fn get_text(&mut self, url: &str) -> Result<String> {
        let mut attempt = 0u32;
        loop {
            let mut req = ureq::get(url).timeout(std::time::Duration::from_secs(60));
            for (k, v) in &self.headers {
                req = req.set(k, v);
            }
            match req.call() {
                Ok(r) => return Ok(r.into_string()?),
                Err(ureq::Error::Status(code, r)) if retryable(code) && attempt < 3 => {
                    let wait = r
                        .header("Retry-After")
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(backoff_secs(attempt));
                    std::thread::sleep(std::time::Duration::from_secs(wait.min(60)));
                    attempt += 1;
                }
                Err(e) => return Err(anyhow!("request failed: {e}")),
            }
        }
    }
}

/// Retry 429 and ≥500 up to 4 attempts (§6.0).
pub fn retryable(code: u16) -> bool {
    code == 429 || code >= 500
}

pub fn backoff_secs(attempt: u32) -> u64 {
    2u64.pow(attempt + 1) // 2, 4, 8
}

// ---------------------------------------------------------------------------
// Shared text helpers
// ---------------------------------------------------------------------------

/// Flatten HTML to markdown-lite: headings, bullets, links kept; script,
/// style, and head dropped (§8.5).
pub fn html_to_text(html: &str) -> String {
    let mut s = html.to_string();
    for tag in ["script", "style", "head"] {
        let re = regex::Regex::new(&format!(r"(?is)<{tag}[^>]*>.*?</{tag}>")).unwrap();
        s = re.replace_all(&s, "").to_string();
    }
    for lvl in 1..=6usize {
        let re = regex::Regex::new(&format!(r"(?is)<h{lvl}[^>]*>(.*?)</h{lvl}>")).unwrap();
        let hashes = "#".repeat(lvl);
        s = re.replace_all(&s, format!("\n{hashes} $1\n")).to_string();
    }
    let li = regex::Regex::new(r"(?is)<li[^>]*>(.*?)</li>").unwrap();
    s = li.replace_all(&s, "\n- $1").to_string();
    let a = regex::Regex::new(r#"(?is)<a[^>]*href="([^"]*)"[^>]*>(.*?)</a>"#).unwrap();
    s = a.replace_all(&s, "[$2]($1)").to_string();
    let br = regex::Regex::new(r"(?i)<br\s*/?>|</p>|</div>|</tr>").unwrap();
    s = br.replace_all(&s, "\n").to_string();
    let tag = regex::Regex::new(r"(?s)<[^>]+>").unwrap();
    s = tag.replace_all(&s, " ").to_string();
    let s = s
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    // Collapse whitespace but keep line structure.
    let mut out = String::new();
    for line in s.lines() {
        let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if !collapsed.is_empty() {
            out.push_str(&collapsed);
            out.push('\n');
        }
    }
    out
}

/// Millis-epoch → RFC3339.
#[cfg(test)]
pub fn ms_to_rfc3339(ms: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)
        .map(|t| t.to_rfc3339())
        .unwrap_or_else(now)
}

/// Slack "1727312345.000200"-style ts → RFC3339.
pub fn slack_ts_to_rfc3339(ts: &str) -> String {
    let secs = ts
        .split('.')
        .next()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0)
        .map(|t| t.to_rfc3339())
        .unwrap_or_else(now)
}

/// Discord snowflake for a moment `days` ago (backfill floor).
pub fn snowflake_days_ago(days: i64) -> u64 {
    let ms = chrono::Utc::now().timestamp_millis() - days * 86_400_000;
    (((ms - 1_420_070_400_000).max(0)) as u64) << 22
}

pub fn lookback_days(cfg: &Value, key: &str, default: i64, since: Option<i64>) -> i64 {
    since.unwrap_or_else(|| {
        source_lookback_blocks(key)
            .into_iter()
            .filter_map(|source| cfg[source]["lookback_days"].as_i64())
            .next()
            .or_else(|| config::get_path(cfg, key).and_then(|v| v.as_i64()))
            .unwrap_or(default)
    })
}

fn source_lookback_blocks(key: &str) -> Vec<&str> {
    let source = key.split('.').next().unwrap_or(key);
    match source {
        // Google Drive's source key is `gdocs`, while its tracked-ref config
        // block is `google` (§6.2). Accept both for compatibility.
        "gdocs" => vec!["google", "gdocs"],
        other => vec![other],
    }
}

pub fn tracked_list(cfg: &Value, key: &str) -> Vec<String> {
    config::get_path(cfg, key)
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn nudge_to_stats(n: NotConnected) -> Result<SyncStats> {
    eprintln!("note: {}", n.0);
    Ok(SyncStats::default())
}

// Connector modules follow.
#[path = "cloud/confluence.rs"]
pub mod confluence;
#[path = "cloud/discord.rs"]
pub mod discord;
#[path = "cloud/gdocs.rs"]
pub mod gdocs;
#[path = "cloud/github.rs"]
pub mod github;
#[path = "cloud/granola.rs"]
pub mod granola;
#[path = "cloud/hubspot.rs"]
pub mod hubspot;
#[path = "cloud/jira.rs"]
pub mod jira;
#[path = "cloud/linear.rs"]
pub mod linear;
#[path = "cloud/microsoft.rs"]
pub mod microsoft;
#[path = "cloud/salesforce.rs"]
pub mod salesforce;
#[path = "cloud/slack.rs"]
pub mod slack;
#[path = "cloud/zendesk.rs"]
pub mod zendesk;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_policy_matches_spec() {
        assert!(retryable(429));
        assert!(retryable(500));
        assert!(retryable(503));
        assert!(!retryable(404));
        assert!(!retryable(401));
        assert_eq!(backoff_secs(0), 2);
        assert_eq!(backoff_secs(2), 8);
    }

    #[test]
    fn html_flattens_to_markdown_lite() {
        let html = "<head><title>x</title></head><h2>Title</h2><p>Hello <a href=\"u\">link</a></p><ul><li>one</li></ul><script>bad()</script>";
        let text = html_to_text(html);
        assert!(text.contains("## Title"));
        assert!(text.contains("[link](u)"));
        assert!(text.contains("- one"));
        assert!(!text.contains("bad()"));
        assert!(!text.contains("<p>"));
    }

    #[test]
    fn lookback_prefers_since_then_source_block_then_default() {
        let cfg = serde_json::json!({
            "slack": {
                "lookback_days": 21,
                "channels": [],
            }
        });

        assert_eq!(lookback_days(&cfg, "slack.lookback_days", 14, None), 21);

        let cfg = serde_json::json!({
            "slack": {
                "lookback_days": 9
            }
        });
        assert_eq!(lookback_days(&cfg, "slack.lookback_days", 14, Some(2)), 2);

        let cfg = serde_json::json!({});
        assert_eq!(lookback_days(&cfg, "slack.lookback_days", 14, None), 14);
    }

    #[test]
    fn lookback_accepts_google_block_for_gdocs() {
        let cfg = serde_json::json!({
            "google": {
                "lookback_days": 45
            },
            "gdocs": {
                "lookback_days": 30
            }
        });

        assert_eq!(lookback_days(&cfg, "gdocs.lookback_days", 30, None), 45);

        let cfg = serde_json::json!({
            "gdocs": {
                "lookback_days": 30
            }
        });
        assert_eq!(lookback_days(&cfg, "gdocs.lookback_days", 14, None), 30);
    }

    fn doc(body: &str, rev: &str) -> RemoteDoc {
        RemoteDoc {
            external_id: "o/r#1".into(),
            canonical_ref: "github:o/r#1".into(),
            title: "T".into(),
            url: None,
            author: Some("ana".into()),
            created_at: None,
            updated_at: Some("2026-01-01T00:00:00Z".into()),
            mime: "text/markdown",
            kind: "issue",
            container: Some(("o/r".into(), "in_repo")),
            body: body.into(),
            revision: rev.into(),
        }
    }

    #[test]
    fn ingest_remote_doc_hash_is_reembed_authority() {
        let conn = duckdb::Connection::open_in_memory().unwrap();
        crate::index::ensure_schema(&conn).unwrap();
        // New doc embeds.
        assert!(
            ingest_remote_doc(&conn, "github", &doc("hello world", "r1"))
                .unwrap()
                .is_some()
        );
        // Same content, same revision: no-op.
        assert!(
            ingest_remote_doc(&conn, "github", &doc("hello world", "r1"))
                .unwrap()
                .is_none()
        );
        // Revision bump, identical text: metadata only (§6.0).
        assert!(
            ingest_remote_doc(&conn, "github", &doc("hello world", "r2"))
                .unwrap()
                .is_none()
        );
        let version: String = conn
            .query_row("SELECT version FROM documents", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, "r2");
        // Content change: re-embed.
        assert!(
            ingest_remote_doc(&conn, "github", &doc("hello brave new world", "r3"))
                .unwrap()
                .is_some()
        );
        // Author + container edges exist.
        let edges: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM edges WHERE rel IN ('authored_by','in_repo')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(edges, 2);
    }

    #[test]
    fn prune_helpers_respect_tracked_sets() {
        let conn = duckdb::Connection::open_in_memory().unwrap();
        crate::index::ensure_schema(&conn).unwrap();
        ingest_remote_doc(&conn, "github", &doc("body", "r1")).unwrap();
        // Untracked prefix prunes; tracked prefix keeps.
        assert_eq!(
            prune_untracked_prefixes(&conn, "github", &["o/r#".into()]).unwrap(),
            0
        );
        assert_eq!(
            prune_untracked_prefixes(&conn, "github", &["other/x#".into()]).unwrap(),
            1
        );
    }

    #[test]
    fn timestamp_conversions() {
        assert!(slack_ts_to_rfc3339("1727312345.000200").starts_with("2024-09-26"));
        assert!(ms_to_rfc3339(1_727_312_345_000).starts_with("2024-09-26"));
        // Snowflake floor is monotonic in recency.
        assert!(snowflake_days_ago(1) > snowflake_days_ago(14));
    }
}
