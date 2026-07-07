//! Microsoft 365 connector (SPEC §6.10): OneDrive/SharePoint files, Outlook
//! mail (one doc per conversation), Teams channel messages. Device-code
//! credential with rotating refresh token. Files prune via delta deletions;
//! mail and Teams never prune. PDFs extract through the Unlimited-OCR
//! toolchain (§8.6, no fallbacks); Office formats are not in this build.

use super::{
    credential_or_nudge, get_meta, ingest_remote_doc, tracked_list, Http, RemoteDoc, SyncStats,
};
use crate::index::set_meta;
use crate::{authcmd, workspace};
use anyhow::{anyhow, Result};
use duckdb::Connection;
use serde_json::Value;
use std::collections::BTreeMap;

const GRAPH: &str = "https://graph.microsoft.com/v1.0";

pub fn sync(conn: &Connection, cfg: &Value, rebuild: bool) -> Result<SyncStats> {
    let drives = tracked_list(cfg, "microsoft.drives");
    let mail = tracked_list(cfg, "microsoft.mail");
    let teams = tracked_list(cfg, "microsoft.teams");
    if drives.is_empty() && mail.is_empty() && teams.is_empty() {
        eprintln!(
            "note: nothing tracked for microsoft — `mari track microsoft add me --list-key drives`"
        );
        return Ok(SyncStats::default());
    }
    let cred = match credential_or_nudge("microsoft") {
        Ok(c) => c,
        Err(n) => return super::nudge_to_stats(n),
    };
    let access = cred["access_token"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let mut http = Http::new(vec![("Authorization".into(), format!("Bearer {access}"))])
        .with_refresh(move || refresh_token().map(|t| format!("Bearer {t}")));

    let mut stats = SyncStats::default();
    for d in &drives {
        if let Err(e) = sync_drive(conn, &mut http, d, rebuild, &mut stats) {
            eprintln!("note: microsoft drive {d}: {e}");
        }
    }
    for folder in &mail {
        if let Err(e) = sync_mail(conn, &mut http, folder, rebuild, &mut stats) {
            eprintln!("note: microsoft mail {folder}: {e}");
        }
    }
    for t in &teams {
        if let Err(e) = sync_teams(conn, &mut http, t, &mut stats) {
            eprintln!("note: microsoft teams {t}: {e}");
        }
    }
    Ok(stats)
}

/// Refresh via the public Azure CLI client; the rotated refresh token is
/// stored back (§6.10).
fn refresh_token() -> Option<String> {
    let cred = authcmd::credential("microsoft")?;
    let refresh = cred["refresh_token"].as_str()?;
    let client_id = cred["client_id"].as_str()?;
    let resp: Value = ureq::post("https://login.microsoftonline.com/common/oauth2/v2.0/token")
        .send_form(&[
            ("client_id", client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh),
            ("scope", cred["scope"].as_str().unwrap_or("")),
        ])
        .ok()?
        .into_json()
        .ok()?;
    let access = resp["access_token"].as_str()?.to_string();
    let mut updated = cred.clone();
    updated["access_token"] = resp["access_token"].clone();
    if resp["refresh_token"].is_string() {
        updated["refresh_token"] = resp["refresh_token"].clone();
    }
    let path = workspace::credentials_dir(
        workspace::source_scope("microsoft") == "global",
        &workspace::work_root(),
    )
    .join("microsoft.json");
    let _ = workspace::write_credential(&path, &updated);
    Some(access)
}

const TEXT_EXTS: &[&str] = &[
    "md", "markdown", "txt", "text", "html", "htm", "csv", "json", "rst",
];

fn sync_drive(
    conn: &Connection,
    http: &mut Http,
    drive_ref: &str,
    rebuild: bool,
    stats: &mut SyncStats,
) -> Result<()> {
    let root = if drive_ref == "me" {
        format!("{GRAPH}/me/drive")
    } else {
        format!(
            "{GRAPH}/drives/{}",
            drive_ref.trim_start_matches("ms:drive:")
        )
    };
    // Delta API: change feed with deletions (files prune on delete).
    let delta_key = format!("microsoft.delta.{drive_ref}");
    let mut url = if rebuild {
        format!("{root}/root/delta")
    } else {
        get_meta(conn, &delta_key).unwrap_or_else(|| format!("{root}/root/delta"))
    };
    loop {
        let resp = http.get(&url)?;
        for item in resp["value"].as_array().cloned().unwrap_or_default() {
            let id = item["id"].as_str().unwrap_or_default().to_string();
            if item.get("deleted").is_some() {
                let doc_id = crate::index::hash_hex(&format!("microsoft:ms:file:{id}"));
                if crate::index::sync::delete_doc(conn, &doc_id).is_ok() {
                    stats.deleted += 1;
                }
                continue;
            }
            if item.get("file").is_none() {
                continue; // folders
            }
            stats.seen += 1;
            let name = item["name"].as_str().unwrap_or("").to_string();
            let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
            let is_pdf = ext == "pdf";
            if !TEXT_EXTS.contains(&ext.as_str()) && !is_pdf {
                eprintln!(
                    "note: microsoft skipping {name} — extraction not available in this build"
                );
                continue;
            }
            // eTag revision decides fetch (§6.10).
            let etag = item["eTag"].as_str().unwrap_or_default().to_string();
            if !rebuild {
                if let Some(stored) = get_meta(conn, &format!("microsoft.etag.{id}")) {
                    if stored == etag {
                        continue;
                    }
                }
            }
            let content_url = format!("{root}/items/{id}/content");
            let body = if is_pdf {
                // PDFs go through the Unlimited-OCR toolchain (§8.6) — no fallbacks.
                match http.get_bytes(&content_url).and_then(|bytes| {
                    let tmp = std::env::temp_dir().join(format!("mari-ms-{id}.pdf"));
                    std::fs::write(&tmp, &bytes)?;
                    let out = crate::ocr::extract_pdf(&tmp);
                    let _ = std::fs::remove_file(&tmp);
                    out
                }) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("✗ microsoft PDF {name}: {e:#}");
                        continue;
                    }
                }
            } else {
                match http.get_text(&content_url) {
                    Ok(b) if ext == "html" || ext == "htm" => super::html_to_text(&b),
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("note: microsoft {name} skipped: {e}");
                        continue;
                    }
                }
            };
            let doc = file_doc(&item, body);
            match ingest_remote_doc(conn, "microsoft", &doc) {
                Ok(Some(chunks)) => {
                    stats.changed += 1;
                    stats.chunks += chunks;
                    eprintln!("  microsoft {name}");
                }
                Ok(None) => {}
                Err(e) => eprintln!("note: microsoft {name} skipped: {e}"),
            }
            set_meta(conn, &format!("microsoft.etag.{id}"), &etag)?;
        }
        if let Some(next) = resp["@odata.nextLink"].as_str() {
            url = next.to_string();
        } else {
            if let Some(delta) = resp["@odata.deltaLink"].as_str() {
                set_meta(conn, &delta_key, delta)?;
            }
            return Ok(());
        }
    }
}

fn sync_mail(
    conn: &Connection,
    http: &mut Http,
    folder: &str,
    rebuild: bool,
    stats: &mut SyncStats,
) -> Result<()> {
    let folder_id = folder.trim_start_matches("ms:mail:");
    let cursor_key = format!("microsoft.mail.{folder_id}");
    let cursor = if rebuild {
        None
    } else {
        get_meta(conn, &cursor_key)
    };
    let filter = cursor
        .as_ref()
        .map(|c| format!("&$filter=receivedDateTime gt {c}"))
        .unwrap_or_default();
    let mut url = format!(
        "{GRAPH}/me/mailFolders/{folder_id}/messages?$top=50&$orderby=receivedDateTime asc&$select=id,subject,bodyPreview,body,conversationId,from,receivedDateTime{filter}"
    );
    // One document per conversation (§6.10).
    let mut conversations: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut newest = String::new();
    loop {
        let resp = http.get(&url)?;
        for m in resp["value"].as_array().cloned().unwrap_or_default() {
            let conv = m["conversationId"].as_str().unwrap_or("none").to_string();
            if let Some(r) = m["receivedDateTime"].as_str() {
                if r > newest.as_str() {
                    newest = r.to_string();
                }
            }
            conversations.entry(conv).or_default().push(m);
        }
        match resp["@odata.nextLink"].as_str() {
            Some(next) => url = next.to_string(),
            None => break,
        }
    }
    for (conv_id, msgs) in conversations {
        stats.seen += 1;
        let doc = conversation_doc(folder_id, &conv_id, &msgs);
        match ingest_remote_doc(conn, "microsoft", &doc) {
            Ok(Some(chunks)) => {
                stats.changed += 1;
                stats.chunks += chunks;
            }
            Ok(None) => {}
            Err(e) => eprintln!("note: microsoft mail {conv_id} skipped: {e}"),
        }
    }
    if !newest.is_empty() {
        set_meta(conn, &cursor_key, &newest)?;
    }
    Ok(())
}

fn sync_teams(
    conn: &Connection,
    http: &mut Http,
    team_ref: &str,
    stats: &mut SyncStats,
) -> Result<()> {
    // Refs: ms:teams:<teamId>/<channelId> or <teamId>/<channelId>.
    let r = team_ref.trim_start_matches("ms:teams:");
    let (team_id, channel_id) = r
        .split_once('/')
        .ok_or_else(|| anyhow!("teams ref must be <teamId>/<channelId>"))?;
    let mut url = format!("{GRAPH}/teams/{team_id}/channels/{channel_id}/messages?$top=50");
    loop {
        let resp = http.get(&url)?;
        for m in resp["value"].as_array().cloned().unwrap_or_default() {
            stats.seen += 1;
            let doc = teams_message_doc(team_id, channel_id, &m);
            match ingest_remote_doc(conn, "microsoft", &doc) {
                Ok(Some(chunks)) => {
                    stats.changed += 1;
                    stats.chunks += chunks;
                }
                Ok(None) => {}
                Err(e) => eprintln!("note: microsoft teams msg skipped: {e}"),
            }
        }
        match resp["@odata.nextLink"].as_str() {
            Some(next) => url = next.to_string(),
            None => return Ok(()),
        }
    }
}

pub fn file_doc(item: &Value, body: String) -> RemoteDoc {
    let id = item["id"].as_str().unwrap_or_default().to_string();
    RemoteDoc {
        external_id: format!("ms:file:{id}"),
        canonical_ref: format!("ms:file:{id}"),
        title: item["name"].as_str().unwrap_or("").to_string(),
        url: item["webUrl"].as_str().map(String::from),
        author: item["lastModifiedBy"]["user"]["displayName"]
            .as_str()
            .map(String::from),
        created_at: item["createdDateTime"].as_str().map(String::from),
        updated_at: item["lastModifiedDateTime"].as_str().map(String::from),
        mime: "text/plain",
        kind: "file",
        container: None,
        body,
        revision: item["eTag"]
            .as_str()
            .or_else(|| item["lastModifiedDateTime"].as_str())
            .unwrap_or_default()
            .to_string(),
    }
}

pub fn conversation_doc(folder: &str, conv_id: &str, msgs: &[Value]) -> RemoteDoc {
    let subject = msgs
        .first()
        .and_then(|m| m["subject"].as_str())
        .unwrap_or("(no subject)")
        .to_string();
    let mut body = format!("# {subject}\n");
    let mut last = String::new();
    for m in msgs {
        let from = m["from"]["emailAddress"]["name"]
            .as_str()
            .or_else(|| m["from"]["emailAddress"]["address"].as_str())
            .unwrap_or("unknown");
        let text = match m["body"]["contentType"].as_str() {
            Some("html") => super::html_to_text(m["body"]["content"].as_str().unwrap_or("")),
            _ => m["body"]["content"]
                .as_str()
                .or_else(|| m["bodyPreview"].as_str())
                .unwrap_or("")
                .to_string(),
        };
        body.push_str(&format!("\n---\n{from}:\n{text}\n"));
        if let Some(r) = m["receivedDateTime"].as_str() {
            last = r.to_string();
        }
    }
    RemoteDoc {
        external_id: format!("mail:{conv_id}"),
        canonical_ref: format!("ms:mail:{folder}/{conv_id}"),
        title: subject,
        url: None,
        author: msgs
            .first()
            .and_then(|m| m["from"]["emailAddress"]["name"].as_str())
            .map(String::from),
        created_at: msgs
            .first()
            .and_then(|m| m["receivedDateTime"].as_str())
            .map(String::from),
        updated_at: Some(last.clone()).filter(|s| !s.is_empty()),
        mime: "text/plain",
        kind: "mail",
        container: Some((folder.to_string(), "in_channel")),
        body,
        revision: last,
    }
}

pub fn teams_message_doc(team_id: &str, channel_id: &str, m: &Value) -> RemoteDoc {
    let id = m["id"].as_str().unwrap_or_default().to_string();
    let author = m["from"]["user"]["displayName"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let text = match m["body"]["contentType"].as_str() {
        Some("html") => super::html_to_text(m["body"]["content"].as_str().unwrap_or("")),
        _ => m["body"]["content"].as_str().unwrap_or("").to_string(),
    };
    RemoteDoc {
        external_id: format!("teams:{team_id}/{channel_id}/{id}"),
        canonical_ref: format!("ms:teams:{team_id}/{channel_id}/{id}"),
        title: format!("teams: {}", text.chars().take(60).collect::<String>()),
        url: m["webUrl"].as_str().map(String::from),
        author: Some(author.clone()),
        created_at: m["createdDateTime"].as_str().map(String::from),
        updated_at: m["lastModifiedDateTime"]
            .as_str()
            .or_else(|| m["createdDateTime"].as_str())
            .map(String::from),
        mime: "text/plain",
        kind: "message",
        // Teams messages carry no revision (§6.10) — created time stands in.
        container: Some((format!("{team_id}/{channel_id}"), "in_channel")),
        body: format!("{author}: {text}\n"),
        revision: m["createdDateTime"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mail_conversation_folds_messages() {
        let msgs = vec![
            json!({"subject": "Q3 launch", "receivedDateTime": "t1",
                    "from": {"emailAddress": {"name": "Ana"}},
                    "body": {"contentType": "text", "content": "Draft ready"}}),
            json!({"subject": "RE: Q3 launch", "receivedDateTime": "t2",
                    "from": {"emailAddress": {"name": "Bo"}},
                    "body": {"contentType": "html", "content": "<p>LGTM</p>"}}),
        ];
        let doc = conversation_doc("inbox", "CONV1", &msgs);
        assert_eq!(doc.external_id, "mail:CONV1");
        assert!(doc.body.contains("Ana:"));
        assert!(doc.body.contains("LGTM"));
        assert_eq!(doc.revision, "t2");
    }

    #[test]
    fn teams_message_maps() {
        let m = json!({"id": "5", "createdDateTime": "t",
                        "from": {"user": {"displayName": "Ana"}},
                        "body": {"contentType": "html", "content": "<b>ship it</b>"}});
        let doc = teams_message_doc("T1", "C1", &m);
        assert_eq!(doc.external_id, "teams:T1/C1/5");
        assert!(doc.body.contains("Ana: ship it"));
    }
}
