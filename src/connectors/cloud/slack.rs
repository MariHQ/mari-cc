//! Slack connector (SPEC §6.1): one document per thread (root + replies),
//! one per standalone message. Per-channel timestamp cursor plus a trailing
//! 7-day re-scan window; first sync backfills `slack.lookback_days`.

use super::{
    credential_or_nudge, get_meta, ingest_remote_doc, lookback_days, slack_ts_to_rfc3339,
    tracked_list, Http, RemoteDoc, SyncStats,
};
use crate::index::set_meta;
use anyhow::Result;
use duckdb::Connection;
use serde_json::Value;
use std::collections::HashMap;

const RESCAN_SECS: f64 = 7.0 * 86_400.0;

pub fn sync(
    conn: &Connection,
    cfg: &Value,
    rebuild: bool,
    since: Option<i64>,
) -> Result<SyncStats> {
    let cred = match credential_or_nudge("slack") {
        Ok(c) => c,
        Err(n) => return super::nudge_to_stats(n),
    };
    let token = cred["token"].as_str().unwrap_or_default().to_string();
    let team_url = cred["url"]
        .as_str()
        .unwrap_or("https://slack.com/")
        .to_string();
    let mut http = Http::new(vec![("Authorization".into(), format!("Bearer {token}"))]);

    let lookback = lookback_days(cfg, "slack.lookback_days", 14, since);
    let mut stats = SyncStats::default();

    // Channel universe: everything the token is a member of; the tracked
    // `channels` list (or all/*) narrows.
    let all = member_channels(&mut http)?;
    let tracked = tracked_list(cfg, "slack.channels");
    let selected: Vec<(String, String)> =
        if tracked.is_empty() || tracked.iter().any(|t| t == "all" || t == "*") {
            all
        } else {
            all.into_iter()
                .filter(|(id, name)| {
                    tracked
                        .iter()
                        .any(|t| t == id || t == name || t.trim_start_matches('#') == name)
                })
                .collect()
        };

    let users = user_directory(conn, &mut http)?;

    for (channel_id, channel_name) in selected {
        let cursor_key = format!("slack.cursor.{channel_id}");
        let cursor: Option<f64> = if rebuild {
            None
        } else {
            get_meta(conn, &cursor_key).and_then(|v| v.parse().ok())
        };
        let oldest = match cursor {
            Some(ts) => (ts - RESCAN_SECS).max(0.0),
            None if lookback > 0 => {
                chrono::Utc::now().timestamp() as f64 - lookback as f64 * 86_400.0
            }
            None => 0.0,
        };
        let mut max_ts: f64 = cursor.unwrap_or(0.0);
        let mut page_cursor: Option<String> = None;
        loop {
            let mut url = format!(
                "https://slack.com/api/conversations.history?channel={channel_id}&limit=200&oldest={oldest}"
            );
            if let Some(c) = &page_cursor {
                url.push_str(&format!("&cursor={c}"));
            }
            let resp = http.get(&url)?;
            if !resp["ok"].as_bool().unwrap_or(false) {
                eprintln!("note: slack #{channel_name}: {}", resp["error"]);
                break;
            }
            for msg in resp["messages"].as_array().cloned().unwrap_or_default() {
                let ts = msg["ts"].as_str().unwrap_or_default().to_string();
                if let Ok(t) = ts.parse::<f64>() {
                    max_ts = max_ts.max(t);
                }
                let thread_ts = msg["thread_ts"].as_str().map(String::from);
                let is_root = thread_ts.as_deref().map(|t| t == ts).unwrap_or(true);
                if !is_root {
                    continue; // replies are folded into their root's document
                }
                let replies = if msg["reply_count"].as_i64().unwrap_or(0) > 0 {
                    thread_replies(&mut http, &channel_id, &ts)?
                } else {
                    Vec::new()
                };
                stats.seen += 1;
                let doc = thread_doc(
                    &channel_id,
                    &channel_name,
                    &team_url,
                    &msg,
                    &replies,
                    &users,
                );
                match ingest_remote_doc(conn, "slack", &doc) {
                    Ok(Some(chunks)) => {
                        stats.changed += 1;
                        stats.chunks += chunks;
                        eprintln!("  slack #{channel_name} {}", doc.title);
                    }
                    Ok(None) => {}
                    Err(e) => eprintln!("note: slack doc {} skipped: {e}", doc.external_id),
                }
            }
            page_cursor = resp["response_metadata"]["next_cursor"]
                .as_str()
                .filter(|c| !c.is_empty())
                .map(String::from);
            if page_cursor.is_none() {
                break;
            }
        }
        if max_ts > 0.0 {
            set_meta(conn, &cursor_key, &format!("{max_ts}"))?;
        }
    }
    Ok(stats)
}

fn member_channels(http: &mut Http) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut url = "https://slack.com/api/users.conversations?types=public_channel,private_channel,mpim,im&limit=200".to_string();
        if let Some(c) = &cursor {
            url.push_str(&format!("&cursor={c}"));
        }
        let resp = http.get(&url)?;
        if !resp["ok"].as_bool().unwrap_or(false) {
            // Missing groups:read degrades to public channels — logged, not fatal (§6.1).
            eprintln!("note: slack channel listing degraded: {}", resp["error"]);
            let fallback = http
                .get("https://slack.com/api/conversations.list?types=public_channel&limit=200")?;
            for c in fallback["channels"].as_array().cloned().unwrap_or_default() {
                if c["is_member"].as_bool().unwrap_or(false) {
                    out.push((
                        c["id"].as_str().unwrap_or_default().to_string(),
                        c["name"].as_str().unwrap_or_default().to_string(),
                    ));
                }
            }
            return Ok(out);
        }
        for c in resp["channels"].as_array().cloned().unwrap_or_default() {
            out.push((
                c["id"].as_str().unwrap_or_default().to_string(),
                c["name"].as_str().unwrap_or("dm").to_string(),
            ));
        }
        cursor = resp["response_metadata"]["next_cursor"]
            .as_str()
            .filter(|c| !c.is_empty())
            .map(String::from);
        if cursor.is_none() {
            return Ok(out);
        }
    }
}

fn thread_replies(http: &mut Http, channel: &str, root_ts: &str) -> Result<Vec<Value>> {
    let resp = http.get(&format!(
        "https://slack.com/api/conversations.replies?channel={channel}&ts={root_ts}&limit=200"
    ))?;
    Ok(resp["messages"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|m| m["ts"].as_str() != Some(root_ts))
        .collect())
}

/// User directory, cached in catalog state (§6.1).
fn user_directory(conn: &Connection, http: &mut Http) -> Result<HashMap<String, String>> {
    if let Some(cached) = get_meta(conn, "slack.users") {
        if let Ok(map) = serde_json::from_str(&cached) {
            return Ok(map);
        }
    }
    let mut map = HashMap::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut url = "https://slack.com/api/users.list?limit=200".to_string();
        if let Some(c) = &cursor {
            url.push_str(&format!("&cursor={c}"));
        }
        let resp = http.get(&url)?;
        if !resp["ok"].as_bool().unwrap_or(false) {
            break;
        }
        for u in resp["members"].as_array().cloned().unwrap_or_default() {
            let id = u["id"].as_str().unwrap_or_default().to_string();
            let name = u["profile"]["display_name"]
                .as_str()
                .filter(|s| !s.is_empty())
                .or_else(|| u["real_name"].as_str())
                .or_else(|| u["name"].as_str())
                .unwrap_or_default()
                .to_string();
            map.insert(id, name);
        }
        cursor = resp["response_metadata"]["next_cursor"]
            .as_str()
            .filter(|c| !c.is_empty())
            .map(String::from);
        if cursor.is_none() {
            break;
        }
    }
    let _ = set_meta(conn, "slack.users", &serde_json::to_string(&map)?);
    Ok(map)
}

pub fn thread_doc(
    channel_id: &str,
    channel_name: &str,
    team_url: &str,
    root: &Value,
    replies: &[Value],
    users: &HashMap<String, String>,
) -> RemoteDoc {
    let ts = root["ts"].as_str().unwrap_or_default();
    let author_of = |m: &Value| -> String {
        m["user"]
            .as_str()
            .map(|id| users.get(id).cloned().unwrap_or_else(|| id.to_string()))
            .or_else(|| m["username"].as_str().map(String::from))
            .unwrap_or_else(|| "unknown".into())
    };
    let mut body = format!(
        "{}: {}\n",
        author_of(root),
        root["text"].as_str().unwrap_or("")
    );
    let mut last_ts = ts.to_string();
    for r in replies {
        body.push_str(&format!(
            "{}: {}\n",
            author_of(r),
            r["text"].as_str().unwrap_or("")
        ));
        if let Some(t) = r["ts"].as_str() {
            last_ts = t.to_string();
        }
    }
    let permalink = format!(
        "{}archives/{channel_id}/p{}",
        team_url.trim_end_matches('/').to_string() + "/",
        ts.replace('.', "")
    );
    let first_line: String = root["text"]
        .as_str()
        .unwrap_or("")
        .chars()
        .take(80)
        .collect();
    RemoteDoc {
        external_id: format!("{channel_id}/{ts}"),
        canonical_ref: format!("slack:{channel_name}/{ts}"),
        title: format!("#{channel_name}: {first_line}"),
        url: Some(permalink),
        author: Some(author_of(root)),
        created_at: Some(slack_ts_to_rfc3339(ts)),
        updated_at: Some(slack_ts_to_rfc3339(&last_ts)),
        mime: "text/plain",
        kind: "thread",
        container: Some((channel_id.to_string(), "in_channel")),
        body,
        revision: last_ts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn thread_doc_folds_replies_and_builds_permalink() {
        let users: HashMap<String, String> = [
            ("U1".to_string(), "ana".to_string()),
            ("U2".to_string(), "bo".to_string()),
        ]
        .into_iter()
        .collect();
        let root = json!({"ts": "1727312345.000200", "user": "U1", "text": "outage in prod"});
        let replies = vec![json!({"ts": "1727312400.000300", "user": "U2", "text": "on it"})];
        let doc = thread_doc(
            "C1",
            "incidents",
            "https://acme.slack.com/",
            &root,
            &replies,
            &users,
        );
        assert_eq!(doc.external_id, "C1/1727312345.000200");
        assert!(doc.body.contains("ana: outage in prod"));
        assert!(doc.body.contains("bo: on it"));
        assert_eq!(
            doc.url.as_deref(),
            Some("https://acme.slack.com/archives/C1/p1727312345000200")
        );
        assert_eq!(doc.revision, "1727312400.000300");
        assert_eq!(doc.container, Some(("C1".to_string(), "in_channel")));
    }
}
