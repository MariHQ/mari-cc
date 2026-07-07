//! Zendesk connector (SPEC §6.7): tickets (incremental-export epoch cursor)
//! and help-center articles (paged in full). Never prunes.

use super::{
    credential_or_nudge, get_meta, html_to_text, ingest_remote_doc, tracked_list, Http, RemoteDoc,
    SyncStats,
};
use crate::authcmd;
use crate::index::set_meta;
use anyhow::Result;
use duckdb::Connection;
use serde_json::Value;

pub fn sync(conn: &Connection, cfg: &Value, rebuild: bool) -> Result<SyncStats> {
    let cred = match credential_or_nudge("zendesk") {
        Ok(c) => c,
        Err(n) => return super::nudge_to_stats(n),
    };
    let sub = cred["subdomain"].as_str().unwrap_or_default().to_string();
    let basic = authcmd::base64(&format!(
        "{}/token:{}",
        cred["email"].as_str().unwrap_or(""),
        cred["token"].as_str().unwrap_or("")
    ));
    let mut http = Http::new(vec![("Authorization".into(), format!("Basic {basic}"))]);
    let base = format!("https://{sub}.zendesk.com");

    let include = tracked_list(cfg, "zendesk.include");
    let want_tickets = include.is_empty() || include.iter().any(|i| i.ends_with("tickets"));
    let want_articles = include.is_empty() || include.iter().any(|i| i.ends_with("articles"));
    let brands: Vec<i64> = cfg["zendesk"]["brands"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_i64()).collect())
        .unwrap_or_default();

    let mut stats = SyncStats::default();
    if want_tickets {
        sync_tickets(conn, &mut http, &base, rebuild, &brands, &mut stats)?;
    }
    if want_articles {
        sync_articles(conn, &mut http, &base, &brands, &mut stats)?;
    }
    Ok(stats)
}

fn sync_tickets(
    conn: &Connection,
    http: &mut Http,
    base: &str,
    rebuild: bool,
    brands: &[i64],
    stats: &mut SyncStats,
) -> Result<()> {
    let mut start_time = if rebuild {
        0
    } else {
        get_meta(conn, "zendesk.tickets.start_time")
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0)
    };
    loop {
        let resp = http.get(&format!(
            "{base}/api/v2/incremental/tickets.json?start_time={start_time}"
        ))?;
        for t in resp["tickets"].as_array().cloned().unwrap_or_default() {
            if !brands.is_empty() {
                if let Some(b) = t["brand_id"].as_i64() {
                    if !brands.contains(&b) {
                        continue;
                    }
                }
            }
            if t["status"].as_str() == Some("deleted") {
                continue;
            }
            stats.seen += 1;
            let id = t["id"].as_i64().unwrap_or(0);
            let comments = http
                .get(&format!("{base}/api/v2/tickets/{id}/comments.json"))
                .ok()
                .and_then(|v| v["comments"].as_array().cloned())
                .unwrap_or_default();
            let doc = ticket_doc(base, &t, &comments);
            match ingest_remote_doc(conn, "zendesk", &doc) {
                Ok(Some(chunks)) => {
                    stats.changed += 1;
                    stats.chunks += chunks;
                    eprintln!("  zendesk {}", doc.external_id);
                }
                Ok(None) => {}
                Err(e) => eprintln!("note: zendesk ticket/{id} skipped: {e}"),
            }
        }
        let end = resp["end_time"].as_i64().unwrap_or(0);
        if end > 0 {
            start_time = end;
            set_meta(conn, "zendesk.tickets.start_time", &end.to_string())?;
        }
        if resp["end_of_stream"].as_bool().unwrap_or(true) {
            break;
        }
    }
    Ok(())
}

fn sync_articles(
    conn: &Connection,
    http: &mut Http,
    base: &str,
    brands: &[i64],
    stats: &mut SyncStats,
) -> Result<()> {
    let mut url = format!("{base}/api/v2/help_center/articles.json?per_page=100");
    loop {
        let resp = match http.get(&url) {
            Ok(r) => r,
            Err(e) => {
                // Help center may be disabled — tolerated (articles simply absent).
                eprintln!("note: zendesk articles unavailable: {e}");
                return Ok(());
            }
        };
        for a in resp["articles"].as_array().cloned().unwrap_or_default() {
            if !brands.is_empty() {
                if let Some(b) = a["brand_id"].as_i64() {
                    if !brands.contains(&b) {
                        continue;
                    }
                }
            }
            stats.seen += 1;
            let doc = article_doc(&a);
            match ingest_remote_doc(conn, "zendesk", &doc) {
                Ok(Some(chunks)) => {
                    stats.changed += 1;
                    stats.chunks += chunks;
                    eprintln!("  zendesk {}", doc.external_id);
                }
                Ok(None) => {}
                Err(e) => eprintln!("note: zendesk {} skipped: {e}", doc.external_id),
            }
        }
        match resp["next_page"].as_str() {
            Some(next) if !next.is_empty() => url = next.to_string(),
            _ => break,
        }
    }
    Ok(())
}

pub fn ticket_doc(base: &str, t: &Value, comments: &[Value]) -> RemoteDoc {
    let id = t["id"].as_i64().unwrap_or(0);
    let subject = t["subject"].as_str().unwrap_or("").to_string();
    let mut body = format!(
        "# {subject}\n\n{}\n",
        t["description"].as_str().unwrap_or("")
    );
    for c in comments.iter().skip(1) {
        let vis = if c["public"].as_bool().unwrap_or(true) {
            ""
        } else {
            " (internal)"
        };
        body.push_str(&format!(
            "\n---{vis}\n{}\n",
            c["body"].as_str().unwrap_or("")
        ));
    }
    RemoteDoc {
        external_id: format!("ticket/{id}"),
        canonical_ref: format!("zendesk:ticket/{id}"),
        title: subject,
        url: Some(format!("{base}/agent/tickets/{id}")),
        author: None,
        created_at: t["created_at"].as_str().map(String::from),
        updated_at: t["updated_at"].as_str().map(String::from),
        mime: "text/plain",
        kind: "ticket",
        container: None,
        body,
        revision: t["updated_at"].as_str().unwrap_or_default().to_string(),
    }
}

pub fn article_doc(a: &Value) -> RemoteDoc {
    let id = a["id"].as_i64().unwrap_or(0);
    let title = a["title"].as_str().unwrap_or("").to_string();
    let body = format!(
        "# {title}\n\n{}",
        html_to_text(a["body"].as_str().unwrap_or(""))
    );
    RemoteDoc {
        external_id: format!("article/{id}"),
        canonical_ref: format!("zendesk:article/{id}"),
        title,
        url: a["html_url"].as_str().map(String::from),
        author: None,
        created_at: a["created_at"].as_str().map(String::from),
        updated_at: a["updated_at"].as_str().map(String::from),
        mime: "text/plain",
        kind: "article",
        container: None,
        body,
        revision: a["updated_at"].as_str().unwrap_or_default().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ticket_and_article_docs_map() {
        let t = json!({"id": 9, "subject": "Refund", "description": "Please refund",
                        "created_at": "c", "updated_at": "u"});
        let comments = vec![
            json!({"body": "Please refund", "public": true}),
            json!({"body": "escalating", "public": false}),
        ];
        let doc = ticket_doc("https://acme.zendesk.com", &t, &comments);
        assert_eq!(doc.external_id, "ticket/9");
        assert!(doc.body.contains("(internal)"));
        assert!(doc.body.contains("escalating"));

        let a = json!({"id": 5, "title": "FAQ", "body": "<p>Answer</p>", "html_url": "h",
                        "updated_at": "u2"});
        let ad = article_doc(&a);
        assert_eq!(ad.external_id, "article/5");
        assert!(ad.body.contains("Answer"));
        assert_eq!(ad.revision, "u2");
    }
}
