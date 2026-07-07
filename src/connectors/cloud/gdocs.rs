//! Google Drive connector (SPEC §6.2): rides the user's gcloud session
//! (short-lived access token per sync). Google Docs exported as Markdown
//! (fallback plain text). With nothing tracked, auto-indexes docs the user
//! owns; explicit docs/folders narrow and disable auto-index. Comments
//! become separate docs when `gdocs.comments` is on. PDFs are downloaded
//! and text-extracted through the Unlimited-OCR toolchain (§8.6) — no
//! fallback engines.

use super::{
    credential_or_nudge, get_meta, ingest_remote_doc, lookback_days, tracked_list, Http, RemoteDoc,
    SyncStats,
};
use crate::index::set_meta;
use anyhow::{anyhow, Result};
use duckdb::Connection;
use serde_json::Value;

const DRIVE: &str = "https://www.googleapis.com/drive/v3";
const DOC_MIME: &str = "application/vnd.google-apps.document";

pub fn sync(
    conn: &Connection,
    cfg: &Value,
    rebuild: bool,
    since: Option<i64>,
) -> Result<SyncStats> {
    if let Err(n) = credential_or_nudge("google") {
        return super::nudge_to_stats(n);
    }
    let token = gcloud_token()?;
    let mut http = Http::new(vec![("Authorization".into(), format!("Bearer {token}"))])
        .with_refresh(|| gcloud_token().ok().map(|t| format!("Bearer {t}")));

    let docs = tracked_list(cfg, "google.docs");
    let folders = tracked_list(cfg, "google.folders");
    let lookback = lookback_days(cfg, "gdocs.lookback_days", 30, since);
    let want_comments = cfg["gdocs"]["comments"].as_bool().unwrap_or(true);

    let mut stats = SyncStats::default();
    let mut files: Vec<Value> = Vec::new();

    if docs.is_empty() && folders.is_empty() {
        // Auto-index mode: owned Google Docs, discovery cursor on newest
        // modifiedTime; first sync uses the lookback window (0 = all).
        let cursor = if rebuild {
            None
        } else {
            get_meta(conn, "gdocs.cursor")
        };
        let floor = cursor.or_else(|| {
            (lookback > 0)
                .then(|| (chrono::Utc::now() - chrono::Duration::days(lookback)).to_rfc3339())
        });
        let mut q = format!(
            "'me' in owners and (mimeType = '{DOC_MIME}' or mimeType = 'application/pdf') and trashed = false"
        );
        if let Some(f) = &floor {
            q.push_str(&format!(" and modifiedTime > '{f}'"));
        }
        files.extend(list_files(&mut http, &q)?);
        if let Some(newest) = files
            .iter()
            .filter_map(|f| f["modifiedTime"].as_str())
            .max()
        {
            set_meta(conn, "gdocs.cursor", newest)?;
        }
    } else {
        for d in &docs {
            let id = file_id_from_ref(d);
            if let Ok(meta) = http.get(&format!(
                "{DRIVE}/files/{id}?fields=id,name,mimeType,headRevisionId,modifiedTime,createdTime,owners,webViewLink,trashed"
            )) {
                files.push(meta);
            }
        }
        for f in &folders {
            let id = file_id_from_ref(f);
            collect_folder(&mut http, &id, &mut files)?;
        }
    }

    for file in &files {
        if file["trashed"].as_bool().unwrap_or(false) {
            continue;
        }
        let mime = file["mimeType"].as_str().unwrap_or("");
        let is_pdf = mime == "application/pdf";
        if mime != DOC_MIME && !is_pdf {
            eprintln!(
                "note: gdocs skipping {} ({mime}) — extraction not available in this build",
                file["name"].as_str().unwrap_or("?")
            );
            continue;
        }
        stats.seen += 1;
        let id = file["id"].as_str().unwrap_or_default().to_string();
        // Per-file head-revision id decides fetch (§6.2).
        let revision = file["headRevisionId"]
            .as_str()
            .or_else(|| file["modifiedTime"].as_str())
            .unwrap_or_default()
            .to_string();
        if !rebuild {
            if let Some(stored) = get_meta(conn, &format!("gdocs.rev.{id}")) {
                if stored == revision {
                    continue;
                }
            }
        }
        let body = if is_pdf {
            match download_pdf_text(&mut http, &id) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!(
                        "✗ gdocs PDF {}: {e:#}",
                        file["name"].as_str().unwrap_or("?")
                    );
                    continue;
                }
            }
        } else {
            export_markdown(&mut http, &id)?
        };
        let doc = file_doc(file, body);
        match ingest_remote_doc(conn, "gdocs", &doc) {
            Ok(Some(chunks)) => {
                stats.changed += 1;
                stats.chunks += chunks;
                eprintln!("  gdocs {}", doc.title);
            }
            Ok(None) => {}
            Err(e) => eprintln!("note: gdocs {id} skipped: {e}"),
        }
        set_meta(conn, &format!("gdocs.rev.{id}"), &revision)?;
        if want_comments {
            sync_comments(conn, &mut http, &id, file, &mut stats)?;
        }
    }
    Ok(stats)
}

fn gcloud_token() -> Result<String> {
    let out = std::process::Command::new("gcloud")
        .args(["auth", "print-access-token"])
        .output()
        .map_err(|_| anyhow!("gcloud not found — run `mari auth google` first"))?;
    if !out.status.success() {
        return Err(anyhow!(
            "gcloud session expired — run `gcloud auth login --enable-gdrive-access`"
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn list_files(http: &mut Http, q: &str) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    let mut token: Option<String> = None;
    loop {
        let mut url = format!(
            "{DRIVE}/files?q={}&pageSize=100&fields=nextPageToken,files(id,name,mimeType,headRevisionId,modifiedTime,createdTime,owners,webViewLink,trashed)",
            percent_encoding::utf8_percent_encode(q, percent_encoding::NON_ALPHANUMERIC)
        );
        if let Some(t) = &token {
            url.push_str(&format!("&pageToken={t}"));
        }
        let resp = http.get(&url)?;
        out.extend(resp["files"].as_array().cloned().unwrap_or_default());
        token = resp["nextPageToken"].as_str().map(String::from);
        if token.is_none() {
            return Ok(out);
        }
    }
}

/// Folders are crawled recursively (§6.2).
fn collect_folder(http: &mut Http, folder_id: &str, out: &mut Vec<Value>) -> Result<()> {
    let children = list_files(
        http,
        &format!("'{folder_id}' in parents and trashed = false"),
    )?;
    for c in children {
        if c["mimeType"].as_str() == Some("application/vnd.google-apps.folder") {
            let id = c["id"].as_str().unwrap_or_default().to_string();
            collect_folder(http, &id, out)?;
        } else {
            out.push(c);
        }
    }
    Ok(())
}

/// Download a Drive PDF and extract per §8.6 via the Unlimited-OCR
/// toolchain — no fallbacks.
fn download_pdf_text(http: &mut Http, id: &str) -> Result<String> {
    let bytes = http.get_bytes(&format!("{DRIVE}/files/{id}?alt=media"))?;
    let tmp = std::env::temp_dir().join(format!("mari-gdocs-{id}.pdf"));
    std::fs::write(&tmp, &bytes)?;
    let out = crate::ocr::extract_pdf(&tmp);
    let _ = std::fs::remove_file(&tmp);
    out
}

fn export_markdown(http: &mut Http, id: &str) -> Result<String> {
    for mime in ["text/markdown", "text/plain"] {
        let url = format!(
            "{DRIVE}/files/{id}/export?mimeType={}",
            mime.replace('/', "%2F")
        );
        match http.get_text(&url) {
            Ok(text) => return Ok(text),
            Err(_) => continue,
        }
    }
    Err(anyhow!("export failed for {id}"))
}

fn sync_comments(
    conn: &Connection,
    http: &mut Http,
    file_id: &str,
    file: &Value,
    stats: &mut SyncStats,
) -> Result<()> {
    let resp = match http.get(&format!(
        "{DRIVE}/files/{file_id}/comments?fields=comments(id,content,author,createdTime,modifiedTime,replies(content,author))&pageSize=100"
    )) {
        Ok(r) => r,
        Err(_) => return Ok(()),
    };
    for c in resp["comments"].as_array().cloned().unwrap_or_default() {
        stats.seen += 1;
        let doc = comment_doc(file_id, file, &c);
        match ingest_remote_doc(conn, "gdocs", &doc) {
            Ok(Some(chunks)) => {
                stats.changed += 1;
                stats.chunks += chunks;
            }
            Ok(None) => {}
            Err(e) => eprintln!("note: gdocs comment skipped: {e}"),
        }
    }
    Ok(())
}

pub fn file_doc(file: &Value, body: String) -> RemoteDoc {
    let id = file["id"].as_str().unwrap_or_default().to_string();
    RemoteDoc {
        external_id: id.clone(),
        canonical_ref: format!("gdocs:{id}"),
        title: file["name"].as_str().unwrap_or("").to_string(),
        url: file["webViewLink"].as_str().map(String::from),
        author: file["owners"][0]["displayName"].as_str().map(String::from),
        created_at: file["createdTime"].as_str().map(String::from),
        updated_at: file["modifiedTime"].as_str().map(String::from),
        mime: "text/markdown",
        kind: "file",
        container: None,
        body,
        revision: file["headRevisionId"]
            .as_str()
            .or_else(|| file["modifiedTime"].as_str())
            .unwrap_or_default()
            .to_string(),
    }
}

/// Drive comments (+replies) as separate author-attributed docs
/// `<fileId>#comment:<id>`, mime text/x-comment (§6.2).
pub fn comment_doc(file_id: &str, file: &Value, c: &Value) -> RemoteDoc {
    let cid = c["id"].as_str().unwrap_or_default().to_string();
    let author = c["author"]["displayName"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let mut body = format!("{author}: {}\n", c["content"].as_str().unwrap_or(""));
    for r in c["replies"].as_array().cloned().unwrap_or_default() {
        body.push_str(&format!(
            "{}: {}\n",
            r["author"]["displayName"].as_str().unwrap_or("unknown"),
            r["content"].as_str().unwrap_or("")
        ));
    }
    RemoteDoc {
        external_id: format!("{file_id}#comment:{cid}"),
        canonical_ref: format!("gdocs:{file_id}#comment:{cid}"),
        title: format!("comment on {}", file["name"].as_str().unwrap_or("document")),
        url: file["webViewLink"].as_str().map(String::from),
        author: Some(author),
        created_at: c["createdTime"].as_str().map(String::from),
        updated_at: c["modifiedTime"].as_str().map(String::from),
        mime: "text/x-comment",
        kind: "comment",
        container: Some((file_id.to_string(), "in_repo")),
        body,
        revision: c["modifiedTime"]
            .as_str()
            .or_else(|| c["createdTime"].as_str())
            .unwrap_or_default()
            .to_string(),
    }
}

/// Accept a bare file id or any Drive URL shape.
pub fn file_id_from_ref(r: &str) -> String {
    for marker in ["/d/", "/folders/", "id="] {
        if let Some(i) = r.find(marker) {
            let rest = &r[i + marker.len()..];
            return rest
                .split(|c: char| c == '/' || c == '?' || c == '&' || c == '#')
                .next()
                .unwrap_or(rest)
                .to_string();
        }
    }
    r.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn drive_refs_resolve_to_ids() {
        assert_eq!(file_id_from_ref("abc123"), "abc123");
        assert_eq!(
            file_id_from_ref("https://docs.google.com/document/d/XYZ/edit#h=1"),
            "XYZ"
        );
        assert_eq!(
            file_id_from_ref("https://drive.google.com/drive/folders/FFF?usp=x"),
            "FFF"
        );
    }

    #[test]
    fn comment_docs_are_separate_and_attributed() {
        let file = json!({"name": "Plan", "webViewLink": "w"});
        let c = json!({"id": "c1", "content": "why $12?", "author": {"displayName": "Ana"},
                        "createdTime": "t1", "modifiedTime": "t2",
                        "replies": [{"content": "market", "author": {"displayName": "Bo"}}]});
        let doc = comment_doc("F1", &file, &c);
        assert_eq!(doc.external_id, "F1#comment:c1");
        assert_eq!(doc.mime, "text/x-comment");
        assert!(doc.body.contains("Ana: why $12?"));
        assert!(doc.body.contains("Bo: market"));
    }
}
