//! JSON API for the local console. Every read opens the published catalog
//! read-only; every write reuses the CLI's own write path (tag/lineage/config/
//! track/sync), so the console and the CLI can never disagree about how data is
//! mutated. Handlers are deliberately thin: shape the request, call into the
//! existing modules or run a parametrized SELECT, return JSON.

use crate::{
    assets, authcmd, cloud, config, connectors, curation, detector, docsite, i18n, index, lineage,
    rulescmd, workspace,
};
use anyhow::{anyhow, Result};
use duckdb::types::ValueRef;
use duckdb::Connection;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tiny_http::Method;

/// Serializes API requests. The console operates on the process working
/// directory (that's how `workspace::work_root()` resolves the active project),
/// and project-switching changes it with `set_current_dir`. Holding this lock
/// for the duration of each request makes those cwd reads/writes race-free —
/// fine for a local single user.
static REQUEST_LOCK: Mutex<()> = Mutex::new(());

pub struct Ctx {
    pub method: Method,
    pub path: String,
    pub query: String,
    pub body: String,
}

/// Runtime config the SPA fetches on boot (`/config.json`). Same-origin API,
/// no cloud, no GitHub app — the local console needs almost nothing here.
pub fn runtime_config() -> Value {
    json!({
        "apiBase": "",
        "marketingUrl": "",
        "local": true,
    })
}

/// Local single-user session. The console has no login; we synthesize an
/// always-authenticated identity from git/OS so the SPA renders immediately.
pub fn auth(path: &str) -> Value {
    if path == "/auth/logout" {
        return json!({ "ok": true });
    }
    let (login, name) = local_identity();
    json!({
        "authenticated": true,
        "user": { "login": login, "name": name },
        "orgSlug": "local",
        "orgName": "Local workspace",
        "installations": [],
    })
}

/// Dispatch an `/api/*` request. Returns `(http_status, json_body)`.
pub fn route(ctx: &Ctx) -> Result<(u16, Value)> {
    // Serialize all requests so cwd (active-project) reads/writes never race.
    let _guard = REQUEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let segs: Vec<&str> = ctx.path.trim_matches('/').split('/').collect();
    // segs[0] == "api"
    let rest = &segs[1..];
    match (&ctx.method, rest) {
        (Method::Get, ["status"]) => ok(status()?),
        (Method::Get, ["overview"]) => ok(overview()?),

        (Method::Get, ["sources"]) => ok(sources()?),
        (Method::Post, ["sources", "track"]) => ok(sources_track(ctx)?),
        (Method::Post, ["sources", "sync"]) => ok(sources_sync(ctx)?),

        (Method::Get, ["documents"]) => ok(documents(ctx)?),
        (Method::Get, ["documents", id]) => ok(document(id)?),

        (Method::Get, ["search"]) => ok(search(ctx)?),

        (Method::Get, ["tags"]) => ok(tags()?),
        (Method::Post, ["tags"]) => ok(tags_apply(ctx)?),
        (Method::Delete, ["tags"]) => ok(tags_remove(ctx)?),

        (Method::Get, ["lineage"]) => ok(lineage_list()?),
        (Method::Post, ["lineage"]) => ok(lineage_add(ctx)?),
        (Method::Post, ["lineage", id, "confirm"]) => ok(lineage_status(id, "confirm")?),
        (Method::Post, ["lineage", id, "reject"]) => ok(lineage_status(id, "reject")?),

        (Method::Get, ["facts"]) => ok(facts()?),
        (Method::Get, ["glossary"]) => ok(glossary()?),

        (Method::Get, ["config"]) => ok(config_get()?),
        (Method::Put, ["config"]) => ok(config_set(ctx)?),

        (Method::Get, ["projects"]) => ok(projects_list()?),
        (Method::Post, ["projects", "switch"]) => ok(projects_switch(ctx)?),
        (Method::Post, ["projects", "register"]) => ok(projects_register(ctx)?),

        (Method::Get, ["nudges"]) => ok(nudges_list()?),
        (Method::Post, ["nudges"]) => ok(nudges_add(ctx)?),
        (Method::Delete, ["nudges"]) => ok(nudges_remove(ctx)?),

        (Method::Get, ["rules"]) => ok(rules_list()?),
        (Method::Post, ["rules"]) => ok(rules_add(ctx)?),
        (Method::Post, ["rules", "discover"]) => ok(rules_discover()?),
        (Method::Delete, ["rules"]) => ok(rules_remove(ctx)?),

        (Method::Get, ["detector"]) => ok(detector_get()?),
        (Method::Post, ["detector", "zero"]) => ok(detector_zero(ctx)?),
        (Method::Post, ["detector", "ignore"]) => ok(detector_ignore(ctx)?),
        (Method::Post, ["detect"]) => ok(detect(ctx)?),

        (Method::Get, ["templates"]) => ok(templates_list()?),
        (Method::Post, ["templates", "scaffold"]) => ok(templates_scaffold(ctx)?),

        (Method::Post, ["tags", "statuses"]) => ok(tag_statuses_set(ctx)?),

        (Method::Get, ["localization"]) => ok(i18n::overview_json()),
        (Method::Get, ["localization", "coverage"]) => ok(localization_coverage(ctx)?),
        (Method::Get, ["localization", "file"]) => ok(repo_file(ctx)?),
        (Method::Get, ["docsite"]) => ok(json!({
            "plan": docsite::plan_json(),
            "status": docsite::status_json(),
        })),

        (Method::Get, ["cloud"]) => ok(cloud_status()?),
        (Method::Post, ["cloud", "pull"]) => ok(cloud_pull()?),
        (Method::Post, ["cloud", "sync"]) => ok(cloud_sync(ctx)?),
        (Method::Post, ["cloud", "role"]) => ok(cloud_role(ctx)?),
        (Method::Post, ["cloud", "connect"]) => ok(cloud_connect(ctx, "connect")?),
        (Method::Post, ["cloud", "init"]) => ok(cloud_connect(ctx, "init")?),

        _ => Ok((404, json!({ "error": format!("no route for {} {}", ctx.method, ctx.path) }))),
    }
}

fn ok(v: Value) -> Result<(u16, Value)> {
    Ok((200, v))
}

/* ── request helpers ─────────────────────────────────────────────────────── */

fn root() -> std::path::PathBuf {
    workspace::work_root()
}

fn body_json(ctx: &Ctx) -> Result<Value> {
    if ctx.body.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&ctx.body).map_err(|e| anyhow!("invalid JSON body: {e}"))
}

fn query_param(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return Some(
                    percent_encoding::percent_decode_str(v)
                        .decode_utf8_lossy()
                        .replace('+', " "),
                );
            }
        }
    }
    None
}

fn local_identity() -> (String, String) {
    let name = std::process::Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| std::env::var("USER").unwrap_or_else(|_| "local".into()));
    let login = std::env::var("USER").unwrap_or_else(|_| "local".into());
    (login, name)
}

/* ── DuckDB → JSON ───────────────────────────────────────────────────────── */

fn read_catalog() -> Result<Connection> {
    index::open_catalog_read(false)
}

fn valueref_to_json(v: ValueRef<'_>) -> Value {
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Boolean(b) => Value::Bool(b),
        ValueRef::TinyInt(n) => json!(n),
        ValueRef::SmallInt(n) => json!(n),
        ValueRef::Int(n) => json!(n),
        ValueRef::BigInt(n) => json!(n),
        ValueRef::HugeInt(n) => json!(n as i64),
        ValueRef::UTinyInt(n) => json!(n),
        ValueRef::USmallInt(n) => json!(n),
        ValueRef::UInt(n) => json!(n),
        ValueRef::UBigInt(n) => json!(n),
        ValueRef::Float(n) => json!(n),
        ValueRef::Double(n) => json!(n),
        ValueRef::Text(s) => Value::String(String::from_utf8_lossy(s).into_owned()),
        ValueRef::Blob(b) => Value::String(String::from_utf8_lossy(b).into_owned()),
        _ => Value::Null,
    }
}

/// Run a parametrized read query and return rows as JSON objects keyed by
/// column name. String params only (sufficient for the console's filters).
fn rows_json(conn: &Connection, sql: &str, params: &[&str]) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query(duckdb::params_from_iter(params.iter()))?;
    let names: Vec<String> = rows
        .as_ref()
        .map(|s| s.column_names())
        .unwrap_or_default();
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let mut obj = serde_json::Map::new();
        for (i, name) in names.iter().enumerate() {
            obj.insert(name.clone(), valueref_to_json(row.get_ref(i)?));
        }
        out.push(Value::Object(obj));
    }
    Ok(out)
}

fn scalar_i64(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap_or(0)
}

/* ── status / overview ───────────────────────────────────────────────────── */

fn status() -> Result<Value> {
    let root = root();
    let cfg = config::resolve(Some(&root));
    let conn = read_catalog()?;
    let last_sync: Option<String> = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE key = 'last_sync'",
            [],
            |r| r.get(0),
        )
        .ok();
    let embedding: Option<String> = conn
        .query_row(
            "SELECT DISTINCT model_id FROM embeddings LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok();
    let docs = scalar_i64(&conn, "SELECT count(*)::BIGINT FROM documents");
    let chunks = scalar_i64(&conn, "SELECT count(*)::BIGINT FROM chunks");
    let tags = scalar_i64(&conn, "SELECT count(*)::BIGINT FROM tags");
    let lineage_edges = scalar_i64(&conn, "SELECT count(*)::BIGINT FROM lineage_edges");
    Ok(json!({
        "workspace": root.display().to_string(),
        "catalog": index::catalog_path(false).display().to_string(),
        "lastSync": last_sync,
        "embeddingModel": embedding.unwrap_or_else(|| index::EMBEDDING_MODEL.to_string()),
        "staleDays": cfg["sync"]["stale_days"].as_i64().unwrap_or(7),
        "counts": { "documents": docs, "chunks": chunks, "tags": tags, "lineageEdges": lineage_edges },
        "cloudEnabled": cfg["cloud"]["enabled"].as_bool().unwrap_or(false),
    }))
}

fn overview() -> Result<Value> {
    let conn = read_catalog()?;
    let root = root();
    let cfg = config::resolve(Some(&root));
    let stale_days = cfg["sync"]["stale_days"].as_i64().unwrap_or(7);

    let docs = scalar_i64(&conn, "SELECT count(*)::BIGINT FROM documents");
    let sources_connected = connected_sources_count();
    let proposed = scalar_i64(
        &conn,
        "SELECT count(*)::BIGINT FROM lineage_edges WHERE status = 'proposed'",
    );

    let tag_counts = rows_json(
        &conn,
        "SELECT status, count(*)::BIGINT AS n FROM tags GROUP BY status ORDER BY n DESC",
        &[],
    )?;

    // Freshness: docs updated within stale window vs older, over the docs that
    // carry an updated_at timestamp.
    let cutoff = chrono::Utc::now() - chrono::Duration::days(stale_days.max(1));
    let cutoff_s = cutoff.to_rfc3339();
    let fresh = conn
        .query_row(
            "SELECT count(*)::BIGINT FROM documents WHERE updated_at >= ?1",
            [&cutoff_s],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0);
    let with_ts = scalar_i64(
        &conn,
        "SELECT count(*)::BIGINT FROM documents WHERE updated_at IS NOT NULL",
    );
    let stale = (with_ts - fresh).max(0);

    let recent_syncs = rows_json(
        &conn,
        "SELECT se.source_id, se.status, se.started_at, se.finished_at, \
                se.docs_seen, se.docs_changed, se.error \
           FROM sync_events se ORDER BY se.started_at DESC LIMIT 12",
        &[],
    )?;

    let per_source = rows_json(
        &conn,
        "SELECT s.provider AS provider, count(d.doc_id)::BIGINT AS documents \
           FROM sources s LEFT JOIN documents d ON d.source_id = s.source_id \
          GROUP BY s.provider ORDER BY documents DESC",
        &[],
    )?;

    Ok(json!({
        "kpis": {
            "documents": docs,
            "sourcesConnected": sources_connected,
            "proposedLineage": proposed,
            "tags": scalar_i64(&conn, "SELECT count(*)::BIGINT FROM tags"),
        },
        "tagCounts": tag_counts,
        "freshness": { "fresh": fresh, "stale": stale },
        "perSource": per_source,
        "recentSyncs": recent_syncs,
    }))
}

/* ── sources / connectors ────────────────────────────────────────────────── */

/// The mari source registry with the tracked-ref config keys per source, mirror
/// of `connectors::list_keys` (kept private there).
const SOURCE_LIST_KEYS: &[(&str, &[&str])] = &[
    ("slack", &["slack.channels"]),
    ("gdocs", &["google.docs", "google.folders"]),
    ("github", &["github.repos"]),
    ("git", &["git.repos"]),
    ("confluence", &["confluence.spaces", "confluence.pages"]),
    ("jira", &["jira.projects"]),
    ("zendesk", &["zendesk.include"]),
    ("salesforce", &["salesforce.objects"]),
    ("hubspot", &["hubspot.include"]),
    ("microsoft", &["microsoft.drives", "microsoft.mail", "microsoft.teams"]),
    ("discord", &["discord.channels", "discord.guilds"]),
    ("linear", &["linear.teams", "linear.projects"]),
    ("granola", &["granola.folders"]),
    ("localfiles", &["localfiles.paths"]),
];

/// The auth provider that backs a source (google↔gdocs; localfiles/git/granola
/// need no credential).
fn auth_provider_for(source: &str) -> Option<&'static str> {
    match source {
        "gdocs" => Some("google"),
        "slack" => Some("slack"),
        "github" => Some("github"),
        "confluence" => Some("confluence"),
        "jira" => Some("jira"),
        "zendesk" => Some("zendesk"),
        "salesforce" => Some("salesforce"),
        "hubspot" => Some("hubspot"),
        "microsoft" => Some("microsoft"),
        "discord" => Some("discord"),
        "linear" => Some("linear"),
        _ => None, // git, granola, localfiles: no remote credential
    }
}

fn connected_sources_count() -> i64 {
    SOURCE_LIST_KEYS
        .iter()
        .filter(|(source, _)| is_connected(source))
        .count() as i64
}

fn is_connected(source: &str) -> bool {
    match auth_provider_for(source) {
        Some(provider) => authcmd::credential(provider).is_some(),
        None => true, // credential-free sources are always "available"
    }
}

fn sources() -> Result<Value> {
    let root = root();
    let cfg = config::resolve(Some(&root));
    let conn = read_catalog()?;

    // Indexed doc count + last sync per provider from the catalog.
    let mut list = Vec::new();
    for (source, keys) in SOURCE_LIST_KEYS {
        let provider = auth_provider_for(source);
        let connected = is_connected(source);
        let tracked: Vec<Value> = keys
            .iter()
            .map(|k| {
                let vals = config::get_path(&cfg, k)
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                json!({ "key": k, "refs": vals })
            })
            .collect();
        let indexed = conn
            .query_row(
                "SELECT count(d.doc_id)::BIGINT FROM documents d \
                 JOIN sources s ON s.source_id = d.source_id WHERE s.provider = ?1",
                [source],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0);
        let src_row: Option<(Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT last_sync_at, last_error FROM sources WHERE provider = ?1 \
                 ORDER BY last_sync_at DESC LIMIT 1",
                [source],
                |r| Ok((r.get(0).ok(), r.get(1).ok())),
            )
            .ok();
        let (last_sync, last_error) = src_row.unwrap_or((None, None));
        // The per-source config subtree (e.g. slack.lookback_days).
        let cfg_key = if *source == "gdocs" { "google" } else { source };
        list.push(json!({
            "source": source,
            "authProvider": provider,
            "credentialFree": provider.is_none(),
            "connected": connected,
            "scope": workspace::source_scope(source),
            "tracked": tracked,
            "indexedDocuments": indexed,
            "lastSyncAt": last_sync,
            "lastError": last_error,
            "config": cfg.get(cfg_key).cloned().unwrap_or(json!({})),
        }));
    }
    Ok(json!({ "sources": list }))
}

fn sources_track(ctx: &Ctx) -> Result<Value> {
    let b = body_json(ctx)?;
    let source = b["source"].as_str().ok_or_else(|| anyhow!("source required"))?;
    let action = b["action"].as_str().unwrap_or("add");
    let reference = b["ref"].as_str().ok_or_else(|| anyhow!("ref required"))?;
    let list_key = b["listKey"].as_str();
    if action != "add" && action != "remove" {
        return Err(anyhow!("action must be add or remove"));
    }
    let args = vec![source.to_string(), action.to_string(), reference.to_string()];
    let code = connectors::track(&args, list_key)?;
    if code != 0 {
        return Err(anyhow!("track failed (see server log)"));
    }
    Ok(json!({ "ok": true }))
}

fn sources_sync(ctx: &Ctx) -> Result<Value> {
    let b = body_json(ctx)?;
    let source = b["source"].as_str();
    let code = index::sync::run(source, false, None, false)?;
    Ok(json!({ "ok": code == 0, "exitCode": code }))
}

/* ── documents / search ──────────────────────────────────────────────────── */

fn documents(ctx: &Ctx) -> Result<Value> {
    let conn = read_catalog()?;
    let q = query_param(&ctx.query, "q");
    let source = query_param(&ctx.query, "source");
    let tag = query_param(&ctx.query, "tag");
    let limit = query_param(&ctx.query, "limit")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(100)
        .clamp(1, 1000);

    let mut sql = String::from(
        "SELECT d.doc_id, d.title, d.path, d.canonical_ref, d.url, s.provider, \
                d.kind, d.updated_at, d.author_name, t.status AS tag \
           FROM documents d \
           JOIN sources s ON s.source_id = d.source_id \
           LEFT JOIN tags t ON t.target_id = d.doc_id AND t.target_type = 'doc' \
          WHERE 1 = 1",
    );
    let mut params: Vec<String> = Vec::new();
    if let Some(q) = &q {
        sql.push_str(" AND (lower(d.title) LIKE ?  OR lower(d.path) LIKE ? OR lower(d.canonical_ref) LIKE ?)");
        let like = format!("%{}%", q.to_lowercase());
        params.push(like.clone());
        params.push(like.clone());
        params.push(like);
    }
    if let Some(s) = &source {
        sql.push_str(" AND s.provider = ?");
        params.push(s.clone());
    }
    if let Some(t) = &tag {
        sql.push_str(" AND t.status = ?");
        params.push(t.clone());
    }
    sql.push_str(" ORDER BY d.updated_at DESC NULLS LAST LIMIT ?");
    params.push(limit.to_string());
    let param_refs: Vec<&str> = params.iter().map(|s| s.as_str()).collect();
    let rows = rows_json(&conn, &sql, &param_refs)?;
    Ok(json!({ "documents": rows }))
}

fn document(id: &str) -> Result<Value> {
    let conn = read_catalog()?;
    let doc = rows_json(
        &conn,
        "SELECT d.doc_id, d.title, d.path, d.canonical_ref, d.url, s.provider, \
                d.kind, d.mime_type, d.author_name, d.created_at, d.updated_at, \
                d.body, d.metadata_json, t.status AS tag, t.note AS tagNote \
           FROM documents d JOIN sources s ON s.source_id = d.source_id \
           LEFT JOIN tags t ON t.target_id = d.doc_id AND t.target_type = 'doc' \
          WHERE d.doc_id = ? OR d.canonical_ref = ? OR d.path = ? LIMIT 1",
        &[id, id, id],
    )?;
    let Some(doc) = doc.into_iter().next() else {
        return Err(anyhow!("document not found: {id}"));
    };
    let doc_id = doc["doc_id"].as_str().unwrap_or("").to_string();
    let chunks = rows_json(
        &conn,
        "SELECT chunk_id, chunk_index, heading_path, start_line, end_line, token_count \
           FROM chunks WHERE doc_id = ? ORDER BY chunk_index",
        &[&doc_id],
    )?;
    // Lineage edges touching any span of this doc.
    let lineage = rows_json(
        &conn,
        "SELECT le.lineage_id AS id, le.status, le.rel, le.confidence, \
                COALESCE(le.confirmed_by,'') AS by, \
                COALESCE(fd.path, fd.canonical_ref) AS fromRef, \
                COALESCE(td.path, td.canonical_ref) AS toRef \
           FROM lineage_edges le \
           JOIN spans fs ON fs.span_id = le.from_span_id \
           JOIN documents fd ON fd.doc_id = fs.doc_id \
           JOIN spans ts ON ts.span_id = le.to_span_id \
           JOIN documents td ON td.doc_id = ts.doc_id \
          WHERE fs.doc_id = ? OR ts.doc_id = ?",
        &[&doc_id, &doc_id],
    )?;
    Ok(json!({ "document": doc, "chunks": chunks, "lineage": lineage }))
}

fn search(ctx: &Ctx) -> Result<Value> {
    let q = query_param(&ctx.query, "q").unwrap_or_default();
    if q.trim().is_empty() {
        return Ok(json!({ "query": "", "hits": [] }));
    }
    let k = query_param(&ctx.query, "k").and_then(|s| s.parse::<usize>().ok());
    let source = query_param(&ctx.query, "source");
    let tag = query_param(&ctx.query, "tag");
    let args = index::search::SearchArgs {
        query: q,
        full: Some(400),
        variants: Vec::new(),
        k,
        source,
        doc: None,
        author: None,
        since: None,
        before: None,
        tag,
        no_tag: None,
        expand: None,
        json: true,
    };
    index::search::hits_json(&args)
}

/* ── tags ────────────────────────────────────────────────────────────────── */

fn tags() -> Result<Value> {
    let conn = read_catalog()?;
    let rows = rows_json(
        &conn,
        "SELECT t.target_type, t.target_id, t.status, COALESCE(t.note,'') AS note, \
                t.\"by\" AS by, t.\"at\" AS at, \
                COALESCE(d.path, d.canonical_ref, t.target_id) AS ref, d.title \
           FROM tags t \
           LEFT JOIN documents d ON d.doc_id = t.target_id AND t.target_type = 'doc' \
          ORDER BY t.\"at\" DESC",
        &[],
    )?;
    let root = root();
    let cfg = config::resolve(Some(&root));
    let statuses = cfg["tags"]["statuses"].clone();
    Ok(json!({ "tags": rows, "statuses": statuses }))
}

fn tags_apply(ctx: &Ctx) -> Result<Value> {
    let b = body_json(ctx)?;
    let reference = b["ref"].as_str().ok_or_else(|| anyhow!("ref required"))?;
    let status = b["status"].as_str().ok_or_else(|| anyhow!("status required"))?;
    let note = b["note"].as_str();
    let superseded_by = b["supersededBy"].as_str();
    let args = vec![reference.to_string(), status.to_string()];
    let code = curation::tag(&args, note, None, false, None, superseded_by)?;
    if code != 0 {
        return Err(anyhow!("tag failed (see server log)"));
    }
    Ok(json!({ "ok": true }))
}

fn tags_remove(ctx: &Ctx) -> Result<Value> {
    let reference = query_param(&ctx.query, "ref")
        .or_else(|| body_json(ctx).ok().and_then(|b| b["ref"].as_str().map(String::from)))
        .ok_or_else(|| anyhow!("ref required"))?;
    let args = vec!["remove".to_string(), reference];
    let code = curation::tag(&args, None, None, false, None, None)?;
    if code != 0 {
        return Err(anyhow!("no tag on that ref"));
    }
    Ok(json!({ "ok": true }))
}

/* ── lineage ─────────────────────────────────────────────────────────────── */

fn lineage_list() -> Result<Value> {
    let conn = read_catalog()?;
    let rows = rows_json(
        &conn,
        "SELECT le.lineage_id AS id, le.status, le.rel, le.confidence, \
                COALESCE(le.confirmed_by,'') AS by, le.metadata_json AS metadata, \
                COALESCE(fd.path, fd.canonical_ref) AS fromPath, fs.start_line AS fromStart, fs.end_line AS fromEnd, \
                COALESCE(td.path, td.canonical_ref) AS toPath, ts.start_line AS toStart, ts.end_line AS toEnd \
           FROM lineage_edges le \
           JOIN spans fs ON fs.span_id = le.from_span_id \
           JOIN documents fd ON fd.doc_id = fs.doc_id \
           JOIN spans ts ON ts.span_id = le.to_span_id \
           JOIN documents td ON td.doc_id = ts.doc_id \
          ORDER BY le.status, fromPath",
        &[],
    )?;
    Ok(json!({ "edges": rows }))
}

fn lineage_add(ctx: &Ctx) -> Result<Value> {
    let b = body_json(ctx)?;
    let from = b["from"].as_str().ok_or_else(|| anyhow!("from required"))?;
    let to = b["to"].as_str().ok_or_else(|| anyhow!("to required"))?;
    let by = b["by"].as_str();
    let note = b["note"].as_str();
    let args = vec!["add".to_string(), from.to_string(), to.to_string()];
    let code = lineage::run(&args, false, by, note)?;
    if code != 0 {
        return Err(anyhow!("lineage add failed (see server log)"));
    }
    Ok(json!({ "ok": true }))
}

fn lineage_status(id: &str, action: &str) -> Result<Value> {
    let args = vec![action.to_string(), id.to_string()];
    let code = lineage::run(&args, false, None, None)?;
    if code != 0 {
        return Err(anyhow!("no lineage edge matches {id}"));
    }
    Ok(json!({ "ok": true }))
}

/* ── facts / glossary ────────────────────────────────────────────────────── */

fn facts() -> Result<Value> {
    let root = root();
    let cfg = config::resolve(Some(&root));
    let file = cfg["facts"]["file"].as_str().unwrap_or("FACTS.md");
    let path = root.join(file);
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    // Each non-empty, non-heading line is a claim in the ledger.
    let items: Vec<Value> = content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| json!({ "claim": l.trim_start_matches(['-', '*', ' ']) }))
        .collect();
    Ok(json!({ "file": file, "items": items, "raw": content }))
}

fn glossary() -> Result<Value> {
    let root = root();
    let cfg = config::resolve(Some(&root));
    let groups = curation::glossary_groups(&root, &cfg);
    let terms: Vec<Value> = groups
        .into_iter()
        .filter_map(|g| {
            let mut it = g.into_iter();
            let use_ = it.next()?;
            let variants: Vec<String> = it.collect();
            Some(json!({ "use": use_, "variants": variants }))
        })
        .collect();
    let file = cfg["glossary"]["file"].as_str().unwrap_or("STYLE.md");
    Ok(json!({ "file": file, "terms": terms }))
}

/* ── config ──────────────────────────────────────────────────────────────── */

fn config_get() -> Result<Value> {
    let root = root();
    let effective = config::resolve(Some(&root));
    let known = config::known_paths();
    let defaults = config::defaults();
    let paths: Vec<Value> = known
        .into_iter()
        .map(|p| {
            let t = config::get_path(&defaults, &p).map(type_name).unwrap_or("string");
            json!({ "path": p, "type": t })
        })
        .collect();
    Ok(json!({
        "effective": effective,
        "paths": paths,
        "global": config::read_json(&config::global_config_path()),
        "repo": config::read_json(&config::repo_config_path(&root)),
    }))
}

fn config_set(ctx: &Ctx) -> Result<Value> {
    let b = body_json(ctx)?;
    let path = b["path"].as_str().ok_or_else(|| anyhow!("path required"))?;
    let scope = b["scope"].as_str().unwrap_or("repo");
    // Accept either a raw JSON value or a string to coerce against the default.
    let value = match &b["value"] {
        Value::String(s) => config::coerce(path, s)?,
        other if !other.is_null() => other.clone(),
        _ => return Err(anyhow!("value required")),
    };
    match scope {
        "global" => config::set_global(path, value)?,
        _ => {
            let root = root();
            config::set_in_file(&config::repo_config_path(&root), path, value)?;
        }
    }
    Ok(json!({ "ok": true, "rebuildReminder": config::needs_rebuild_reminder(path) }))
}

/* ── cloud (S3 / git sync) ───────────────────────────────────────────────── */

fn cloud_status() -> Result<Value> {
    let root = root();
    let cfg = config::resolve(Some(&root));
    Ok(json!({
        "enabled": cfg["cloud"]["enabled"].as_bool().unwrap_or(false),
        "role": cloud::role(),
        "lastPull": cloud::last_pull().map(|t| t.to_rfc3339()),
        "cloud": cfg.get("cloud").cloned().unwrap_or(json!({})),
        "storage": cfg.get("storage").cloned().unwrap_or(json!({})),
    }))
}

fn cloud_pull() -> Result<Value> {
    let code = cloud::pull()?;
    Ok(json!({ "ok": code == 0, "exitCode": code }))
}

fn cloud_sync(ctx: &Ctx) -> Result<Value> {
    let b = body_json(ctx)?;
    let compact = b["compact"].as_bool().unwrap_or(false);
    let no_push = b["noPush"].as_bool().unwrap_or(false);
    let retain = b["retain"].as_u64().map(|v| v as usize);
    let code = cloud::run(
        &["sync".to_string()],
        None, None, None, None,
        false, compact, no_push, retain,
    )?;
    Ok(json!({ "ok": code == 0, "exitCode": code }))
}

fn cloud_role(ctx: &Ctx) -> Result<Value> {
    let b = body_json(ctx)?;
    let role = b["role"].as_str().ok_or_else(|| anyhow!("role required (writer|consumer)"))?;
    let code = cloud::run(
        &["role".to_string(), role.to_string()],
        None, None, None, None,
        false, false, false, None,
    )?;
    if code != 0 {
        return Err(anyhow!("role must be writer or consumer"));
    }
    Ok(json!({ "ok": true }))
}

/// `connect` (join an existing warehouse) and `init` (create one) share flags.
fn cloud_connect(ctx: &Ctx, action: &str) -> Result<Value> {
    let b = body_json(ctx)?;
    let backend = b["backend"].as_str();
    let bucket = b["bucket"].as_str();
    let prefix = b["prefix"].as_str();
    let region = b["region"].as_str();
    let force = b["force"].as_bool().unwrap_or(false);
    let code = cloud::run(
        &[action.to_string()],
        backend, bucket, prefix, region,
        force, false, false, None,
    )?;
    if code != 0 {
        return Err(anyhow!("cloud {action} failed (see server log)"));
    }
    Ok(json!({ "ok": true }))
}

/* ── projects (switch between indexed workspaces) ────────────────────────── */

fn registry_path() -> PathBuf {
    config::mari_home().join("projects.json")
}

fn read_registry() -> serde_json::Map<String, Value> {
    config::read_json(&registry_path())
        .as_object()
        .cloned()
        .unwrap_or_default()
}

fn write_registry(map: &serde_json::Map<String, Value>) -> Result<()> {
    let path = registry_path();
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).ok();
    }
    std::fs::write(&path, serde_json::to_string_pretty(&Value::Object(map.clone()))?)?;
    Ok(())
}

/// Record a project's absolute path in the registry, keyed by workspace id, so
/// it becomes switchable later. Public so `mari console` can register the
/// launch directory on startup.
pub fn register_current() {
    let _ = register_path(&workspace::work_root());
}

fn register_path(root: &Path) -> Result<String> {
    let abs = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let id = workspace::workspace_id(&abs);
    let mut reg = read_registry();
    reg.insert(
        id.clone(),
        json!({ "path": abs.display().to_string(), "lastOpened": index::now() }),
    );
    write_registry(&reg)?;
    Ok(id)
}

/// Strip the trailing `-<8hex>` disambiguator to recover the project folder name.
fn workspace_slug(id: &str) -> String {
    match id.rsplit_once('-') {
        Some((slug, hex)) if hex.len() == 8 && hex.chars().all(|c| c.is_ascii_hexdigit()) => {
            slug.to_string()
        }
        _ => id.to_string(),
    }
}

fn active_workspace_id() -> String {
    let root = workspace::work_root();
    let abs = std::fs::canonicalize(&root).unwrap_or(root);
    workspace::workspace_id(&abs)
}

fn projects_list() -> Result<Value> {
    let home = config::mari_home();
    let reg = read_registry();
    let active_id = active_workspace_id();
    let mut projects = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&home) {
        for e in entries.flatten() {
            if !e.path().is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('_') || name == "models" || name == "credentials" {
                continue;
            }
            let catalog = e.path().join("catalog.duckdb");
            if !e.path().join("iceberg").exists() && !catalog.exists() {
                continue;
            }
            let (docs, last_sync) = match index::open_readonly_path(&catalog) {
                Ok(Some(conn)) => {
                    let d = conn
                        .query_row("SELECT count(*)::BIGINT FROM documents", [], |r| {
                            r.get::<_, i64>(0)
                        })
                        .unwrap_or(0);
                    let ls: Option<String> = conn
                        .query_row(
                            "SELECT value FROM schema_meta WHERE key='last_sync'",
                            [],
                            |r| r.get(0),
                        )
                        .ok();
                    (d, ls)
                }
                _ => (0, None),
            };
            let path = reg.get(&name).and_then(|v| v["path"].as_str()).map(String::from);
            projects.push(json!({
                "id": name,
                "slug": workspace_slug(&name),
                "documents": docs,
                "lastSync": last_sync,
                "path": path,
                "active": name == active_id,
            }));
        }
    }
    projects.sort_by(|a, b| {
        b["documents"].as_i64().unwrap_or(0).cmp(&a["documents"].as_i64().unwrap_or(0))
    });
    Ok(json!({
        "projects": projects,
        "activeId": active_id,
        "activePath": std::fs::canonicalize(workspace::work_root()).map(|p| p.display().to_string()).unwrap_or_default(),
    }))
}

fn projects_switch(ctx: &Ctx) -> Result<Value> {
    let b = body_json(ctx)?;
    let path = if let Some(p) = b["path"].as_str() {
        PathBuf::from(p)
    } else if let Some(id) = b["workspaceId"].as_str() {
        let reg = read_registry();
        let p = reg
            .get(id)
            .and_then(|v| v["path"].as_str())
            .ok_or_else(|| anyhow!("no known path for this project — open `mari console` in it once, or associate a path"))?;
        PathBuf::from(p)
    } else {
        return Err(anyhow!("path or workspaceId required"));
    };
    if !path.is_dir() {
        return Err(anyhow!("path does not exist: {}", path.display()));
    }
    std::env::set_current_dir(&path)
        .map_err(|e| anyhow!("could not switch to {}: {e}", path.display()))?;
    let id = register_path(&path)?;
    Ok(json!({ "ok": true, "activeId": id, "path": path.display().to_string() }))
}

fn projects_register(ctx: &Ctx) -> Result<Value> {
    let b = body_json(ctx)?;
    let path = b["path"].as_str().ok_or_else(|| anyhow!("path required"))?;
    let p = PathBuf::from(path);
    if !p.is_dir() {
        return Err(anyhow!("path does not exist: {path}"));
    }
    let id = register_path(&p)?;
    Ok(json!({ "ok": true, "id": id }))
}

/* ── nudges ──────────────────────────────────────────────────────────────── */

fn nudges_list() -> Result<Value> {
    let cfg = config::resolve(Some(&root()));
    Ok(json!({ "nudges": cfg.get("nudges").cloned().unwrap_or(json!([])) }))
}

fn nudges_add(ctx: &Ctx) -> Result<Value> {
    let b = body_json(ctx)?;
    let name = b["name"].as_str().ok_or_else(|| anyhow!("name required"))?;
    let when = b["when"].as_str().ok_or_else(|| anyhow!("when (glob[#symbol]) required"))?;
    let edit: Vec<String> = b["edit"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    if edit.is_empty() {
        return Err(anyhow!("at least one edit target required"));
    }
    let message = b["message"].as_str();
    let exclude = b["exclude"].as_str();
    let code = rulescmd::nudge(
        &["add".to_string(), name.to_string()],
        false,
        Some(when),
        &edit,
        message,
        exclude,
    )?;
    if code != 0 {
        return Err(anyhow!("nudge add failed — check that `when` and `edit` targets resolve"));
    }
    Ok(json!({ "ok": true }))
}

fn nudges_remove(ctx: &Ctx) -> Result<Value> {
    let name = query_param(&ctx.query, "name")
        .or_else(|| body_json(ctx).ok().and_then(|b| b["name"].as_str().map(String::from)))
        .ok_or_else(|| anyhow!("name required"))?;
    let code = rulescmd::nudge(&["remove".to_string(), name], false, None, &[], None, None)?;
    if code != 0 {
        return Err(anyhow!("no such nudge"));
    }
    Ok(json!({ "ok": true }))
}

/* ── edit-notify rules ───────────────────────────────────────────────────── */

fn rules_list() -> Result<Value> {
    let cfg = config::resolve(Some(&root()));
    Ok(json!({ "rules": cfg.get("rules").cloned().unwrap_or(json!([])) }))
}

fn rules_add(ctx: &Ctx) -> Result<Value> {
    let b = body_json(ctx)?;
    let name = b["name"].as_str().ok_or_else(|| anyhow!("name required"))?;
    let paths = b["paths"].as_str().ok_or_else(|| anyhow!("paths (comma-separated globs) required"))?;
    let notify = b["notify"].as_str().ok_or_else(|| anyhow!("notify message required"))?;
    let exclude = b["exclude"].as_str();
    let code = rulescmd::rules(
        &["add".to_string(), name.to_string()],
        false,
        false,
        Some(paths),
        Some(notify),
        exclude,
    )?;
    if code != 0 {
        return Err(anyhow!("rule add failed"));
    }
    Ok(json!({ "ok": true }))
}

fn rules_remove(ctx: &Ctx) -> Result<Value> {
    let name = query_param(&ctx.query, "name")
        .or_else(|| body_json(ctx).ok().and_then(|b| b["name"].as_str().map(String::from)))
        .ok_or_else(|| anyhow!("name required"))?;
    let code = rulescmd::rules(&["remove".to_string(), name], false, false, None, None, None)?;
    if code != 0 {
        return Err(anyhow!("no such rule"));
    }
    Ok(json!({ "ok": true }))
}

fn rules_discover() -> Result<Value> {
    // write=true persists discovered edit-notify rules; return the updated list.
    rulescmd::rules(&["discover".to_string()], false, true, None, None, None)?;
    rules_list()
}

/* ── detector governance ─────────────────────────────────────────────────── */

fn detector_get() -> Result<Value> {
    let cfg = config::resolve(Some(&root()));
    let d = &cfg["detector"];
    let catalog: Vec<Value> = detector::registry()
        .iter()
        .map(|r| {
            json!({
                "id": r.id,
                "family": serde_json::to_value(&r.family).unwrap_or_else(|_| json!("")),
                "pack": r.pack,
            })
        })
        .collect();
    Ok(json!({
        "styleGuide": d.get("styleGuide").cloned().unwrap_or(json!("microsoft")),
        "zeroTolerance": d.get("zeroTolerance").cloned().unwrap_or(json!([])),
        "ignoreRules": d.get("ignoreRules").cloned().unwrap_or(json!([])),
        "ignoreFiles": d.get("ignoreFiles").cloned().unwrap_or(json!([])),
        "grammar": d.get("grammar").cloned().unwrap_or(json!(false)),
        "catalog": catalog,
    }))
}

fn detector_zero(ctx: &Ctx) -> Result<Value> {
    let b = body_json(ctx)?;
    let rule = b["rule"].as_str().ok_or_else(|| anyhow!("rule id required"))?;
    let action = b["action"].as_str().unwrap_or("add");
    if action != "add" && action != "remove" {
        return Err(anyhow!("action must be add or remove"));
    }
    let code = rulescmd::zero(&[action.to_string(), rule.to_string()])?;
    if code != 0 {
        return Err(anyhow!("zero {action} failed"));
    }
    Ok(json!({ "ok": true }))
}

fn detector_ignore(ctx: &Ctx) -> Result<Value> {
    let b = body_json(ctx)?;
    let rule = b["rule"].as_str().ok_or_else(|| anyhow!("rule id required"))?;
    let action = b["action"].as_str().unwrap_or("add");
    let reason = b["reason"].as_str();
    match action {
        "add" => {
            let code = rulescmd::ignores(&["add-rule".to_string(), rule.to_string()], reason)?;
            if code != 0 {
                return Err(anyhow!("ignore failed"));
            }
        }
        "remove" => remove_from_repo_array("detector.ignoreRules", rule)?,
        _ => return Err(anyhow!("action must be add or remove")),
    }
    Ok(json!({ "ok": true }))
}

/// Run the deterministic detector on pasted text or a repo file and return the
/// findings + slop score. Honors the repo's detector config (style guide,
/// ignores, zero-tolerance) so the console shows what `mari detect` would.
fn detect(ctx: &Ctx) -> Result<Value> {
    let b = body_json(ctx)?;
    let style = b["style"].as_str();
    let settings = detector::runner::settings(false, style);
    let (path, text) = if let Some(p) = b["path"].as_str().filter(|s| !s.is_empty()) {
        let full = safe_join(&root(), p)?;
        let text = std::fs::read_to_string(&full).map_err(|e| anyhow!("cannot read {p}: {e}"))?;
        (p.to_string(), text)
    } else if let Some(t) = b["text"].as_str() {
        ("input.md".to_string(), t.to_string())
    } else {
        return Err(anyhow!("text or path required"));
    };
    let result = detector::runner::detect_text(&path, &text, &settings);
    let score = detector::score::compute(&text, &result.findings, None);
    // Finding's line/col are `#[serde(skip)]`, so build JSON explicitly to keep them.
    let findings: Vec<Value> = result
        .findings
        .iter()
        .map(|f| {
            json!({
                "ruleId": f.rule_id,
                "family": serde_json::to_value(&f.family).unwrap_or_else(|_| json!("")),
                "severity": serde_json::to_value(&f.severity).unwrap_or_else(|_| json!("")),
                "message": f.message,
                "span": f.span,
                "offset": f.offset,
                "length": f.length,
                "line": f.line,
                "col": f.col,
            })
        })
        .collect();
    Ok(json!({
        "path": path,
        "styleGuide": settings.style_guide,
        "wordCount": result.word_count,
        "score": score,
        "findings": findings,
    }))
}

fn remove_from_repo_array(dotted: &str, item: &str) -> Result<()> {
    let path = config::repo_config_path(&root());
    let mut cfg = config::read_json(&path);
    let arr: Vec<Value> = config::get_path(&cfg, dotted)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|v| v.as_str() != Some(item))
        .collect();
    config::set_path(&mut cfg, dotted, Value::Array(arr));
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).ok();
    }
    std::fs::write(&path, serde_json::to_string_pretty(&cfg)?)?;
    Ok(())
}

/* ── templates (document archetypes) ─────────────────────────────────────── */

fn templates_list() -> Result<Value> {
    Ok(json!({ "templates": assets::archetypes() }))
}

fn templates_scaffold(ctx: &Ctx) -> Result<Value> {
    let b = body_json(ctx)?;
    let typ = b["type"].as_str().ok_or_else(|| anyhow!("type required"))?;
    let title = b["title"].as_str();
    let force = b["force"].as_bool().unwrap_or(false);
    let mut args = vec!["scaffold".to_string(), typ.to_string()];
    if let Some(t) = title {
        if !t.trim().is_empty() {
            args.push(t.to_string());
        }
    }
    let code = assets::run(&args, false, force)?;
    if code != 0 {
        return Err(anyhow!("scaffold failed — the target file may already exist (use force)"));
    }
    Ok(json!({ "ok": true }))
}

/* ── localization explorer ───────────────────────────────────────────────── */

fn localization_coverage(ctx: &Ctx) -> Result<Value> {
    let root = root();
    let source = query_param(&ctx.query, "source").ok_or_else(|| anyhow!("source required"))?;
    let translation =
        query_param(&ctx.query, "translation").ok_or_else(|| anyhow!("translation required"))?;
    let src = safe_join(&root, &source)?;
    let tr = safe_join(&root, &translation)?;
    Ok(i18n::coverage_json(&src, &tr))
}

/// Read a repo-relative text file for the explorer. Path-traversal is rejected:
/// the resolved path must stay inside the workspace root.
fn repo_file(ctx: &Ctx) -> Result<Value> {
    let root = root();
    let rel = query_param(&ctx.query, "path").ok_or_else(|| anyhow!("path required"))?;
    let path = safe_join(&root, &rel)?;
    let content = std::fs::read_to_string(&path).map_err(|e| anyhow!("cannot read {rel}: {e}"))?;
    // Cap very large files so the browser stays responsive.
    let truncated = content.chars().count() > 200_000;
    let body: String = if truncated {
        content.chars().take(200_000).collect()
    } else {
        content
    };
    Ok(json!({ "path": rel, "content": body, "truncated": truncated }))
}

/// Join a repo-relative path onto the workspace root, rejecting anything that
/// escapes the root (via `..`, absolute paths, or symlinks).
fn safe_join(root: &std::path::Path, rel: &str) -> Result<PathBuf> {
    let joined = root.join(rel);
    let canon = std::fs::canonicalize(&joined).map_err(|_| anyhow!("no such path: {rel}"))?;
    let root_canon = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    if !canon.starts_with(&root_canon) {
        return Err(anyhow!("path is outside the workspace"));
    }
    Ok(canon)
}

fn tag_statuses_set(ctx: &Ctx) -> Result<Value> {
    let b = body_json(ctx)?;
    let statuses = b["statuses"].as_array().ok_or_else(|| anyhow!("statuses array required"))?;
    let clean: Vec<Value> = statuses
        .iter()
        .filter_map(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| json!(s.trim()))
        .collect();
    if clean.is_empty() {
        return Err(anyhow!("at least one status required"));
    }
    config::set_in_file(&config::repo_config_path(&root()), "tags.statuses", Value::Array(clean))?;
    Ok(json!({ "ok": true }))
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_f64() => "number",
        Value::Number(_) => "integer",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
        _ => "string",
    }
}
