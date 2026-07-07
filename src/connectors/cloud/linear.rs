//! Linear connector (SPEC §6.13, GitHub/Jira pattern): one document per
//! issue (title + description + comments). Per-team `updatedAt` cursor;
//! prunes untracked teams.

use super::{
    credential_or_nudge, get_meta, ingest_remote_doc, prune_untracked_prefixes, tracked_list, Http,
    RemoteDoc, SyncStats,
};
use crate::index::set_meta;
use anyhow::Result;
use duckdb::Connection;
use serde_json::{json, Value};

const API: &str = "https://api.linear.app/graphql";

pub fn sync(conn: &Connection, cfg: &Value, rebuild: bool) -> Result<SyncStats> {
    let teams: Vec<String> = tracked_list(cfg, "linear.teams")
        .iter()
        .map(|t| normalize_team(t))
        .collect();
    if teams.is_empty() {
        eprintln!("note: no Linear teams tracked — `mari track linear add TEAM`");
        return Ok(SyncStats::default());
    }
    let cred = match credential_or_nudge("linear") {
        Ok(c) => c,
        Err(n) => return super::nudge_to_stats(n),
    };
    let token = cred["token"].as_str().unwrap_or_default().to_string();
    let mut http = Http::new(vec![
        ("Authorization".into(), token),
        ("Content-Type".into(), "application/json".into()),
    ]);

    let mut stats = SyncStats::default();
    for team in &teams {
        let cursor_key = format!("linear.since.{team}");
        let cursor = if rebuild {
            None
        } else {
            get_meta(conn, &cursor_key)
        };
        let mut after: Option<String> = None;
        let mut max_updated = cursor.clone().unwrap_or_default();
        loop {
            let filter = match &cursor {
                Some(c) => format!(
                    r#"{{ team: {{ key: {{ eq: "{team}" }} }}, updatedAt: {{ gt: "{c}" }} }}"#
                ),
                None => format!(r#"{{ team: {{ key: {{ eq: "{team}" }} }} }}"#),
            };
            let after_arg = after
                .as_ref()
                .map(|a| format!(r#", after: "{a}""#))
                .unwrap_or_default();
            let query = format!(
                r#"{{ issues(filter: {filter}, first: 50{after_arg}) {{
                     pageInfo {{ hasNextPage endCursor }}
                     nodes {{ identifier title description url updatedAt createdAt
                              creator {{ name }}
                              comments {{ nodes {{ body user {{ name }} }} }} }} }} }}"#
            );
            let resp = http.post(API, &json!({ "query": query }))?;
            if let Some(errors) = resp["errors"].as_array() {
                if !errors.is_empty() {
                    return Err(anyhow::anyhow!(
                        "linear GraphQL error: {}",
                        errors[0]["message"]
                    ));
                }
            }
            let issues = &resp["data"]["issues"];
            for node in issues["nodes"].as_array().cloned().unwrap_or_default() {
                stats.seen += 1;
                let doc = issue_doc(team, &node);
                if let Some(u) = node["updatedAt"].as_str() {
                    if u > max_updated.as_str() {
                        max_updated = u.to_string();
                    }
                }
                match ingest_remote_doc(conn, "linear", &doc) {
                    Ok(Some(chunks)) => {
                        stats.changed += 1;
                        stats.chunks += chunks;
                        eprintln!("  linear {}", doc.external_id);
                    }
                    Ok(None) => {}
                    Err(e) => eprintln!("note: linear {} skipped: {e}", doc.external_id),
                }
            }
            if issues["pageInfo"]["hasNextPage"].as_bool().unwrap_or(false) {
                after = issues["pageInfo"]["endCursor"].as_str().map(String::from);
            } else {
                break;
            }
        }
        if !max_updated.is_empty() {
            set_meta(conn, &cursor_key, &max_updated)?;
        }
    }
    let prefixes: Vec<String> = teams.iter().map(|t| format!("{t}-")).collect();
    stats.deleted += prune_untracked_prefixes(conn, "linear", &prefixes)?;
    Ok(stats)
}

pub fn issue_doc(team: &str, node: &Value) -> RemoteDoc {
    let id = node["identifier"].as_str().unwrap_or_default().to_string();
    let title = node["title"].as_str().unwrap_or("").to_string();
    let mut body = format!(
        "# {title}\n\n{}\n",
        node["description"].as_str().unwrap_or("")
    );
    for c in node["comments"]["nodes"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        body.push_str(&format!(
            "\n---\n{}: {}\n",
            c["user"]["name"].as_str().unwrap_or("unknown"),
            c["body"].as_str().unwrap_or("")
        ));
    }
    RemoteDoc {
        external_id: id.clone(),
        canonical_ref: format!("linear:{id}"),
        title,
        url: node["url"].as_str().map(String::from),
        author: node["creator"]["name"].as_str().map(String::from),
        created_at: node["createdAt"].as_str().map(String::from),
        updated_at: node["updatedAt"].as_str().map(String::from),
        mime: "text/markdown",
        kind: "issue",
        container: Some((team.to_string(), "in_project")),
        body,
        revision: node["updatedAt"].as_str().unwrap_or_default().to_string(),
    }
}

fn normalize_team(r: &str) -> String {
    if let Some(k) = r.strip_prefix("linear:") {
        return k.to_string();
    }
    // https://linear.app/<org>/team/<KEY>/...
    if let Some(i) = r.find("/team/") {
        let rest = &r[i + 6..];
        return rest.split('/').next().unwrap_or(rest).to_string();
    }
    r.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn issue_doc_maps_comments_and_team_refs() {
        let node = json!({
            "identifier": "ENG-7", "title": "Ship pricing", "description": "New tiers",
            "url": "https://linear.app/acme/issue/ENG-7", "updatedAt": "2026-01-02",
            "createdAt": "2026-01-01", "creator": {"name": "Ana"},
            "comments": {"nodes": [{"body": "done", "user": {"name": "Bo"}}]}
        });
        let doc = issue_doc("ENG", &node);
        assert_eq!(doc.external_id, "ENG-7");
        assert!(doc.body.contains("Bo: done"));
        assert_eq!(doc.container, Some(("ENG".to_string(), "in_project")));
        assert_eq!(
            normalize_team("https://linear.app/acme/team/ENG/active"),
            "ENG"
        );
        assert_eq!(normalize_team("linear:OPS"), "OPS");
    }
}
