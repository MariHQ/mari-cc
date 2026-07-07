//! Discord connector (SPEC §6.11): one document per message in tracked
//! channels and all text channels of tracked guilds. Per-channel timestamp
//! cursor, snowflake pagination; 14-day first-sync lookback.

use super::{
    credential_or_nudge, get_meta, ingest_remote_doc, lookback_days, prune_source_except,
    snowflake_days_ago, tracked_list, Http, RemoteDoc, SyncStats,
};
use crate::index::set_meta;
use anyhow::Result;
use duckdb::Connection;
use serde_json::Value;
use std::collections::BTreeSet;

const API: &str = "https://discord.com/api/v10";
/// Text channel types: 0 text, 5 announcement, 10–12 threads (§6.11).
const TEXT_TYPES: &[i64] = &[0, 5, 10, 11, 12];

pub fn sync(
    conn: &Connection,
    cfg: &Value,
    rebuild: bool,
    since: Option<i64>,
) -> Result<SyncStats> {
    let channels = tracked_list(cfg, "discord.channels");
    let guilds = tracked_list(cfg, "discord.guilds");
    if channels.is_empty() && guilds.is_empty() {
        eprintln!(
            "note: no Discord channels/guilds tracked — `mari track discord add <channelId>`"
        );
        return Ok(SyncStats::default());
    }
    let cred = match credential_or_nudge("discord") {
        Ok(c) => c,
        Err(n) => return super::nudge_to_stats(n),
    };
    let token = cred["token"].as_str().unwrap_or_default().to_string();
    let mut http = Http::new(vec![("Authorization".into(), format!("Bot {token}"))]);
    let lookback = lookback_days(cfg, "discord.lookback_days", 14, since);

    // Resolve the channel set: explicit channels + all text channels of guilds.
    let mut targets: Vec<(String, String)> = Vec::new(); // (id, name)
    for c in &channels {
        let id = normalize_channel(c);
        let name = http
            .get(&format!("{API}/channels/{id}"))
            .ok()
            .and_then(|v| v["name"].as_str().map(String::from))
            .unwrap_or_else(|| id.clone());
        targets.push((id, name));
    }
    for g in &guilds {
        let gid = g.strip_prefix("discord:guild:").unwrap_or(g);
        let chans = http.get(&format!("{API}/guilds/{gid}/channels"))?;
        for c in chans.as_array().cloned().unwrap_or_default() {
            if TEXT_TYPES.contains(&c["type"].as_i64().unwrap_or(-1)) {
                targets.push((
                    c["id"].as_str().unwrap_or_default().to_string(),
                    c["name"].as_str().unwrap_or("channel").to_string(),
                ));
            }
        }
    }

    let mut stats = SyncStats::default();
    let mut seen_ids: BTreeSet<String> = BTreeSet::new();
    for (channel_id, channel_name) in &targets {
        let cursor_key = format!("discord.cursor.{channel_id}");
        let floor: u64 = if rebuild {
            if lookback > 0 {
                snowflake_days_ago(lookback)
            } else {
                0
            }
        } else {
            get_meta(conn, &cursor_key)
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| {
                    if lookback > 0 {
                        snowflake_days_ago(lookback)
                    } else {
                        0
                    }
                })
        };
        // Backward snowflake pagination from newest until we pass the floor.
        let mut before: Option<String> = None;
        let mut newest: u64 = floor;
        'channel: loop {
            let mut url = format!("{API}/channels/{channel_id}/messages?limit=100");
            if let Some(b) = &before {
                url.push_str(&format!("&before={b}"));
            }
            let batch = match http.get(&url) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("note: discord #{channel_name}: {e}");
                    break;
                }
            };
            let msgs = batch.as_array().cloned().unwrap_or_default();
            if msgs.is_empty() {
                break;
            }
            for m in &msgs {
                let id: u64 = m["id"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0);
                if id <= floor {
                    break 'channel;
                }
                newest = newest.max(id);
                stats.seen += 1;
                let doc = message_doc(channel_id, channel_name, m);
                seen_ids.insert(doc.external_id.clone());
                match ingest_remote_doc(conn, "discord", &doc) {
                    Ok(Some(chunks)) => {
                        stats.changed += 1;
                        stats.chunks += chunks;
                    }
                    Ok(None) => {}
                    Err(e) => eprintln!("note: discord {} skipped: {e}", doc.external_id),
                }
            }
            before = msgs.last().and_then(|m| m["id"].as_str()).map(String::from);
        }
        if newest > floor {
            set_meta(conn, &cursor_key, &newest.to_string())?;
        }
    }
    let _ = (&seen_ids, prune_source_except); // messages are additive; no prune on partial windows
    Ok(stats)
}

fn normalize_channel(r: &str) -> String {
    if let Some(k) = r.strip_prefix("discord:") {
        return k.to_string();
    }
    // https://discord.com/channels/<guild>/<channel>[/<message>]
    if let Some(i) = r.find("/channels/") {
        let parts: Vec<&str> = r[i + 10..].split('/').collect();
        if parts.len() >= 2 {
            return parts[1].to_string();
        }
    }
    r.to_string()
}

pub fn message_doc(channel_id: &str, channel_name: &str, m: &Value) -> RemoteDoc {
    let id = m["id"].as_str().unwrap_or_default().to_string();
    let author = m["author"]["global_name"]
        .as_str()
        .or_else(|| m["author"]["username"].as_str())
        .unwrap_or("unknown")
        .to_string();
    let text = m["content"].as_str().unwrap_or("").to_string();
    let title: String = format!(
        "#{channel_name}: {}",
        text.chars().take(60).collect::<String>()
    );
    let guild = m["guild_id"].as_str().unwrap_or("@me");
    RemoteDoc {
        external_id: format!("{channel_name}/{id}"),
        canonical_ref: format!("discord:{channel_id}/{id}"),
        title,
        url: Some(format!(
            "https://discord.com/channels/{guild}/{channel_id}/{id}"
        )),
        author: Some(author.clone()),
        created_at: m["timestamp"].as_str().map(String::from),
        updated_at: m["edited_timestamp"]
            .as_str()
            .or_else(|| m["timestamp"].as_str())
            .map(String::from),
        mime: "text/plain",
        kind: "message",
        container: Some((channel_id.to_string(), "in_channel")),
        body: format!("{author}: {text}\n"),
        revision: m["edited_timestamp"]
            .as_str()
            .or_else(|| m["timestamp"].as_str())
            .unwrap_or_default()
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn message_doc_maps_and_channel_refs_normalize() {
        let m = json!({"id": "111", "content": "release is out",
                        "author": {"username": "ana"}, "timestamp": "2026-01-01T00:00:00Z",
                        "edited_timestamp": null, "guild_id": "9"});
        let doc = message_doc("C9", "releases", &m);
        assert_eq!(doc.external_id, "releases/111");
        assert!(doc.body.contains("ana: release is out"));
        assert_eq!(
            doc.url.as_deref(),
            Some("https://discord.com/channels/9/C9/111")
        );
        assert_eq!(normalize_channel("discord:123"), "123");
        assert_eq!(
            normalize_channel("https://discord.com/channels/9/456/789"),
            "456"
        );
    }
}
