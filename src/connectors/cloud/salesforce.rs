//! Salesforce connector (SPEC §6.8): Knowledge articles + Cases via SOQL.
//! Whole-collection: never prunes; re-embeds when last-modified advances.
//! Tokens are short-lived and not refreshed — re-auth on 401.

use super::{credential_or_nudge, ingest_remote_doc, tracked_list, Http, RemoteDoc, SyncStats};
use anyhow::Result;
use duckdb::Connection;
use serde_json::Value;

const API: &str = "v59.0";

pub fn sync(conn: &Connection, cfg: &Value, _rebuild: bool) -> Result<SyncStats> {
    let cred = match credential_or_nudge("salesforce") {
        Ok(c) => c,
        Err(n) => return super::nudge_to_stats(n),
    };
    let base = cred["url"]
        .as_str()
        .unwrap_or_default()
        .trim_end_matches('/')
        .to_string();
    let token = cred["token"].as_str().unwrap_or_default().to_string();
    let mut http = Http::new(vec![("Authorization".into(), format!("Bearer {token}"))]);

    let objects = tracked_list(cfg, "salesforce.objects");
    let want_articles = objects.is_empty() || objects.iter().any(|o| o.ends_with("articles"));
    let want_cases = objects.is_empty() || objects.iter().any(|o| o.ends_with("cases"));

    let mut stats = SyncStats::default();
    if want_articles {
        let soql = "SELECT Id, Title, Summary, UrlName, LastModifiedDate, CreatedDate FROM KnowledgeArticleVersion WHERE PublishStatus = 'Online'";
        match query_all(&mut http, &base, soql) {
            Ok(rows) => {
                for r in rows {
                    stats.seen += 1;
                    ingest(conn, &article_doc(&base, &r), &mut stats);
                }
            }
            // Orgs without Knowledge lack the object — log and continue.
            Err(e) => eprintln!("note: salesforce articles unavailable: {e}"),
        }
    }
    if want_cases {
        let soql =
            "SELECT Id, CaseNumber, Subject, Description, LastModifiedDate, CreatedDate FROM Case";
        for r in query_all(&mut http, &base, soql)? {
            stats.seen += 1;
            ingest(conn, &case_doc(&base, &r), &mut stats);
        }
    }
    Ok(stats)
}

fn ingest(conn: &Connection, doc: &RemoteDoc, stats: &mut SyncStats) {
    match ingest_remote_doc(conn, "salesforce", doc) {
        Ok(Some(chunks)) => {
            stats.changed += 1;
            stats.chunks += chunks;
            eprintln!("  salesforce {}", doc.external_id);
        }
        Ok(None) => {}
        Err(e) => eprintln!("note: salesforce {} skipped: {e}", doc.external_id),
    }
}

fn query_all(http: &mut Http, base: &str, soql: &str) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    let mut url = format!(
        "{base}/services/data/{API}/query?q={}",
        percent_encoding::utf8_percent_encode(soql, percent_encoding::NON_ALPHANUMERIC)
    );
    loop {
        let resp = http.get(&url)?;
        out.extend(resp["records"].as_array().cloned().unwrap_or_default());
        match resp["nextRecordsUrl"].as_str() {
            Some(next) => url = format!("{base}{next}"),
            None => return Ok(out),
        }
    }
}

pub fn article_doc(base: &str, r: &Value) -> RemoteDoc {
    let id = r["Id"].as_str().unwrap_or_default().to_string();
    let title = r["Title"].as_str().unwrap_or("").to_string();
    RemoteDoc {
        external_id: format!("article/{id}"),
        canonical_ref: format!("salesforce:article/{id}"),
        title: title.clone(),
        url: Some(format!("{base}/lightning/r/Knowledge__kav/{id}/view")),
        author: None,
        created_at: r["CreatedDate"].as_str().map(String::from),
        updated_at: r["LastModifiedDate"].as_str().map(String::from),
        mime: "text/plain",
        kind: "article",
        container: None,
        body: format!("# {title}\n\n{}\n", r["Summary"].as_str().unwrap_or("")),
        revision: r["LastModifiedDate"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
    }
}

pub fn case_doc(base: &str, r: &Value) -> RemoteDoc {
    let id = r["Id"].as_str().unwrap_or_default().to_string();
    let subject = r["Subject"].as_str().unwrap_or("").to_string();
    let number = r["CaseNumber"].as_str().unwrap_or("").to_string();
    RemoteDoc {
        external_id: format!("case/{id}"),
        canonical_ref: format!("salesforce:case/{id}"),
        title: format!("Case {number}: {subject}"),
        url: Some(format!("{base}/lightning/r/Case/{id}/view")),
        author: None,
        created_at: r["CreatedDate"].as_str().map(String::from),
        updated_at: r["LastModifiedDate"].as_str().map(String::from),
        mime: "text/plain",
        kind: "case",
        container: None,
        body: format!(
            "# {subject}\n\n{}\n",
            r["Description"].as_str().unwrap_or("")
        ),
        revision: r["LastModifiedDate"]
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
    fn docs_map_from_soql_rows() {
        let a = json!({"Id": "kA0", "Title": "Reset password", "Summary": "Steps",
                        "LastModifiedDate": "2026-01-02", "CreatedDate": "2026-01-01"});
        let doc = article_doc("https://acme.my.salesforce.com", &a);
        assert_eq!(doc.external_id, "article/kA0");
        assert!(doc.body.contains("Steps"));

        let c = json!({"Id": "500x", "CaseNumber": "0001", "Subject": "Down",
                        "Description": "prod outage", "LastModifiedDate": "u"});
        let doc = case_doc("https://acme.my.salesforce.com", &c);
        assert_eq!(doc.external_id, "case/500x");
        assert!(doc.title.contains("0001"));
        assert_eq!(doc.revision, "u");
    }
}
