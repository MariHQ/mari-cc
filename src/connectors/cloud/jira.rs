//! Jira connector (SPEC §6.6): one document per issue (summary +
//! description + comments). Per-project `updated >` cursor; prunes
//! untracked projects.

use super::{
    credential_or_nudge, get_meta, ingest_remote_doc, prune_untracked_prefixes, tracked_list, Http,
    RemoteDoc, SyncStats,
};
use crate::authcmd;
use crate::index::set_meta;
use anyhow::Result;
use duckdb::Connection;
use serde_json::Value;

pub fn sync(conn: &Connection, cfg: &Value, rebuild: bool) -> Result<SyncStats> {
    let projects: Vec<String> = tracked_list(cfg, "jira.projects")
        .iter()
        .map(|p| normalize_project(p))
        .collect();
    if projects.is_empty() {
        eprintln!("note: no Jira projects tracked — `mari track jira add PROJ`");
        return Ok(SyncStats::default());
    }
    let cred = match credential_or_nudge("jira") {
        Ok(c) => c,
        Err(n) => return super::nudge_to_stats(n),
    };
    let base = cred["url"]
        .as_str()
        .unwrap_or_default()
        .trim_end_matches('/')
        .to_string();
    // Anonymous mode (§6.6): public instance, no Authorization header.
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

    for project in &projects {
        let cursor_key = format!("jira.since.{project}");
        let cursor = if rebuild {
            None
        } else {
            get_meta(conn, &cursor_key)
        };
        let jql = match &cursor {
            Some(c) => format!("project = {project} AND updated > \"{c}\" ORDER BY updated ASC"),
            None => format!("project = {project} ORDER BY updated ASC"),
        };
        let mut start = 0usize;
        let mut max_updated = cursor.unwrap_or_default();
        loop {
            let url = format!(
                "{base}/rest/api/2/search?jql={}&maxResults=100&startAt={start}&fields=summary,description,comment,reporter,created,updated",
                urlencode(&jql)
            );
            let resp = http.get(&url)?;
            let issues = resp["issues"].as_array().cloned().unwrap_or_default();
            for issue in &issues {
                stats.seen += 1;
                let doc = issue_doc(&base, issue);
                if let Some(u) = issue["fields"]["updated"].as_str() {
                    let cursor_form = to_jql_time(u);
                    if cursor_form > max_updated {
                        max_updated = cursor_form;
                    }
                }
                match ingest_remote_doc(conn, "jira", &doc) {
                    Ok(Some(chunks)) => {
                        stats.changed += 1;
                        stats.chunks += chunks;
                        eprintln!("  jira {}", doc.external_id);
                    }
                    Ok(None) => {}
                    Err(e) => eprintln!("note: jira {} skipped: {e}", doc.external_id),
                }
            }
            if issues.len() < 100 {
                break;
            }
            start += 100;
        }
        if !max_updated.is_empty() {
            set_meta(conn, &cursor_key, &max_updated)?;
        }
    }
    let prefixes: Vec<String> = projects.iter().map(|p| format!("{p}-")).collect();
    stats.deleted += prune_untracked_prefixes(conn, "jira", &prefixes)?;
    Ok(stats)
}

pub fn issue_doc(base: &str, issue: &Value) -> RemoteDoc {
    let key = issue["key"].as_str().unwrap_or_default().to_string();
    let f = &issue["fields"];
    let summary = f["summary"].as_str().unwrap_or("").to_string();
    let mut body = format!(
        "# {summary}\n\n{}\n",
        f["description"].as_str().unwrap_or("")
    );
    for c in f["comment"]["comments"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        body.push_str(&format!(
            "\n---\n{}: {}\n",
            c["author"]["displayName"].as_str().unwrap_or("unknown"),
            c["body"].as_str().unwrap_or("")
        ));
    }
    let project = key.split('-').next().unwrap_or("").to_string();
    RemoteDoc {
        external_id: key.clone(),
        canonical_ref: format!("jira:{key}"),
        title: summary,
        url: Some(format!("{base}/browse/{key}")),
        author: f["reporter"]["displayName"].as_str().map(String::from),
        created_at: f["created"].as_str().map(String::from),
        updated_at: f["updated"].as_str().map(String::from),
        mime: "text/plain",
        kind: "issue",
        container: (!project.is_empty()).then_some((project, "in_project")),
        body,
        revision: f["updated"].as_str().unwrap_or_default().to_string(),
    }
}

fn normalize_project(r: &str) -> String {
    if let Some(k) = r.strip_prefix("jira:") {
        return k.to_string();
    }
    if let Some(i) = r.find("/browse/") {
        let rest = &r[i + 8..];
        let key = rest.split('/').next().unwrap_or(rest);
        return key.split('-').next().unwrap_or(key).to_string();
    }
    r.to_string()
}

/// Jira JQL wants "yyyy/MM/dd HH:mm"; issue timestamps are ISO-ish.
fn to_jql_time(iso: &str) -> String {
    let d = &iso[..iso.len().min(16)];
    d.replace('-', "/").replace('T', " ")
}

fn urlencode(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn issue_doc_maps_key_reporter_comments() {
        let issue = json!({
            "key": "ENG-42",
            "fields": {
                "summary": "Pricing tier change",
                "description": "Move to $12",
                "reporter": {"displayName": "Ana"},
                "created": "2026-01-01T00:00:00.000+0000",
                "updated": "2026-01-02T10:30:00.000+0000",
                "comment": {"comments": [{"author": {"displayName": "Bo"}, "body": "shipped"}]}
            }
        });
        let doc = issue_doc("https://acme.atlassian.net", &issue);
        assert_eq!(doc.external_id, "ENG-42");
        assert_eq!(
            doc.url.as_deref(),
            Some("https://acme.atlassian.net/browse/ENG-42")
        );
        assert!(doc.body.contains("Bo: shipped"));
        assert_eq!(doc.container, Some(("ENG".to_string(), "in_project")));
    }

    #[test]
    fn project_refs_and_jql_time() {
        assert_eq!(normalize_project("jira:ENG"), "ENG");
        assert_eq!(
            normalize_project("https://x.atlassian.net/browse/ENG-9"),
            "ENG"
        );
        assert_eq!(
            to_jql_time("2026-01-02T10:30:00.000+0000"),
            "2026/01/02 10:30"
        );
    }
}
