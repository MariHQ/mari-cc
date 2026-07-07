//! GitHub connector (SPEC §6.3): issues + PRs (title, body, comments) of
//! tracked repos. Per-repo `updated_at` high-water cursor; prunes untracked
//! repos' docs. No auto-index, no lookback.

use super::{
    credential_or_nudge, get_meta, ingest_remote_doc, prune_untracked_prefixes, tracked_list, Http,
    RemoteDoc, SyncStats,
};
use crate::index::set_meta;
use anyhow::Result;
use duckdb::Connection;
use serde_json::Value;

pub fn sync(conn: &Connection, cfg: &Value, rebuild: bool) -> Result<SyncStats> {
    let repos = tracked_list(cfg, "github.repos");
    if repos.is_empty() {
        eprintln!("note: no GitHub repos tracked — `mari track github add owner/repo`");
        return Ok(SyncStats::default());
    }
    let cred = match credential_or_nudge("github") {
        Ok(c) => c,
        Err(n) => return super::nudge_to_stats(n),
    };
    let token = cred["token"].as_str().unwrap_or_default().to_string();
    let mut http = Http::new(vec![
        ("Authorization".into(), format!("Bearer {token}")),
        ("User-Agent".into(), "mari".into()),
        ("Accept".into(), "application/vnd.github+json".into()),
    ]);
    let include = tracked_list(cfg, "github.include");
    let want_issues = include.is_empty() || include.iter().any(|i| i == "issues");
    let want_pulls = include.is_empty() || include.iter().any(|i| i == "pulls");

    let mut stats = SyncStats::default();
    for repo in &repos {
        let cursor_key = format!("github.since.{repo}");
        let since = if rebuild {
            None
        } else {
            get_meta(conn, &cursor_key)
        };
        let mut max_updated = since.clone().unwrap_or_default();
        let mut page = 1usize;
        loop {
            let mut url = format!(
                "https://api.github.com/repos/{repo}/issues?state=all&sort=updated&direction=asc&per_page=100&page={page}"
            );
            if let Some(s) = &since {
                url.push_str(&format!("&since={s}"));
            }
            let items = http.get(&url)?;
            let Some(arr) = items.as_array() else { break };
            if arr.is_empty() {
                break;
            }
            for item in arr {
                let is_pr = item.get("pull_request").is_some();
                if (is_pr && !want_pulls) || (!is_pr && !want_issues) {
                    continue;
                }
                stats.seen += 1;
                let comments = if item["comments"].as_i64().unwrap_or(0) > 0 {
                    item["comments_url"]
                        .as_str()
                        .and_then(|u| http.get(u).ok())
                        .and_then(|v| v.as_array().cloned())
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                let doc = issue_doc(repo, item, &comments);
                if let Some(u) = item["updated_at"].as_str() {
                    if u > max_updated.as_str() {
                        max_updated = u.to_string();
                    }
                }
                match ingest_remote_doc(conn, "github", &doc) {
                    Ok(Some(chunks)) => {
                        stats.changed += 1;
                        stats.chunks += chunks;
                        eprintln!("  github {}", doc.external_id);
                    }
                    Ok(None) => {}
                    Err(e) => eprintln!("note: github {} skipped: {e}", doc.external_id),
                }
            }
            if arr.len() < 100 {
                break;
            }
            page += 1;
        }
        if !max_updated.is_empty() {
            set_meta(conn, &cursor_key, &max_updated)?;
        }
    }
    // Prune docs of untracked repos (§6.3).
    let prefixes: Vec<String> = repos.iter().map(|r| format!("{r}#")).collect();
    stats.deleted += prune_untracked_prefixes(conn, "github", &prefixes)?;
    Ok(stats)
}

pub fn issue_doc(repo: &str, item: &Value, comments: &[Value]) -> RemoteDoc {
    let number = item["number"].as_i64().unwrap_or(0);
    let title = item["title"].as_str().unwrap_or("").to_string();
    let mut body = format!("# {title}\n\n{}\n", item["body"].as_str().unwrap_or(""));
    for c in comments {
        body.push_str(&format!(
            "\n---\n{}: {}\n",
            c["user"]["login"].as_str().unwrap_or("unknown"),
            c["body"].as_str().unwrap_or("")
        ));
    }
    let kind = if item.get("pull_request").is_some() {
        "pull"
    } else {
        "issue"
    };
    RemoteDoc {
        external_id: format!("{repo}#{number}"),
        canonical_ref: format!("github:{repo}#{number}"),
        title,
        url: item["html_url"].as_str().map(String::from),
        author: item["user"]["login"].as_str().map(String::from),
        created_at: item["created_at"].as_str().map(String::from),
        updated_at: item["updated_at"].as_str().map(String::from),
        mime: "text/markdown",
        kind,
        container: Some((repo.to_string(), "in_repo")),
        body,
        revision: item["updated_at"].as_str().unwrap_or_default().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn issue_doc_maps_fields_and_detects_prs() {
        let item = json!({
            "number": 42, "title": "Fix pricing", "body": "See details",
            "html_url": "https://github.com/o/r/issues/42",
            "user": {"login": "ana"},
            "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-02T00:00:00Z"
        });
        let comments = vec![json!({"user": {"login": "bo"}, "body": "LGTM"})];
        let doc = issue_doc("o/r", &item, &comments);
        assert_eq!(doc.external_id, "o/r#42");
        assert_eq!(doc.kind, "issue");
        assert!(doc.body.contains("bo: LGTM"));
        assert_eq!(doc.revision, "2026-01-02T00:00:00Z");

        let pr =
            json!({"number": 7, "title": "t", "pull_request": {}, "user": {}, "updated_at": "x"});
        assert_eq!(issue_doc("o/r", &pr, &[]).kind, "pull");
    }
}
