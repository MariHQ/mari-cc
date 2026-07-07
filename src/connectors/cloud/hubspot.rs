//! HubSpot connector (SPEC §6.9): tickets, notes (HTML→text), KB articles
//! (tolerated-if-absent). Whole-collection: never prunes. Cursor-paged;
//! revision = `updatedAt`.

use super::{
    credential_or_nudge, html_to_text, ingest_remote_doc, tracked_list, Http, RemoteDoc, SyncStats,
};
use anyhow::Result;
use duckdb::Connection;
use serde_json::Value;

pub fn sync(conn: &Connection, cfg: &Value, _rebuild: bool) -> Result<SyncStats> {
    let cred = match credential_or_nudge("hubspot") {
        Ok(c) => c,
        Err(n) => return super::nudge_to_stats(n),
    };
    let token = cred["token"].as_str().unwrap_or_default().to_string();
    let mut http = Http::new(vec![("Authorization".into(), format!("Bearer {token}"))]);

    let include = tracked_list(cfg, "hubspot.include");
    let want = |k: &str| include.is_empty() || include.iter().any(|i| i.ends_with(k));

    let mut stats = SyncStats::default();
    if want("tickets") {
        page_objects(
            &mut http,
            "tickets",
            "subject,content,hs_lastmodifieddate",
            |o| {
                stats.seen += 1;
                ingest(conn, &ticket_doc(o), &mut stats);
            },
        )?;
    }
    if want("notes") {
        page_objects(
            &mut http,
            "notes",
            "hs_note_body,hs_lastmodifieddate",
            |o| {
                stats.seen += 1;
                ingest(conn, &note_doc(o), &mut stats);
            },
        )?;
    }
    if want("kb") {
        // Tolerated-if-absent (§6.9): many portals lack the KB product.
        match http.get(
            "https://api.hubapi.com/cms/v3/site-search/search?q=*&type=KNOWLEDGE_ARTICLE&limit=100",
        ) {
            Ok(resp) => {
                for r in resp["results"].as_array().cloned().unwrap_or_default() {
                    stats.seen += 1;
                    ingest(conn, &kb_doc(&r), &mut stats);
                }
            }
            Err(e) => eprintln!("note: hubspot KB unavailable: {e}"),
        }
    }
    Ok(stats)
}

fn ingest(conn: &Connection, doc: &RemoteDoc, stats: &mut SyncStats) {
    match ingest_remote_doc(conn, "hubspot", doc) {
        Ok(Some(chunks)) => {
            stats.changed += 1;
            stats.chunks += chunks;
            eprintln!("  hubspot {}", doc.external_id);
        }
        Ok(None) => {}
        Err(e) => eprintln!("note: hubspot {} skipped: {e}", doc.external_id),
    }
}

fn page_objects(
    http: &mut Http,
    object: &str,
    properties: &str,
    mut each: impl FnMut(&Value),
) -> Result<()> {
    let mut after: Option<String> = None;
    loop {
        let mut url = format!(
            "https://api.hubapi.com/crm/v3/objects/{object}?limit=100&properties={properties}"
        );
        if let Some(a) = &after {
            url.push_str(&format!("&after={a}"));
        }
        let resp = http.get(&url)?;
        for o in resp["results"].as_array().cloned().unwrap_or_default() {
            each(&o);
        }
        after = resp["paging"]["next"]["after"].as_str().map(String::from);
        if after.is_none() {
            return Ok(());
        }
    }
}

pub fn ticket_doc(o: &Value) -> RemoteDoc {
    let id = o["id"].as_str().unwrap_or_default().to_string();
    let p = &o["properties"];
    let subject = p["subject"].as_str().unwrap_or("").to_string();
    let updated = p["hs_lastmodifieddate"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    RemoteDoc {
        external_id: format!("ticket/{id}"),
        canonical_ref: format!("hubspot:ticket/{id}"),
        title: subject.clone(),
        url: None,
        author: None,
        created_at: o["createdAt"].as_str().map(String::from),
        updated_at: Some(updated.clone()).filter(|s| !s.is_empty()),
        mime: "text/plain",
        kind: "ticket",
        container: None,
        body: format!("# {subject}\n\n{}\n", p["content"].as_str().unwrap_or("")),
        revision: if updated.is_empty() {
            o["updatedAt"].as_str().unwrap_or_default().to_string()
        } else {
            updated
        },
    }
}

pub fn note_doc(o: &Value) -> RemoteDoc {
    let id = o["id"].as_str().unwrap_or_default().to_string();
    let p = &o["properties"];
    let body = html_to_text(p["hs_note_body"].as_str().unwrap_or(""));
    let title: String = body
        .lines()
        .next()
        .unwrap_or("note")
        .chars()
        .take(80)
        .collect();
    RemoteDoc {
        external_id: format!("note/{id}"),
        canonical_ref: format!("hubspot:note/{id}"),
        title,
        url: None,
        author: None,
        created_at: o["createdAt"].as_str().map(String::from),
        updated_at: o["updatedAt"].as_str().map(String::from),
        mime: "text/plain",
        kind: "note",
        container: None,
        body,
        revision: o["updatedAt"].as_str().unwrap_or_default().to_string(),
    }
}

pub fn kb_doc(r: &Value) -> RemoteDoc {
    let id = r["id"]
        .as_str()
        .map(String::from)
        .unwrap_or_else(|| r["id"].as_i64().map(|i| i.to_string()).unwrap_or_default());
    let title = r["title"].as_str().unwrap_or("").to_string();
    RemoteDoc {
        external_id: format!("kb/{id}"),
        canonical_ref: format!("hubspot:kb/{id}"),
        title: title.clone(),
        url: r["url"].as_str().map(String::from),
        author: None,
        created_at: None,
        updated_at: None,
        mime: "text/plain",
        kind: "article",
        container: None,
        body: format!(
            "# {title}\n\n{}\n",
            html_to_text(r["description"].as_str().unwrap_or(""))
        ),
        revision: hash_short(&title),
    }
}

fn hash_short(s: &str) -> String {
    crate::index::hash_hex(s)[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ticket_and_note_docs_map() {
        let t = json!({"id": "1", "createdAt": "c", "updatedAt": "u",
                        "properties": {"subject": "Bug", "content": "It broke",
                                       "hs_lastmodifieddate": "2026-01-02"}});
        let doc = ticket_doc(&t);
        assert_eq!(doc.external_id, "ticket/1");
        assert_eq!(doc.revision, "2026-01-02");
        assert!(doc.body.contains("It broke"));

        let n = json!({"id": "2", "updatedAt": "u2",
                        "properties": {"hs_note_body": "<p>Call with <b>ACME</b></p>"}});
        let nd = note_doc(&n);
        assert_eq!(nd.external_id, "note/2");
        assert!(nd.body.contains("Call with ACME"));
        assert_eq!(nd.title, "Call with ACME");
    }
}
