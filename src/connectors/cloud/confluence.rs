//! Confluence connector (SPEC §6.5): every page of tracked spaces/pages,
//! storage HTML flattened to text with `# title` prepended. Version-number
//! revision; bodies fetched lazily for changed pages; prunes unseen pages.

use super::{
    credential_or_nudge, html_to_text, ingest_remote_doc, prune_source_except, tracked_list, Http,
    RemoteDoc, SyncStats,
};
use crate::authcmd;
use anyhow::Result;
use duckdb::Connection;
use serde_json::Value;
use std::collections::BTreeSet;

pub fn sync(conn: &Connection, cfg: &Value, rebuild: bool) -> Result<SyncStats> {
    let spaces = tracked_list(cfg, "confluence.spaces");
    let pages = tracked_list(cfg, "confluence.pages");
    if spaces.is_empty() && pages.is_empty() {
        eprintln!(
            "note: no Confluence spaces/pages tracked — `mari track confluence add <SPACEKEY>`"
        );
        return Ok(SyncStats::default());
    }
    let cred = match credential_or_nudge("confluence") {
        Ok(c) => c,
        Err(n) => return super::nudge_to_stats(n),
    };
    let base = cred["url"]
        .as_str()
        .unwrap_or_default()
        .trim_end_matches('/')
        .to_string();
    // Anonymous mode (§6.5): public wiki, no Authorization header.
    let headers = match cred["method"].as_str() {
        Some("anonymous") => vec![],
        Some("cloud") => vec![(
            "Authorization".into(),
            format!(
                "Basic {}",
                authcmd::base64(&format!(
                    "{}:{}",
                    cred["email"].as_str().unwrap_or(""),
                    cred["token"].as_str().unwrap_or("")
                ))
            ),
        )],
        _ => vec![(
            "Authorization".into(),
            format!("Bearer {}", cred["token"].as_str().unwrap_or("")),
        )],
    };
    let mut http = Http::new(headers);
    let mut stats = SyncStats::default();
    let mut seen_ids: BTreeSet<String> = BTreeSet::new();

    // Space listings carry metadata; bodies fetched lazily for changed pages.
    for space in &spaces {
        let key = normalize_space(space);
        let mut start = 0usize;
        loop {
            let url = format!(
                "{base}/rest/api/content?type=page&spaceKey={key}&limit=100&start={start}&expand=version"
            );
            let resp = http.get(&url)?;
            let results = resp["results"].as_array().cloned().unwrap_or_default();
            for page in &results {
                sync_page(
                    conn,
                    &mut http,
                    &base,
                    page,
                    rebuild,
                    &mut stats,
                    &mut seen_ids,
                )?;
            }
            if results.len() < 100 {
                break;
            }
            start += 100;
        }
    }
    for page_ref in &pages {
        let id = normalize_page(page_ref);
        let meta = http.get(&format!("{base}/rest/api/content/{id}?expand=version"))?;
        sync_page(
            conn,
            &mut http,
            &base,
            &meta,
            rebuild,
            &mut stats,
            &mut seen_ids,
        )?;
    }

    // Prune unseen pages — but only when we listed complete spaces.
    if !spaces.is_empty() && pages.is_empty() {
        stats.deleted += prune_source_except(conn, "confluence", &seen_ids)?;
    }
    Ok(stats)
}

fn sync_page(
    conn: &Connection,
    http: &mut Http,
    base: &str,
    meta: &Value,
    rebuild: bool,
    stats: &mut SyncStats,
    seen: &mut BTreeSet<String>,
) -> Result<()> {
    let id = meta["id"].as_str().unwrap_or_default().to_string();
    if id.is_empty() {
        return Ok(());
    }
    seen.insert(id.clone());
    stats.seen += 1;
    let version = meta["version"]["number"].as_i64().unwrap_or(0).to_string();
    // Revision decides fetch (§6.0): skip body fetch when version unchanged.
    if !rebuild {
        if let Some(stored) = super::get_meta(conn, &format!("confluence.version.{id}")) {
            if stored == version {
                return Ok(());
            }
        }
    }
    let full = http.get(&format!(
        "{base}/rest/api/content/{id}?expand=body.storage,version,space,history"
    ))?;
    let doc = page_doc(base, &full);
    match ingest_remote_doc(conn, "confluence", &doc) {
        Ok(Some(chunks)) => {
            stats.changed += 1;
            stats.chunks += chunks;
            eprintln!("  confluence {}", doc.title);
        }
        Ok(None) => {}
        Err(e) => eprintln!("note: confluence {id} skipped: {e}"),
    }
    crate::index::set_meta(conn, &format!("confluence.version.{id}"), &version)?;
    Ok(())
}

pub fn page_doc(base: &str, page: &Value) -> RemoteDoc {
    let id = page["id"].as_str().unwrap_or_default().to_string();
    let title = page["title"].as_str().unwrap_or("").to_string();
    let html = page["body"]["storage"]["value"].as_str().unwrap_or("");
    let body = format!("# {title}\n\n{}", html_to_text(html));
    let space = page["space"]["key"].as_str().unwrap_or("").to_string();
    let webui = page["_links"]["webui"].as_str().unwrap_or_default();
    RemoteDoc {
        external_id: id.clone(),
        canonical_ref: format!("confluence:page:{id}"),
        title,
        url: Some(format!("{base}{webui}")),
        author: page["history"]["createdBy"]["displayName"]
            .as_str()
            .map(String::from),
        created_at: page["history"]["createdDate"].as_str().map(String::from),
        updated_at: page["version"]["when"].as_str().map(String::from),
        mime: "text/plain",
        kind: "page",
        container: (!space.is_empty()).then_some((space, "in_project")),
        body,
        revision: page["version"]["number"].as_i64().unwrap_or(0).to_string(),
    }
}

fn normalize_space(r: &str) -> String {
    // Accept SPACEKEY, confluence:SPACEKEY, or a space URL .../spaces/KEY/...
    if let Some(k) = r.strip_prefix("confluence:") {
        return k.to_string();
    }
    if let Some(i) = r.find("/spaces/") {
        let rest = &r[i + 8..];
        return rest.split('/').next().unwrap_or(rest).to_string();
    }
    r.to_string()
}

fn normalize_page(r: &str) -> String {
    if let Some(k) = r.strip_prefix("confluence:page:") {
        return k.to_string();
    }
    if let Some(i) = r.find("/pages/") {
        let rest = &r[i + 7..];
        return rest.split('/').next().unwrap_or(rest).to_string();
    }
    r.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn page_doc_flattens_storage_html_with_title() {
        let page = json!({
            "id": "123", "title": "Rate limits",
            "body": {"storage": {"value": "<h2>Plans</h2><p>Pro gets <b>100</b> rps</p>"}},
            "space": {"key": "ENG"},
            "version": {"number": 7, "when": "2026-01-02T00:00:00Z"},
            "history": {"createdBy": {"displayName": "Ana"}, "createdDate": "2026-01-01T00:00:00Z"},
            "_links": {"webui": "/spaces/ENG/pages/123"}
        });
        let doc = page_doc("https://acme.atlassian.net/wiki", &page);
        assert!(doc.body.starts_with("# Rate limits"));
        assert!(doc.body.contains("## Plans"));
        assert!(doc.body.contains("Pro gets 100 rps"));
        assert_eq!(doc.revision, "7");
        assert_eq!(doc.container, Some(("ENG".to_string(), "in_project")));
    }

    #[test]
    fn refs_normalize() {
        assert_eq!(normalize_space("confluence:ENG"), "ENG");
        assert_eq!(
            normalize_space("https://x.atlassian.net/wiki/spaces/ENG/overview"),
            "ENG"
        );
        assert_eq!(
            normalize_page("https://x.atlassian.net/wiki/spaces/ENG/pages/99/T"),
            "99"
        );
    }
}
