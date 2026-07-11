//! Keyword retrieval over the DuckDB catalog (SPEC §7 deterministic tier).

use super::{catalog_path, open_catalog_read, read_preflight};
use crate::{config, workspace};
use anyhow::Result;
use duckdb::{params, Connection};
use serde_json::{json, Value};
use std::collections::HashSet;

pub struct SearchArgs {
    pub query: String,
    pub full: Option<usize>,
    pub variants: Vec<String>,
    pub k: Option<usize>,
    pub source: Option<String>,
    pub doc: Option<String>,
    pub author: Option<String>,
    pub since: Option<String>,
    pub before: Option<String>,
    pub tag: Option<String>,
    pub no_tag: Option<String>,
    pub expand: Option<usize>,
    pub json: bool,
}

#[derive(serde::Serialize)]
struct Hit {
    doc_id: String,
    chunk_id: String,
    #[serde(skip)]
    chunk_index: i64,
    source: String,
    canonical_ref: String,
    title: String,
    path: Option<String>,
    url: Option<String>,
    author: Option<String>,
    updated_at: Option<String>,
    heading_path: String,
    start_byte: i64,
    end_byte: i64,
    start_line: i64,
    end_line: i64,
    score: i64,
    score_parts: ScoreParts,
    tag: Option<String>,
    tag_note: Option<String>,
    replacement: Option<String>,
    matched_terms: Vec<String>,
    text: String,
    context: Vec<ContextChunk>,
}

#[derive(Clone, Default, serde::Serialize)]
struct ScoreParts {
    keyword: i64,
    vector: i64,
    tag_multiplier: f64,
    recency_multiplier: f64,
}

#[derive(Clone, serde::Serialize)]
struct ContextChunk {
    chunk_id: String,
    start_line: i64,
    end_line: i64,
    text: String,
}

struct RecentDoc {
    source: String,
    canonical_ref: String,
    title: String,
    path: Option<String>,
    updated_at: Option<String>,
    body: String,
    author: Option<String>,
    observed_at: String,
}

struct DocMatch {
    source: String,
    canonical_ref: String,
    title: String,
    body: String,
    updated_at: Option<String>,
    rank: i64,
}

struct NeighborRow {
    chunk_id: String,
    start_line: i64,
    end_line: i64,
    text: String,
}

struct RelatedRow {
    canonical_ref: String,
    title: String,
    path: Option<String>,
    updated_at: Option<String>,
    rel: String,
    to_type: String,
    to_id: String,
    body: String,
}

#[derive(Clone, Debug, PartialEq)]
struct QueryTerm {
    term: String,
    weight: f64,
}

pub fn run(args: SearchArgs) -> Result<i32> {
    let cfg = config::resolve(Some(&workspace::work_root()));
    if let Some(code) = validate_tag_filters(&cfg, args.tag.as_deref(), args.no_tag.as_deref()) {
        return Ok(code);
    }
    read_preflight(false);
    let query_terms = weighted_terms(&args.query, &args.variants);
    if query_terms.is_empty() {
        eprintln!("no searchable terms");
        return Ok(2);
    }
    let limit = search_limit(&args);
    let catalogs = read_catalogs()?;
    let catalog_refs: Vec<&Connection> = catalogs.iter().collect();
    let hits = ranked_hits_across_catalogs(&args, &query_terms, limit, &catalog_refs)?;
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "query": args.query, "hits": hits }))?
        );
    } else if hits.is_empty() {
        println!("no matches — have you run mari sync?");
    } else {
        for h in &hits {
            println!(
                "{}{}:{}-{}  score={}  {}{}",
                h.canonical_ref,
                h.tag
                    .as_deref()
                    .map(|t| format!(" [{t}]"))
                    .unwrap_or_default(),
                h.start_line,
                h.end_line,
                h.score,
                h.heading_path,
                h.author
                    .as_deref()
                    .map(|a| format!("  author={a}"))
                    .unwrap_or_default()
            );
            print_body_preview(&h.text, args.full);
            if let Some(note) = &h.tag_note {
                println!("  tag note: {note}");
            }
            if let Some(replacement) = &h.replacement {
                println!("  replacement: {replacement}");
            }
            for ctx in &h.context {
                println!("  + {}:{}-{}", ctx.chunk_id, ctx.start_line, ctx.end_line);
                print_context_preview(&ctx.text, args.full);
            }
        }
    }
    Ok(search_exit_code(&hits))
}

/// Top-k distinct documents for a query — (canonical_ref, body) pairs for
/// downstream attention passes (`mari explore --focus`).
pub fn top_docs(query: &str, k: usize) -> Result<Vec<(String, String)>> {
    let args = SearchArgs {
        query: query.to_string(),
        full: None,
        variants: Vec::new(),
        k: Some(k * 3),
        source: None,
        doc: None,
        author: None,
        since: None,
        before: None,
        tag: None,
        no_tag: None,
        expand: None,
        json: false,
    };
    let query_terms = weighted_terms(query, &[]);
    if query_terms.is_empty() {
        return Ok(Vec::new());
    }
    let catalogs = read_catalogs()?;
    let catalog_refs: Vec<&Connection> = catalogs.iter().collect();
    let hits = ranked_hits_across_catalogs(&args, &query_terms, k * 3, &catalog_refs)?;
    let mut out: Vec<(String, String)> = Vec::new();
    let mut seen = HashSet::new();
    for h in hits {
        if !seen.insert(h.doc_id.clone()) {
            continue;
        }
        for conn in &catalog_refs {
            if let Ok(body) = conn.query_row(
                "SELECT body FROM documents WHERE doc_id = ?1",
                [&h.doc_id],
                |r| r.get::<_, String>(0),
            ) {
                out.push((h.canonical_ref.clone(), body));
                break;
            }
        }
        if out.len() >= k {
            break;
        }
    }
    Ok(out)
}

/// Non-printing sibling of `run` for programmatic callers (the local console
/// API): returns `{ query, hits }` as JSON instead of writing to stdout. Tag
/// filters that fail validation and empty queries return an empty hit list.
pub fn hits_json(args: &SearchArgs) -> Result<serde_json::Value> {
    let cfg = config::resolve(Some(&workspace::work_root()));
    if validate_tag_filters(&cfg, args.tag.as_deref(), args.no_tag.as_deref()).is_some() {
        return Ok(json!({ "query": args.query, "hits": [] }));
    }
    let query_terms = weighted_terms(&args.query, &args.variants);
    if query_terms.is_empty() {
        return Ok(json!({ "query": args.query, "hits": [] }));
    }
    let limit = search_limit(args);
    let catalogs = read_catalogs()?;
    let catalog_refs: Vec<&Connection> = catalogs.iter().collect();
    let hits = ranked_hits_across_catalogs(args, &query_terms, limit, &catalog_refs)?;
    Ok(json!({ "query": args.query, "hits": hits }))
}

fn search_limit(args: &SearchArgs) -> usize {
    args.k
        .or_else(|| {
            config::resolve(Some(&workspace::work_root()))["search"]["k"]
                .as_u64()
                .map(|v| v as usize)
        })
        .unwrap_or(8)
        .max(1)
}

fn search_exit_code(hits: &[Hit]) -> i32 {
    if hits.is_empty() {
        1
    } else {
        0
    }
}

fn read_catalogs() -> Result<Vec<Connection>> {
    let mut catalogs = vec![open_catalog_read(false)?];
    if catalog_path(true).exists() {
        read_preflight(true);
        catalogs.push(open_catalog_read(true)?);
    }
    Ok(catalogs)
}

fn ranked_hits_across_catalogs(
    args: &SearchArgs,
    terms: &[QueryTerm],
    limit: usize,
    catalogs: &[&Connection],
) -> Result<Vec<Hit>> {
    let mut all_hits = Vec::new();
    let per_catalog_limit = limit.saturating_mul(5).max(limit);
    let cfg = config::resolve(Some(&workspace::work_root()));
    let hybrid = cfg["search"]["hybrid"].as_bool().unwrap_or(true);
    let rrf_k = cfg["search"]["rrf_k"].as_f64().unwrap_or(60.0);
    let pool = (limit * 4).max(cfg["search"]["keyword_pool"].as_u64().unwrap_or(200) as usize);
    // §7.3 fusion inputs: one vector list per phrasing (main 1.0, variants 0.7),
    // merged by weighted RRF into a single vector ranking per catalog.
    let mut phrasings = vec![args.query.clone()];
    phrasings.extend(args.variants.iter().cloned());
    for (i, conn) in catalogs.iter().enumerate() {
        let is_global = i > 0;
        let vector_ranking = super::vector::rank_many(is_global, &phrasings, pool)
            .map(|lists| merge_vector_lists(&lists, rrf_k));
        let mut hits = search_hits(
            conn,
            &args.query,
            terms,
            &args.variants,
            args.source.as_deref(),
            args.doc.as_deref(),
            args.author.as_deref(),
            args.since.as_deref(),
            args.before.as_deref(),
            per_catalog_limit,
            vector_ranking.as_deref(),
            hybrid,
            rrf_k,
        )?;
        if let Some(tag) = args.tag.as_deref() {
            hits.retain(|h| has_tag(conn, &h.doc_id, tag));
        }
        if let Some(tag) = args.no_tag.as_deref() {
            hits.retain(|h| !has_tag(conn, &h.doc_id, tag));
        }
        apply_tag_boosts(&mut hits);
        apply_recency_decay(&mut hits);
        if merge_sections_enabled() {
            merge_sections(conn, &mut hits)?;
        }
        if should_expand(args.expand) {
            expand_hits(conn, &mut hits, args.expand.unwrap_or(0))?;
        }
        all_hits.extend(hits);
    }
    sort_hits(&mut all_hits);
    dedupe_hits(&mut all_hits);
    all_hits.truncate(limit);
    Ok(all_hits)
}

fn dedupe_hits(hits: &mut Vec<Hit>) {
    // First: exact (source, doc, chunk) dedup across the repo+global scope union.
    let mut seen = HashSet::new();
    hits.retain(|hit| {
        seen.insert(format!(
            "{}\u{1f}{}\u{1f}{}",
            hit.source, hit.doc_id, hit.chunk_id
        ))
    });
    // Second: collapse the same local file+span indexed by multiple overlapping
    // sources (e.g. `git` and `localfiles` both tracking the repo). Hits are
    // already score-sorted, so keeping the first occurrence keeps the best.
    let mut seen_files = HashSet::new();
    hits.retain(|hit| {
        match &hit.path {
            Some(p) if !p.is_empty() => {
                let norm = p.trim_start_matches("./");
                seen_files.insert(format!(
                    "{norm}\u{1f}{}\u{1f}{}",
                    hit.start_line, hit.end_line
                ))
            }
            // No filesystem path (remote sources) — keep as-is.
            _ => true,
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn search_hits(
    conn: &Connection,
    query: &str,
    terms: &[QueryTerm],
    variants: &[String],
    source: Option<&str>,
    doc_filter: Option<&str>,
    author_filter: Option<&str>,
    since: Option<&str>,
    before: Option<&str>,
    limit: usize,
    vector_ranking: Option<&[(String, f64)]>,
    hybrid: bool,
    rrf_k: f64,
) -> Result<Vec<Hit>> {
    let vector_ranks: Option<std::collections::HashMap<&str, usize>> = vector_ranking.map(|v| {
        v.iter()
            .enumerate()
            .map(|(rank, (id, _))| (id.as_str(), rank))
            .collect()
    });
    let mut stmt = conn.prepare(
        "SELECT d.doc_id, c.chunk_id, d.source_id, d.canonical_ref, COALESCE(d.title, ''), d.path, d.url,
                c.heading_path, c.start_byte, c.end_byte, c.start_line, c.end_line, c.text,
                d.author_name, d.author_id, d.updated_at, d.observed_at, c.chunk_index
           FROM chunks c JOIN documents d ON d.doc_id = c.doc_id
          WHERE COALESCE(c.metadata_json, '') NOT LIKE '%\"large\":true%'
          ORDER BY d.updated_at DESC, c.chunk_index ASC"
    )?;
    let mut rows = stmt.query([])?;
    let mut hits = Vec::new();
    while let Some(r) = rows.next()? {
        let doc_id: String = r.get(0)?;
        let source_id: String = r.get(2)?;
        if let Some(s) = source {
            if source_id != s {
                continue;
            }
        }
        let canonical_ref: String = r.get(3)?;
        let title: String = r.get(4)?;
        let path: Option<String> = r.get(5)?;
        let author_name: Option<String> = r.get(13)?;
        let author_id: Option<String> = r.get(14)?;
        let updated_at: Option<String> = r.get(15)?;
        let observed_at: String = r.get(16)?;
        if let Some(df) = doc_filter {
            let needle = df.to_lowercase();
            let doc_id_match = doc_id.to_lowercase().contains(&needle);
            let path_match = path
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .contains(&needle);
            if !doc_id_match
                && !canonical_ref.to_lowercase().contains(&needle)
                && !title.to_lowercase().contains(&needle)
                && !path_match
            {
                continue;
            }
        }
        if let Some(af) = author_filter {
            let needle = af.to_lowercase();
            let author_match = author_name
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .contains(&needle)
                || author_id
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&needle);
            if !author_match {
                continue;
            }
        }
        let updated_for_filter = updated_at.as_deref().unwrap_or(&observed_at);
        if !date_in_range(updated_for_filter, since, before) {
            continue;
        }
        let text: String = r.get(12)?;
        let chunk_id: String = r.get(1)?;
        let lower = text.to_lowercase();
        let matched: Vec<String> = terms
            .iter()
            .filter(|t| lower.contains(t.term.as_str()))
            .map(|t| t.term.clone())
            .collect();
        let in_vector_pool = vector_ranks
            .as_ref()
            .map(|m| m.contains_key(chunk_id.as_str()))
            .unwrap_or(false);
        if matched.is_empty() && !in_vector_pool {
            continue;
        }
        let keyword_score = if matched.is_empty() {
            0
        } else {
            weighted_keyword_score_float(
                keyword_score_weighted(&lower, query, variants, terms),
                query,
            )
        };
        let (tag, tag_note) = doc_tag(conn, &doc_id)?;
        let replacement = if tag.as_deref() == Some("deprecated") {
            replacement_pointer(conn, &doc_id)?
        } else {
            None
        };
        hits.push(Hit {
            doc_id,
            chunk_id,
            chunk_index: r.get(17)?,
            source: source_id,
            canonical_ref,
            title,
            path,
            url: r.get(6)?,
            author: author_name.or(author_id),
            updated_at,
            heading_path: r.get(7)?,
            start_byte: r.get(8)?,
            end_byte: r.get(9)?,
            start_line: r.get(10)?,
            end_line: r.get(11)?,
            score: keyword_score,
            score_parts: ScoreParts {
                keyword: keyword_score,
                vector: 0,
                tag_multiplier: 1.0,
                recency_multiplier: 1.0,
            },
            tag,
            tag_note,
            replacement,
            matched_terms: matched,
            text,
            context: Vec::new(),
        });
    }
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(a.canonical_ref.cmp(&b.canonical_ref))
    });
    if let Some(ranking) = vector_ranking {
        fuse_with_vectors(&mut hits, ranking, query, hybrid, rrf_k);
    }
    hits.truncate(limit);
    Ok(hits)
}

/// Weighted reciprocal-rank fusion (§7.3/§7.4): the keyword ranking (hits
/// are already keyword-sorted) fuses with the vector ranking; `hybrid=false`
/// means vector-only. Auto-routing scales the modality weights by query type.
fn fuse_with_vectors(
    hits: &mut Vec<Hit>,
    ranking: &[(String, f64)],
    query: &str,
    hybrid: bool,
    rrf_k: f64,
) {
    let cfg = config::resolve(Some(&workspace::work_root()));
    let auto = cfg["search"]["auto_weight"].as_bool().unwrap_or(true);
    let mut kw_w = cfg["search"]["keyword_weight"].as_f64().unwrap_or(1.0);
    let mut vec_w = cfg["search"]["vector_weight"].as_f64().unwrap_or(1.0);
    if auto {
        kw_w *= keyword_route_weight(query);
        vec_w *= vector_route_weight(query);
    }
    if !hybrid {
        kw_w = 0.0; // §4.2: search.hybrid=false → vector only
    }
    let vec_rank: std::collections::HashMap<&str, usize> = ranking
        .iter()
        .enumerate()
        .map(|(rank, (id, _))| (id.as_str(), rank))
        .collect();
    for (kw_rank, hit) in hits.iter_mut().enumerate() {
        let kw_term = if hit.matched_terms.is_empty() {
            0.0
        } else {
            kw_w / (rrf_k + kw_rank as f64 + 1.0)
        };
        let vec_term = vec_rank
            .get(hit.chunk_id.as_str())
            .map(|r| vec_w / (rrf_k + *r as f64 + 1.0))
            .unwrap_or(0.0);
        hit.score_parts.keyword = (kw_term * 1_000_000.0).round() as i64;
        hit.score_parts.vector = (vec_term * 1_000_000.0).round() as i64;
        hit.score = hit.score_parts.keyword + hit.score_parts.vector;
    }
    hits.retain(|h| h.score > 0);
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(a.canonical_ref.cmp(&b.canonical_ref))
    });
}

/// Merge per-phrasing vector lists into one ranking via weighted RRF
/// (main query 1.0, each variant 0.7 — §7.3).
fn merge_vector_lists(lists: &[Vec<(String, f64)>], rrf_k: f64) -> Vec<(String, f64)> {
    let mut fused: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for (i, list) in lists.iter().enumerate() {
        let weight = if i == 0 { 1.0 } else { 0.7 };
        for (rank, (id, _)) in list.iter().enumerate() {
            *fused.entry(id.clone()).or_default() += weight / (rrf_k + rank as f64 + 1.0);
        }
    }
    let mut out: Vec<(String, f64)> = fused.into_iter().collect();
    out.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    out
}

/// §7.4 vector-side routing: identifier-like queries lean keyword,
/// natural-language questions lean vector.
fn vector_route_weight(query: &str) -> f64 {
    if is_identifier_like_query(query) {
        0.6
    } else if is_natural_language_question(query) {
        1.3
    } else {
        1.0
    }
}

fn apply_tag_boosts(hits: &mut [Hit]) {
    let cfg = config::resolve(Some(&workspace::work_root()));
    let boosts = &cfg["search"]["tag_boosts"];
    for hit in hits.iter_mut() {
        let Some(tag) = hit.tag.as_deref() else {
            continue;
        };
        let boost = boosts[tag].as_f64().unwrap_or(1.0);
        hit.score_parts.tag_multiplier = boost;
        hit.score = ((hit.score as f64) * boost).round().max(1.0) as i64;
    }
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(a.canonical_ref.cmp(&b.canonical_ref))
    });
}

fn apply_recency_decay(hits: &mut [Hit]) {
    let cfg = config::resolve(Some(&workspace::work_root()));
    let decay = cfg["search"]["recency_decay"].as_f64().unwrap_or(0.0);
    if decay <= 0.0 {
        return;
    }
    let floor = cfg["search"]["recency_floor"].as_f64().unwrap_or(0.75);
    apply_recency_decay_with(hits, decay, floor, chrono::Utc::now());
}

fn apply_recency_decay_with(
    hits: &mut [Hit],
    decay: f64,
    floor: f64,
    now: chrono::DateTime<chrono::Utc>,
) {
    if decay <= 0.0 {
        return;
    }
    for hit in hits.iter_mut() {
        let age = age_years(hit.updated_at.as_deref(), now);
        let multiplier = (1.0 / (1.0 + decay * age)).max(floor);
        hit.score_parts.recency_multiplier = multiplier;
        hit.score = ((hit.score as f64) * multiplier).round().max(1.0) as i64;
    }
    sort_hits(hits);
}

fn age_years(value: Option<&str>, now: chrono::DateTime<chrono::Utc>) -> f64 {
    let Some(value) = value else {
        return 0.25;
    };
    let parsed = chrono::DateTime::parse_from_rfc3339(value)
        .map(|t| t.with_timezone(&chrono::Utc))
        .or_else(|_| {
            chrono::NaiveDate::parse_from_str(&value.replace('/', "-"), "%Y-%m-%d")
                .map(|date| date.and_hms_opt(0, 0, 0).unwrap().and_utc())
        });
    let Ok(parsed) = parsed else {
        return 0.25;
    };
    let seconds = now.signed_duration_since(parsed).num_seconds().max(0) as f64;
    seconds / (365.25 * 24.0 * 60.0 * 60.0)
}

fn sort_hits(hits: &mut [Hit]) {
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(a.canonical_ref.cmp(&b.canonical_ref))
    });
}

fn merge_sections_enabled() -> bool {
    let cfg = config::resolve(Some(&workspace::work_root()));
    cfg["search"]["merge_sections"].as_bool().unwrap_or(true)
}

fn merge_sections(conn: &Connection, hits: &mut Vec<Hit>) -> Result<()> {
    if hits.len() < 2 {
        return Ok(());
    }
    hits.sort_by(|a, b| {
        a.doc_id
            .cmp(&b.doc_id)
            .then(a.chunk_index.cmp(&b.chunk_index))
    });
    let mut merged = Vec::new();
    for hit in hits.drain(..) {
        let can_merge = merged
            .last()
            .map(|prev: &Hit| prev.doc_id == hit.doc_id && prev.chunk_index + 1 == hit.chunk_index)
            .unwrap_or(false);
        if can_merge {
            let prev = merged.last_mut().unwrap();
            merge_hit(conn, prev, hit)?;
        } else {
            merged.push(hit);
        }
    }
    sort_hits(&mut merged);
    *hits = merged;
    Ok(())
}

fn merge_hit(conn: &Connection, base: &mut Hit, next: Hit) -> Result<()> {
    base.end_byte = base.end_byte.max(next.end_byte);
    base.end_line = base.end_line.max(next.end_line);
    base.score = base.score.max(next.score);
    for term in next.matched_terms {
        if !base.matched_terms.contains(&term) {
            base.matched_terms.push(term);
        }
    }
    base.matched_terms.sort();
    base.text = section_text(conn, &base.doc_id, base.start_byte, base.end_byte)?
        .unwrap_or_else(|| format!("{}\n{}", base.text, next.text));
    Ok(())
}

fn section_text(
    conn: &Connection,
    doc_id: &str,
    start_byte: i64,
    end_byte: i64,
) -> Result<Option<String>> {
    let body: Option<String> = conn
        .query_row(
            "SELECT body FROM documents WHERE doc_id = ?1",
            [doc_id],
            |row| row.get(0),
        )
        .ok();
    let Some(body) = body else {
        return Ok(None);
    };
    let start = start_byte.max(0) as usize;
    let end = (end_byte.max(start_byte) as usize).min(body.len());
    if start > body.len() || !body.is_char_boundary(start) || !body.is_char_boundary(end) {
        return Ok(None);
    }
    Ok(Some(body[start..end].to_string()))
}

fn should_expand(expand: Option<usize>) -> bool {
    let Some(expand) = expand else {
        return false;
    };
    if expand == 0 {
        return false;
    }
    let cfg = config::resolve(Some(&workspace::work_root()));
    !cfg["search"]["merge_sections"].as_bool().unwrap_or(true)
}

fn expand_hits(conn: &Connection, hits: &mut [Hit], radius: usize) -> Result<()> {
    for hit in hits {
        let idx: Option<i64> = conn
            .query_row(
                "SELECT chunk_index FROM chunks WHERE chunk_id = ?1",
                [&hit.chunk_id],
                |r| r.get(0),
            )
            .ok();
        let Some(chunk_index) = idx else {
            continue;
        };
        let low = chunk_index.saturating_sub(radius as i64);
        let high = chunk_index + radius as i64;
        let mut stmt = conn.prepare(
            "SELECT chunk_id, start_line, end_line, text FROM chunks
             WHERE doc_id = ?1 AND chunk_index BETWEEN ?2 AND ?3 AND chunk_id <> ?4
               AND COALESCE(metadata_json, '') NOT LIKE '%\"large\":true%'
             ORDER BY chunk_index",
        )?;
        let rows = stmt.query_map(params![hit.doc_id, low, high, hit.chunk_id], |r| {
            Ok(ContextChunk {
                chunk_id: r.get(0)?,
                start_line: r.get(1)?,
                end_line: r.get(2)?,
                text: r.get(3)?,
            })
        })?;
        hit.context = rows.flatten().collect();
    }
    Ok(())
}

#[cfg(test)]
fn terms(query: &str, variants: &[String]) -> Vec<String> {
    weighted_terms(query, variants)
        .into_iter()
        .map(|term| term.term)
        .collect()
}

fn weighted_terms(query: &str, variants: &[String]) -> Vec<QueryTerm> {
    let mut out: Vec<QueryTerm> = query_terms(query)
        .into_iter()
        .map(|term| QueryTerm { term, weight: 1.0 })
        .collect();
    for variant in variants {
        for term in query_terms(variant) {
            if let Some(existing) = out.iter_mut().find(|t| t.term == term) {
                existing.weight = existing.weight.max(0.7);
            } else {
                out.push(QueryTerm { term, weight: 0.7 });
            }
        }
    }
    out.sort_by(|a, b| a.term.cmp(&b.term));
    out
}

fn query_terms(query: &str) -> Vec<String> {
    let mut out: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric() && c != '-')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| {
            s.len() >= 2
                && ![
                    "the", "and", "for", "with", "that", "this", "what", "how", "why", "are", "you",
                ]
                .contains(&s.as_str())
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

fn keyword_score_weighted(
    lower_text: &str,
    query: &str,
    variants: &[String],
    terms: &[QueryTerm],
) -> f64 {
    let term_score = terms
        .iter()
        .filter(|term| lower_text.contains(term.term.as_str()))
        .map(|term| term.weight)
        .sum::<f64>();
    term_score + phrase_score(lower_text, query, 1.0) + variant_phrase_score(lower_text, variants)
}

#[cfg(test)]
fn keyword_score(lower_text: &str, query: &str, matched_terms: &[String]) -> i64 {
    let phrase_bonus = phrase_score(lower_text, query, 1.0).round() as i64;
    matched_terms.len() as i64 + phrase_bonus
}

fn phrase_score(lower_text: &str, query: &str, weight: f64) -> f64 {
    let phrase = query.split_whitespace().collect::<Vec<_>>().join(" ");
    if phrase.chars().count() >= 2 && lower_text.contains(&phrase.to_lowercase()) {
        2.0 * weight
    } else {
        0.0
    }
}

fn variant_phrase_score(lower_text: &str, variants: &[String]) -> f64 {
    variants
        .iter()
        .map(|variant| phrase_score(lower_text, variant, 0.7))
        .sum()
}

fn weighted_keyword_score_float(raw_score: f64, query: &str) -> i64 {
    let cfg = config::resolve(Some(&workspace::work_root()));
    let base_weight = cfg["search"]["keyword_weight"].as_f64().unwrap_or(1.0);
    let auto_weight = cfg["search"]["auto_weight"].as_bool().unwrap_or(true);
    weighted_keyword_score_float_with(raw_score, query, base_weight, auto_weight)
}

#[cfg(test)]
fn weighted_keyword_score_with(
    raw_score: i64,
    query: &str,
    keyword_weight: f64,
    auto_weight: bool,
) -> i64 {
    weighted_keyword_score_float_with(raw_score as f64, query, keyword_weight, auto_weight)
}

fn weighted_keyword_score_float_with(
    raw_score: f64,
    query: &str,
    keyword_weight: f64,
    auto_weight: bool,
) -> i64 {
    let route_weight = if auto_weight {
        keyword_route_weight(query)
    } else {
        1.0
    };
    (raw_score * keyword_weight * route_weight).round().max(1.0) as i64
}

fn keyword_route_weight(query: &str) -> f64 {
    if is_identifier_like_query(query) {
        1.6
    } else if is_natural_language_question(query) {
        0.8
    } else {
        1.0
    }
}

fn is_identifier_like_query(query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.contains('"') || trimmed.contains('\'') {
        return true;
    }
    let tokens = query_tokens(trimmed);
    if tokens.is_empty() {
        return false;
    }
    if tokens.len() == 1 && tokens[0].chars().all(|c| c.is_ascii_digit()) && tokens[0].len() <= 6 {
        return true;
    }
    tokens.len() <= 2
        && tokens.iter().all(|t| {
            t.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '#' | '/' | '.' | '-'))
        })
}

fn is_natural_language_question(query: &str) -> bool {
    let tokens = query_tokens(query);
    query.trim_end().ends_with('?')
        || (tokens.len() >= 5
            && tokens.iter().any(|t| {
                matches!(
                    t.as_str(),
                    "who" | "what" | "when" | "where" | "why" | "how" | "which"
                )
            }))
}

fn query_tokens(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_ascii_alphanumeric() && !matches!(c, '_' | '#' | '/' | '.' | '-'))
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

fn has_tag(conn: &Connection, doc_id: &str, tag: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM tags WHERE target_type = 'doc' AND target_id = ?1 AND status = ?2 LIMIT 1",
        params![doc_id, tag],
        |_| Ok(()),
    )
    .is_ok()
}

fn doc_tag(conn: &Connection, doc_id: &str) -> Result<(Option<String>, Option<String>)> {
    let tag: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT status, note FROM tags WHERE target_type = 'doc' AND target_id = ?1 LIMIT 1",
            [doc_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
        )
        .ok();
    Ok(tag
        .map(|(status, note)| {
            (
                Some(status),
                note.and_then(|n| if n.is_empty() { None } else { Some(n) }),
            )
        })
        .unwrap_or((None, None)))
}

fn replacement_pointer(conn: &Connection, doc_id: &str) -> Result<Option<String>> {
    if let Some(pointer) = replacement_from_lineage(conn, doc_id)? {
        return Ok(Some(pointer));
    }
    replacement_from_edges(conn, doc_id)
}

fn replacement_from_lineage(conn: &Connection, doc_id: &str) -> Result<Option<String>> {
    let pointer: Option<(String, Option<String>, i64, i64, String)> = conn
        .query_row(
            "SELECT COALESCE(other_doc.path, other_doc.canonical_ref), other_doc.title, other_span.start_line, other_span.end_line, le.rel
               FROM lineage_edges le
               JOIN spans deprecated_span
                 ON deprecated_span.span_id = le.from_span_id OR deprecated_span.span_id = le.to_span_id
               JOIN spans other_span
                 ON (other_span.span_id = le.from_span_id OR other_span.span_id = le.to_span_id)
                AND other_span.span_id <> deprecated_span.span_id
               JOIN documents other_doc ON other_doc.doc_id = other_span.doc_id
              WHERE deprecated_span.doc_id = ?1
                AND le.status = 'confirmed'
              ORDER BY
                CASE le.rel WHEN 'replaces' THEN 0 WHEN 'supersedes' THEN 1 WHEN 'updates' THEN 2 WHEN 'documents' THEN 3 ELSE 4 END,
                other_doc.updated_at DESC
              LIMIT 1",
            [doc_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .ok();
    Ok(pointer.map(|(path, title, start, end, rel)| {
        let title = title.filter(|t| !t.is_empty()).unwrap_or(path.clone());
        format!("{path}:{start}-{end}  {title}  via {rel}")
    }))
}

fn replacement_from_edges(conn: &Connection, doc_id: &str) -> Result<Option<String>> {
    let pointer: Option<(String, Option<String>, String)> = conn
        .query_row(
            "SELECT COALESCE(d.path, d.canonical_ref), d.title, e.rel
               FROM edges e
               JOIN documents d ON d.doc_id = e.to_id
              WHERE e.from_type = 'doc'
                AND e.from_id = ?1
                AND e.to_type = 'doc'
                AND e.rel IN ('supersedes', 'updates', 'replaces')
              ORDER BY CASE e.rel WHEN 'supersedes' THEN 0 WHEN 'updates' THEN 1 ELSE 2 END
              LIMIT 1",
            [doc_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .ok();
    Ok(pointer.map(|(path, title, rel)| {
        let title = title.filter(|t| !t.is_empty()).unwrap_or(path.clone());
        format!("{path}  {title}  via {rel}")
    }))
}

fn snippet(s: &str, max: usize) -> String {
    if max == 0 {
        return s.to_string();
    }
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        flat
    } else {
        let mut out: String = flat.chars().take(max).collect();
        out.push_str("...");
        out
    }
}

fn preview_lines(s: &str, max_lines: usize, max_chars_per_line: usize) -> Vec<String> {
    let mut out = Vec::new();
    for raw in s.lines() {
        let line = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() {
            continue;
        }
        let mut chars = line.chars();
        let rendered: String = chars.by_ref().take(max_chars_per_line).collect();
        if chars.next().is_some() {
            out.push(format!("{rendered}..."));
        } else {
            out.push(rendered);
        }
        if out.len() >= max_lines {
            break;
        }
    }
    if out.is_empty() && !s.trim().is_empty() {
        out.push(snippet(s, max_chars_per_line));
    }
    out
}

fn print_body_preview(text: &str, full: Option<usize>) {
    if let Some(n) = full {
        println!("  {}", snippet(text, n));
        return;
    }
    for line in preview_lines(text, 5, 110) {
        println!("  {line}");
    }
}

fn print_context_preview(text: &str, full: Option<usize>) {
    if let Some(n) = full {
        println!("    {}", snippet(text, n));
        return;
    }
    for line in preview_lines(text, 5, 110) {
        println!("    {line}");
    }
}

pub fn recent(
    source: Option<&str>,
    doc: Option<&str>,
    author: Option<&str>,
    since: Option<&str>,
    before: Option<&str>,
    tag: Option<&str>,
    no_tag: Option<&str>,
    limit: usize,
    full: Option<usize>,
    json_out: bool,
) -> Result<i32> {
    let cfg = config::resolve(Some(&workspace::work_root()));
    if let Some(code) = validate_tag_filters(&cfg, tag, no_tag) {
        return Ok(code);
    }
    read_preflight(false);
    let catalogs = read_catalogs()?;
    let catalog_refs: Vec<&Connection> = catalogs.iter().collect();
    let rows = collect_recent_docs(
        &catalog_refs,
        source,
        doc,
        author,
        since,
        before,
        tag,
        no_tag,
        limit,
    )?;
    let result_count = rows.len();
    let mut arr = Vec::new();
    for row in rows {
        if json_out {
            arr.push(json!({"source": row.source, "ref": row.canonical_ref, "title": row.title, "path": row.path, "updated_at": row.updated_at, "author": row.author}));
        } else {
            println!(
                "{}  {}  {}{}",
                row.updated_at.unwrap_or_default(),
                row.canonical_ref,
                row.title,
                row.author
                    .as_deref()
                    .map(|a| format!("  author={a}"))
                    .unwrap_or_default()
            );
            if let Some(n) = full {
                println!("  {}", snippet(&row.body, n));
            }
        }
    }
    if !json_out && result_count == 0 {
        println!("no matches — have you run mari sync?");
    }
    if json_out {
        println!("{}", serde_json::to_string_pretty(&arr)?);
    }
    Ok(results_exit_code(result_count))
}

fn validate_tag_filters(cfg: &Value, tag: Option<&str>, no_tag: Option<&str>) -> Option<i32> {
    for (flag, value) in [("--tag", tag), ("--no-tag", no_tag)] {
        let Some(status) = value else {
            continue;
        };
        if !valid_tag_status(cfg, status) {
            eprintln!(
                "✗ unknown {flag} status '{status}' — valid statuses: {}",
                tag_statuses(cfg).join(", ")
            );
            return Some(2);
        }
    }
    None
}

fn valid_tag_status(cfg: &Value, status: &str) -> bool {
    tag_statuses(cfg).iter().any(|s| s == status)
}

fn tag_statuses(cfg: &Value) -> Vec<String> {
    cfg["tags"]["statuses"]
        .as_array()
        .map(|statuses| {
            statuses
                .iter()
                .filter_map(|status| status.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn collect_recent_docs(
    catalogs: &[&Connection],
    source: Option<&str>,
    doc: Option<&str>,
    author: Option<&str>,
    since: Option<&str>,
    before: Option<&str>,
    tag: Option<&str>,
    no_tag: Option<&str>,
    limit: usize,
) -> Result<Vec<RecentDoc>> {
    let mut out = Vec::new();
    for conn in catalogs {
        out.extend(recent_docs_for_catalog(
            conn, source, doc, author, since, before, tag, no_tag,
        )?);
    }
    out.sort_by(|a, b| {
        b.updated_at
            .as_deref()
            .unwrap_or(&b.observed_at)
            .cmp(a.updated_at.as_deref().unwrap_or(&a.observed_at))
            .then(a.canonical_ref.cmp(&b.canonical_ref))
    });
    dedupe_recent_docs(&mut out);
    out.truncate(limit);
    Ok(out)
}

fn recent_docs_for_catalog(
    conn: &Connection,
    source: Option<&str>,
    doc: Option<&str>,
    author: Option<&str>,
    since: Option<&str>,
    before: Option<&str>,
    tag: Option<&str>,
    no_tag: Option<&str>,
) -> Result<Vec<RecentDoc>> {
    let mut stmt = conn.prepare(
        "SELECT doc_id, source_id, canonical_ref, COALESCE(title, ''), path, updated_at, body, author_name, author_id, observed_at
           FROM documents ORDER BY updated_at DESC"
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, Option<String>>(5)?,
            r.get::<_, String>(6)?,
            r.get::<_, Option<String>>(7)?,
            r.get::<_, Option<String>>(8)?,
            r.get::<_, String>(9)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows.flatten() {
        if source.map(|s| row.1 != s).unwrap_or(false) {
            continue;
        }
        if let Some(d) = doc {
            let needle = d.to_lowercase();
            if !row.0.to_lowercase().contains(&needle)
                && !row.2.to_lowercase().contains(&needle)
                && !row.3.to_lowercase().contains(&needle)
                && !row
                    .4
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&needle)
            {
                continue;
            }
        }
        if let Some(af) = author {
            let needle = af.to_lowercase();
            let matched = row
                .7
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .contains(&needle)
                || row
                    .8
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&needle);
            if !matched {
                continue;
            }
        }
        let updated_for_filter = row.5.as_deref().unwrap_or(&row.9);
        if !date_in_range(updated_for_filter, since, before) {
            continue;
        }
        if let Some(tag) = tag {
            if !has_tag(conn, &row.0, tag) {
                continue;
            }
        }
        if let Some(tag) = no_tag {
            if has_tag(conn, &row.0, tag) {
                continue;
            }
        }
        out.push(RecentDoc {
            source: row.1,
            canonical_ref: row.2,
            title: row.3,
            path: row.4,
            updated_at: row.5,
            body: row.6,
            author: row.7.or(row.8),
            observed_at: row.9,
        });
    }
    Ok(out)
}

fn dedupe_recent_docs(rows: &mut Vec<RecentDoc>) {
    let mut seen = HashSet::new();
    rows.retain(|row| seen.insert(format!("{}\u{1f}{}", row.source, row.canonical_ref)));
}

fn date_in_range(value: &str, since: Option<&str>, before: Option<&str>) -> bool {
    let Some(value) = normalize_date(value) else {
        return true;
    };
    if let Some(since) = since.and_then(normalize_date) {
        if value < since {
            return false;
        }
    }
    if let Some(before) = before.and_then(normalize_date) {
        if value >= before {
            return false;
        }
    }
    true
}

fn normalize_date(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.len() >= 10 {
        let date = &trimmed[..10].replace('/', "-");
        if chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok() {
            return Some(date.to_string());
        }
    }
    None
}

pub fn doc(r: &str, source: Option<&str>, full: Option<usize>) -> Result<i32> {
    if r.trim().is_empty() {
        eprintln!("usage: mari doc <ref> — a document id or title substring");
        return Ok(2);
    }
    read_preflight(false);
    let catalogs = read_catalogs()?;
    let catalog_refs: Vec<&Connection> = catalogs.iter().collect();
    let rows = find_docs_across_catalogs(&catalog_refs, r, source, 5)?;
    let mut emitted = 0usize;
    for row in rows {
        if emitted > 0 {
            println!("\n---");
        }
        println!("{}  {}", row.canonical_ref, row.title);
        match full {
            Some(0) | None => println!("{}", row.body),
            Some(n) => println!("{}", snippet(&row.body, n)),
        }
        emitted += 1;
    }
    if emitted == 0 {
        println!("no matches — have you run mari sync?");
    }
    Ok(results_exit_code(emitted))
}

fn find_docs_across_catalogs(
    catalogs: &[&Connection],
    r: &str,
    source: Option<&str>,
    limit: usize,
) -> Result<Vec<DocMatch>> {
    let mut out = Vec::new();
    for conn in catalogs {
        out.extend(find_docs_for_catalog(
            conn,
            r,
            source,
            limit.saturating_mul(4).max(limit),
        )?);
    }
    out.sort_by(|a, b| {
        a.rank
            .cmp(&b.rank)
            .then(b.updated_at.cmp(&a.updated_at))
            .then(a.canonical_ref.cmp(&b.canonical_ref))
    });
    dedupe_doc_matches(&mut out);
    out.truncate(limit);
    Ok(out)
}

fn find_docs_for_catalog(
    conn: &Connection,
    r: &str,
    source: Option<&str>,
    limit: usize,
) -> Result<Vec<DocMatch>> {
    let needle = format!("%{r}%");
    let mut stmt = conn.prepare(
        "SELECT source_id, canonical_ref, COALESCE(title, ''), body, updated_at,
                CASE WHEN doc_id = ?2 THEN 0 WHEN canonical_ref = ?2 THEN 1 WHEN path = ?2 THEN 2 WHEN title = ?2 THEN 3 ELSE 4 END
           FROM documents
          WHERE doc_id LIKE ?1 OR canonical_ref LIKE ?1 OR title LIKE ?1 OR path LIKE ?1
          ORDER BY CASE WHEN doc_id = ?2 THEN 0 WHEN canonical_ref = ?2 THEN 1 WHEN path = ?2 THEN 2 WHEN title = ?2 THEN 3 ELSE 4 END, updated_at DESC
          LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![needle, r, limit as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, i64>(5)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows.flatten() {
        if source.map(|s| row.0 != s).unwrap_or(false) {
            continue;
        }
        out.push(DocMatch {
            source: row.0,
            canonical_ref: row.1,
            title: row.2,
            body: row.3,
            updated_at: row.4,
            rank: row.5,
        });
    }
    Ok(out)
}

fn dedupe_doc_matches(rows: &mut Vec<DocMatch>) {
    let mut seen = HashSet::new();
    rows.retain(|row| seen.insert(format!("{}\u{1f}{}", row.source, row.canonical_ref)));
}

pub fn neighbors(chunk: &str, radius: usize, full: Option<usize>) -> Result<i32> {
    read_preflight(false);
    let catalogs = read_catalogs()?;
    let catalog_refs: Vec<&Connection> = catalogs.iter().collect();
    let Some(rows) = neighbor_rows_across_catalogs(&catalog_refs, chunk, radius)? else {
        println!("no chunk found in repo or _global catalog");
        return Ok(1);
    };
    for row in rows {
        println!("{}:{}-{}", row.chunk_id, row.start_line, row.end_line);
        println!("  {}", snippet(&row.text, full.unwrap_or(400)));
    }
    Ok(0)
}

fn neighbor_rows_across_catalogs(
    catalogs: &[&Connection],
    chunk: &str,
    radius: usize,
) -> Result<Option<Vec<NeighborRow>>> {
    for conn in catalogs {
        if let Some(rows) = neighbor_rows_for_catalog(conn, chunk, radius)? {
            return Ok(Some(rows));
        }
    }
    Ok(None)
}

fn neighbor_rows_for_catalog(
    conn: &Connection,
    chunk: &str,
    radius: usize,
) -> Result<Option<Vec<NeighborRow>>> {
    let idx: Option<(String, i64)> = conn
        .query_row(
            "SELECT doc_id, chunk_index FROM chunks
              WHERE chunk_id = ?1
                AND COALESCE(metadata_json, '') NOT LIKE '%\"large\":true%'",
            [chunk],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let Some((doc_id, chunk_index)) = idx else {
        return Ok(None);
    };
    let low = chunk_index.saturating_sub(radius as i64);
    let high = chunk_index + radius as i64;
    let mut stmt = conn.prepare(
        "SELECT chunk_id, start_line, end_line, text FROM chunks
          WHERE doc_id = ?1
            AND chunk_index BETWEEN ?2 AND ?3
            AND COALESCE(metadata_json, '') NOT LIKE '%\"large\":true%'
          ORDER BY chunk_index",
    )?;
    let rows = stmt.query_map(params![doc_id, low, high], |r| {
        Ok(NeighborRow {
            chunk_id: r.get(0)?,
            start_line: r.get(1)?,
            end_line: r.get(2)?,
            text: r.get(3)?,
        })
    })?;
    Ok(Some(rows.flatten().collect()))
}

pub fn related(r: &str, source: Option<&str>, limit: usize, full: Option<usize>) -> Result<i32> {
    if r.trim().is_empty() {
        eprintln!("usage: mari related <ref> — a document id or title substring");
        return Ok(2);
    }
    read_preflight(false);
    let catalogs = read_catalogs()?;
    let catalog_refs: Vec<&Connection> = catalogs.iter().collect();
    let Some((canonical_ref, title, rows)) =
        related_rows_across_catalogs(&catalog_refs, r, source, limit)?
    else {
        println!("no matches — have you run mari sync?");
        return Ok(1);
    };
    let mut count = 0usize;
    println!("related to {canonical_ref}  {title}");
    for row in rows {
        count += 1;
        println!(
            "{}  {}  reason={}",
            row.canonical_ref,
            row.title,
            related_reason(&row.rel, &row.to_type, &row.to_id)
        );
        if let Some(path) = row.path {
            println!("  path={path}");
        }
        if let Some(updated) = row.updated_at {
            println!("  updated={updated}");
        }
        if let Some(n) = full {
            println!("  {}", snippet(&row.body, n));
        }
    }
    if count == 0 {
        println!("no related docs in edge graph");
    }
    Ok(results_exit_code(count))
}

fn results_exit_code(count: usize) -> i32 {
    if count == 0 {
        1
    } else {
        0
    }
}

fn related_rows_across_catalogs(
    catalogs: &[&Connection],
    r: &str,
    source: Option<&str>,
    limit: usize,
) -> Result<Option<(String, String, Vec<RelatedRow>)>> {
    for conn in catalogs {
        let Some((doc_id, canonical_ref, title)) = find_doc(conn, r, source)? else {
            continue;
        };
        let rows = related_rows_for_catalog(conn, &doc_id, limit)?;
        return Ok(Some((canonical_ref, title, rows)));
    }
    Ok(None)
}

fn related_rows_for_catalog(
    conn: &Connection,
    doc_id: &str,
    limit: usize,
) -> Result<Vec<RelatedRow>> {
    let mut out = direct_related_rows_for_catalog(conn, doc_id, limit)?;
    if out.len() < limit {
        out.extend(shared_related_rows_for_catalog(
            conn,
            doc_id,
            limit.saturating_sub(out.len()),
        )?);
    }
    dedupe_related_rows(&mut out);
    out.truncate(limit);
    Ok(out)
}

fn direct_related_rows_for_catalog(
    conn: &Connection,
    doc_id: &str,
    limit: usize,
) -> Result<Vec<RelatedRow>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT d.canonical_ref, COALESCE(d.title, ''), d.path, d.updated_at, e.rel, e.to_type, e.to_id, d.body
           FROM edges e
           JOIN documents d ON d.doc_id = e.to_id
          WHERE e.from_type = 'doc'
            AND e.from_id = ?1
            AND e.to_type = 'doc'
            AND e.to_id <> ?1
        UNION ALL
         SELECT DISTINCT d.canonical_ref, COALESCE(d.title, ''), d.path, d.updated_at, e.rel, e.from_type, e.from_id, d.body
           FROM edges e
           JOIN documents d ON d.doc_id = e.from_id
          WHERE e.to_type = 'doc'
            AND e.to_id = ?1
            AND e.from_type = 'doc'
            AND e.from_id <> ?1
          ORDER BY rel, updated_at DESC
          LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![doc_id, limit as i64], |row| {
        Ok(RelatedRow {
            canonical_ref: row.get(0)?,
            title: row.get(1)?,
            path: row.get(2)?,
            updated_at: row.get(3)?,
            rel: row.get(4)?,
            to_type: row.get(5)?,
            to_id: row.get(6)?,
            body: row.get(7)?,
        })
    })?;
    Ok(rows.flatten().collect())
}

fn shared_related_rows_for_catalog(
    conn: &Connection,
    doc_id: &str,
    limit: usize,
) -> Result<Vec<RelatedRow>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT d.canonical_ref, COALESCE(d.title, ''), d.path, d.updated_at, e.rel, e.to_type, e.to_id, d.body
           FROM edges seed
           JOIN edges e ON e.rel = seed.rel AND e.to_type = seed.to_type AND e.to_id = seed.to_id
           JOIN documents d ON d.doc_id = e.from_id
          WHERE seed.from_type = 'doc'
            AND seed.from_id = ?1
            AND e.from_type = 'doc'
            AND e.from_id <> ?1
          ORDER BY e.rel, d.updated_at DESC
          LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![doc_id, limit as i64], |row| {
        Ok(RelatedRow {
            canonical_ref: row.get(0)?,
            title: row.get(1)?,
            path: row.get(2)?,
            updated_at: row.get(3)?,
            rel: row.get(4)?,
            to_type: row.get(5)?,
            to_id: row.get(6)?,
            body: row.get(7)?,
        })
    })?;
    Ok(rows.flatten().collect())
}

fn dedupe_related_rows(rows: &mut Vec<RelatedRow>) {
    let mut seen = HashSet::new();
    rows.retain(|row| {
        seen.insert(format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}",
            row.canonical_ref, row.rel, row.to_type, row.to_id
        ))
    });
}

fn find_doc(
    conn: &Connection,
    r: &str,
    source: Option<&str>,
) -> Result<Option<(String, String, String)>> {
    let needle = format!("%{r}%");
    let mut stmt = conn.prepare(
        "SELECT doc_id, canonical_ref, COALESCE(title, ''), source_id
           FROM documents
          WHERE canonical_ref LIKE ?1 OR title LIKE ?1 OR path LIKE ?1
          ORDER BY updated_at DESC
          LIMIT 10",
    )?;
    let rows = stmt.query_map([needle], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    for row in rows.flatten() {
        if source.map(|s| row.3 == s).unwrap_or(true) {
            return Ok(Some((row.0, row.1, row.2)));
        }
    }
    Ok(None)
}

fn related_reason(rel: &str, to_type: &str, to_id: &str) -> String {
    match (rel, to_type) {
        ("authored_by", "person") => format!("shared author {to_id}"),
        ("in_repo", "container") => format!("shared container {to_id}"),
        ("in_project", "container") => format!("shared project {to_id}"),
        ("in_channel", "container") => format!("shared channel {to_id}"),
        ("links_to", _) => format!("shared link {to_id}"),
        _ => format!("{rel} {to_type}:{to_id}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        age_years, apply_recency_decay_with, apply_tag_boosts, collect_recent_docs, date_in_range,
        dedupe_hits, find_docs_across_catalogs, keyword_score, keyword_score_weighted,
        neighbor_rows_across_catalogs, normalize_date, ranked_hits_across_catalogs, related_reason,
        related_rows_across_catalogs, replacement_pointer, snippet, terms, validate_tag_filters,
        weighted_keyword_score_with, weighted_terms, Hit, ScoreParts, SearchArgs,
    };
    use chrono::TimeZone;
    use duckdb::{params, Connection};
    use serde_json::json;

    #[test]
    fn date_filters_accept_spec_forms() {
        assert_eq!(normalize_date("2026/07/06"), Some("2026-07-06".into()));
        assert_eq!(
            normalize_date("2026-07-06T12:30:00Z"),
            Some("2026-07-06".into())
        );
        assert!(date_in_range(
            "2026-07-06T12:30:00Z",
            Some("2026-07-01"),
            Some("2026-08-01")
        ));
        assert!(!date_in_range(
            "2026-07-06",
            Some("2026-07-07"),
            Some("2026-08-01")
        ));
        assert!(!date_in_range(
            "2026-07-06",
            Some("2026-07-01"),
            Some("2026-07-06")
        ));
    }

    #[test]
    fn terms_dedup_variants() {
        assert_eq!(
            terms("What DuckDB DuckDB?", &["duckdb".into()]),
            vec!["duckdb"]
        );
        assert_eq!(
            terms("pricing", &["annual refund policy".into()]),
            vec!["annual", "policy", "pricing", "refund"]
        );
    }

    #[test]
    fn tag_filters_must_use_configured_statuses() {
        let cfg = json!({
            "tags": {
                "statuses": ["canonical", "stale", "needs-review"]
            }
        });

        assert_eq!(
            validate_tag_filters(&cfg, Some("canonical"), Some("stale")),
            None
        );
        assert_eq!(
            validate_tag_filters(&cfg, Some("totally-bogus"), None),
            Some(2)
        );
        assert_eq!(
            validate_tag_filters(&cfg, None, Some("totally-bogus")),
            Some(2)
        );
    }

    #[test]
    fn keyword_score_counts_distinct_terms_and_phrase_bonus() {
        assert_eq!(
            keyword_score(
                "duckdb duckdb catalog",
                "duckdb catalog",
                &["duckdb".into(), "catalog".into()],
            ),
            4
        );
        assert_eq!(
            keyword_score(
                "duckdb repeated duckdb",
                "duckdb catalog",
                &["duckdb".into()],
            ),
            1
        );
    }

    #[test]
    fn variant_terms_score_below_main_query_terms() {
        let main_terms = weighted_terms("annual refund", &[]);
        let variant_terms = weighted_terms("pricing", &["annual refund".into()]);
        let main_score = keyword_score_weighted("annual refund", "annual refund", &[], &main_terms);
        let variant_score = keyword_score_weighted(
            "annual refund",
            "pricing",
            &["annual refund".into()],
            &variant_terms,
        );

        assert_eq!(main_score, 4.0);
        assert!(variant_score > 2.7);
        assert!(variant_score < main_score);
    }

    #[test]
    fn keyword_weight_honors_auto_query_routing() {
        assert_eq!(weighted_keyword_score_with(10, "api/v1", 1.0, true), 16);
        assert_eq!(
            weighted_keyword_score_with(10, "how do refunds work for annual plans?", 1.0, true),
            8
        );
        assert_eq!(
            weighted_keyword_score_with(10, "how do refunds work for annual plans?", 2.0, false),
            20
        );
    }

    #[test]
    fn search_limit_uses_cli_override_or_config_default() {
        assert_eq!(super::search_limit(&search_args("duckdb")), 8);
        let mut args = search_args("duckdb");
        args.k = Some(0);
        assert_eq!(super::search_limit(&args), 1);
        args.k = Some(3);
        assert_eq!(super::search_limit(&args), 3);
    }

    #[test]
    fn empty_search_results_exit_nonzero() {
        let hit = test_hit("hit", 1, None);
        assert_eq!(super::search_exit_code(&[]), 1);
        assert_eq!(super::search_exit_code(&[hit]), 0);
    }

    #[test]
    fn empty_read_results_exit_nonzero() {
        assert_eq!(super::results_exit_code(0), 1);
        assert_eq!(super::results_exit_code(1), 0);
    }

    #[test]
    fn empty_recent_results_exit_nonzero() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();

        let rows =
            collect_recent_docs(&[&conn], None, None, None, None, None, None, None, 20).unwrap();

        assert!(rows.is_empty());
        assert_eq!(super::results_exit_code(rows.len()), 1);
    }

    #[test]
    fn default_search_preview_is_five_lines_capped_at_110_chars() {
        let text = [
            "line one",
            "line two",
            "line three",
            "line four",
            "line five",
            "line six",
        ]
        .join("\n");
        let lines = super::preview_lines(&text, 5, 110);

        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], "line one");
        assert_eq!(lines[4], "line five");

        let long = "x".repeat(130);
        let lines = super::preview_lines(&long, 5, 110);
        assert_eq!(lines, vec![format!("{}...", "x".repeat(110))]);
    }

    #[test]
    fn full_snippet_zero_is_uncapped() {
        let text = "alpha\nbeta";
        assert_eq!(snippet(text, 0), text);
    }

    #[test]
    fn related_reason_names_shared_container() {
        assert_eq!(
            related_reason("in_repo", "container", "git:src"),
            "shared container git:src"
        );
    }

    #[test]
    fn tag_boosts_adjust_scores() {
        let mut hits = vec![
            test_hit("a", 10, Some("canonical")),
            test_hit("b", 10, None),
        ];
        apply_tag_boosts(&mut hits);
        assert!(hits[0].score >= hits[1].score);
    }

    #[test]
    fn recency_decay_demotes_old_hits_and_respects_floor() {
        let now = chrono::Utc.with_ymd_and_hms(2026, 7, 6, 0, 0, 0).unwrap();
        let mut recent = test_hit("recent", 100, None);
        recent.updated_at = Some("2026-07-06T00:00:00Z".into());
        let mut old = test_hit("old", 100, None);
        old.updated_at = Some("2021-07-06T00:00:00Z".into());
        let mut hits = vec![old, recent];

        apply_recency_decay_with(&mut hits, 1.0, 0.75, now);

        assert_eq!(hits[0].doc_id, "recent");
        assert_eq!(hits[0].score, 100);
        assert_eq!(hits[1].score, 75);
    }

    #[test]
    fn recency_age_uses_quarter_year_for_missing_dates() {
        let now = chrono::Utc.with_ymd_and_hms(2026, 7, 6, 0, 0, 0).unwrap();
        assert_eq!(age_years(None, now), 0.25);
        assert_eq!(age_years(Some("not-a-date"), now), 0.25);
    }

    #[test]
    fn replacement_pointer_uses_confirmed_lineage() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();
        insert_doc(&conn, "old", "docs/old.md", "Old");
        insert_doc(&conn, "new", "docs/new.md", "New");
        for (span, doc, line) in [("old-s", "old", 1), ("new-s", "new", 7)] {
            conn.execute(
                "INSERT INTO spans (span_id, doc_id, chunk_id, span_kind, label, start_byte, end_byte, start_line, end_line, stable_hash, metadata_json)
                 VALUES (?1, ?2, NULL, 'paragraph', NULL, 0, 10, ?3, ?3, ?4, '{}')",
                params![span, doc, line, format!("hash-{span}")],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO lineage_edges (lineage_id, from_span_id, to_span_id, rel, status, confidence, confirmed_by, confirmed_at, last_checked_at, metadata_json)
             VALUES ('l1', 'old-s', 'new-s', 'updates', 'confirmed', 0.9, 'test', 'now', 'now', '{}')",
            [],
        )
        .unwrap();

        let pointer = replacement_pointer(&conn, "old").unwrap().unwrap();
        assert!(pointer.contains("docs/new.md:7-7"));
        assert!(pointer.contains("via updates"));
    }

    #[test]
    fn replacement_pointer_uses_supersedes_edge() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();
        insert_doc(&conn, "old", "docs/old.md", "Old");
        insert_doc(&conn, "new", "docs/new.md", "New");
        conn.execute(
            "INSERT INTO edges (edge_id, from_type, from_id, to_type, to_id, rel, confidence, evidence_span_id, created_by, created_at, metadata_json)
             VALUES ('e1', 'doc', 'old', 'doc', 'new', 'supersedes', 1.0, NULL, 'test', 'now', '{}')",
            [],
        )
        .unwrap();

        let pointer = replacement_pointer(&conn, "old").unwrap().unwrap();
        assert!(pointer.contains("docs/new.md"));
        assert!(pointer.contains("via supersedes"));
    }

    #[test]
    fn recent_tag_filter_uses_doc_id() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();
        insert_doc(&conn, "doc1", "docs/a.md", "A");
        conn.execute(
            "INSERT INTO tags (target_type, target_id, status, note, \"by\", \"at\", metadata_json)
             VALUES ('doc', 'doc1', 'canonical', '', 'test', 'now', '{}')",
            [],
        )
        .unwrap();

        let rows = super::recent_docs_for_catalog(
            &conn,
            None,
            None,
            None,
            None,
            None,
            Some("canonical"),
            None,
        )
        .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].canonical_ref, "git:docs/a.md");
        assert!(super::has_tag(&conn, "doc1", "canonical"));
        assert!(!super::has_tag(&conn, "doc1", "deprecated"));
    }

    #[test]
    fn search_doc_filter_matches_doc_id() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();
        insert_doc_with_chunk(
            &conn,
            "opaque-doc-id",
            "docs/alpha.md",
            "Alpha",
            "needle text",
        );

        let hits = super::search_hits(
            &conn,
            "needle",
            &weighted_terms("needle", &[]),
            &[],
            None,
            Some("opaque-doc"),
            None,
            None,
            None,
            10,
            None,
            true,
            60.0,
        )
        .unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].doc_id, "opaque-doc-id");
    }

    #[test]
    fn recent_doc_filter_matches_doc_id() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();
        insert_doc(&conn, "opaque-doc-id", "docs/alpha.md", "Alpha");

        let rows = super::recent_docs_for_catalog(
            &conn,
            None,
            Some("opaque-doc"),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].canonical_ref, "git:docs/alpha.md");
    }

    #[test]
    fn doc_lookup_matches_doc_id() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();
        insert_doc(&conn, "opaque-doc-id", "docs/alpha.md", "Alpha");

        let rows = super::find_docs_for_catalog(&conn, "opaque-doc-id", None, 5).unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].canonical_ref, "git:docs/alpha.md");
        assert_eq!(rows[0].rank, 0);
    }

    #[test]
    fn keyword_search_excludes_large_chunks() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();
        insert_doc(&conn, "doc1", "docs/a.md", "A");
        for (chunk_id, text, large) in [
            ("base", "plain text", false),
            ("large", "needle only in coarse chunk", true),
        ] {
            conn.execute(
                "INSERT INTO chunks (chunk_id, doc_id, chunk_index, heading_path, section_anchor, start_byte, end_byte, start_line, end_line, token_count, text, text_sha256, metadata_json)
                 VALUES (?1, 'doc1', ?2, '(root)', NULL, 0, 10, 1, 1, 2, ?3, ?4, ?5)",
                params![
                    chunk_id,
                    if large { 1i64 } else { 0i64 },
                    text,
                    format!("sha-{chunk_id}"),
                    json!({"large": large}).to_string(),
                ],
            )
            .unwrap();
        }

        let hits = super::search_hits(
            &conn,
            "needle",
            &weighted_terms("needle", &[]),
            &[],
            None,
            None,
            None,
            None,
            None,
            10,
            None,
            true,
            60.0,
        )
        .unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn path_dedup_collapses_same_file_from_multiple_sources() {
        // Two sources indexing the same local file+span collapse to one hit
        // (the higher-scored one, since hits arrive score-sorted).
        let mk = |source: &str, score: i64| Hit {
            doc_id: format!("{source}-doc"),
            chunk_id: format!("{source}#L1"),
            chunk_index: 0,
            source: source.into(),
            canonical_ref: format!("{source}:docs/pricing.md"),
            title: "Pricing".into(),
            path: Some("docs/pricing.md".into()),
            url: None,
            author: None,
            updated_at: None,
            heading_path: String::new(),
            start_byte: 0,
            end_byte: 10,
            start_line: 1,
            end_line: 8,
            score,
            tag: None,
            tag_note: None,
            replacement: None,
            matched_terms: vec!["pricing".into()],
            text: "The plan costs $49".into(),
            context: Vec::new(),
            score_parts: ScoreParts::default(),
        };
        let mut hits = vec![mk("git", 100), mk("localfiles", 90)];
        dedupe_hits(&mut hits);
        assert_eq!(hits.len(), 1, "same file from two sources should collapse");
        assert_eq!(hits[0].source, "git"); // higher score kept
    }

    #[test]
    fn search_hits_expose_score_parts_for_navigation() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();
        insert_doc_with_chunk(&conn, "doc1", "docs/a.md", "A", "refund policy text");

        let hits = super::search_hits(
            &conn,
            "refund",
            &weighted_terms("refund", &[]),
            &[],
            None,
            None,
            None,
            None,
            None,
            10,
            None,
            true,
            60.0,
        )
        .unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].score_parts.keyword, hits[0].score);
        assert_eq!(hits[0].score_parts.vector, 0);
        assert_eq!(hits[0].score_parts.tag_multiplier, 1.0);
        assert_eq!(hits[0].score_parts.recency_multiplier, 1.0);
    }

    #[test]
    fn ranked_search_unions_repo_and_global_catalogs() {
        let repo = Connection::open_in_memory().unwrap();
        let global = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&repo).unwrap();
        super::super::ensure_schema(&global).unwrap();
        insert_doc_with_chunk(
            &repo,
            "repo-doc",
            "docs/repo.md",
            "Repo",
            "repo only catalog text",
        );
        insert_doc_with_chunk(
            &global,
            "global-doc",
            "docs/global.md",
            "Global",
            "global refund policy text",
        );

        let args = search_args("refund");
        let hits = ranked_hits_across_catalogs(
            &args,
            &weighted_terms("refund", &[]),
            10,
            &[&repo, &global],
        )
        .unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].doc_id, "global-doc");
    }

    #[test]
    fn ranked_search_dedupes_across_catalogs_after_sorting() {
        let repo = Connection::open_in_memory().unwrap();
        let global = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&repo).unwrap();
        super::super::ensure_schema(&global).unwrap();
        insert_doc_with_chunk(
            &repo,
            "shared-doc",
            "docs/shared.md",
            "Shared",
            "refund text",
        );
        insert_doc_with_chunk(
            &global,
            "shared-doc",
            "docs/shared.md",
            "Shared",
            "refund policy text",
        );

        let args = search_args("refund policy");
        let hits = ranked_hits_across_catalogs(
            &args,
            &weighted_terms("refund policy", &[]),
            10,
            &[&repo, &global],
        )
        .unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].doc_id, "shared-doc");
        assert_eq!(hits[0].score, 6);
    }

    #[test]
    fn recent_unions_repo_and_global_catalogs() {
        let repo = Connection::open_in_memory().unwrap();
        let global = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&repo).unwrap();
        super::super::ensure_schema(&global).unwrap();
        insert_doc(&repo, "repo-doc", "docs/repo.md", "Repo");
        insert_doc(&global, "global-doc", "docs/global.md", "Global");

        let rows = collect_recent_docs(
            &[&repo, &global],
            None,
            Some("global"),
            None,
            None,
            None,
            None,
            None,
            10,
        )
        .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].canonical_ref, "git:docs/global.md");
    }

    #[test]
    fn doc_lookup_unions_repo_and_global_catalogs() {
        let repo = Connection::open_in_memory().unwrap();
        let global = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&repo).unwrap();
        super::super::ensure_schema(&global).unwrap();
        insert_doc(&repo, "repo-doc", "docs/repo.md", "Repo");
        insert_doc(&global, "global-doc", "docs/global.md", "Global");

        let rows = find_docs_across_catalogs(&[&repo, &global], "global", None, 5).unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].canonical_ref, "git:docs/global.md");
    }

    #[test]
    fn neighbors_resolves_global_chunk_ids() {
        let repo = Connection::open_in_memory().unwrap();
        let global = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&repo).unwrap();
        super::super::ensure_schema(&global).unwrap();
        insert_doc_with_chunk(
            &global,
            "global-doc",
            "docs/global.md",
            "Global",
            "global body",
        );

        let rows = neighbor_rows_across_catalogs(&[&repo, &global], "global-doc:0", 1)
            .unwrap()
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].chunk_id, "global-doc:0");
    }

    #[test]
    fn related_resolves_global_seed_docs() {
        let repo = Connection::open_in_memory().unwrap();
        let global = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&repo).unwrap();
        super::super::ensure_schema(&global).unwrap();
        insert_doc(&global, "seed", "docs/seed.md", "Seed");
        insert_doc(&global, "peer", "docs/peer.md", "Peer");
        for (edge_id, from) in [("e1", "seed"), ("e2", "peer")] {
            conn_insert_edge(&global, edge_id, from, "container", "git:docs", "in_repo");
        }

        let (canonical_ref, _title, rows) =
            related_rows_across_catalogs(&[&repo, &global], "seed", None, 10)
                .unwrap()
                .unwrap();

        assert_eq!(canonical_ref, "git:docs/seed.md");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].canonical_ref, "git:docs/peer.md");
        assert_eq!(
            related_reason(&rows[0].rel, &rows[0].to_type, &rows[0].to_id),
            "shared container git:docs"
        );
    }

    #[test]
    fn related_includes_direct_outgoing_doc_edges() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();
        insert_doc(&conn, "seed", "docs/seed.md", "Seed");
        insert_doc(&conn, "target", "docs/target.md", "Target");
        conn_insert_edge(&conn, "e1", "seed", "doc", "target", "links_to");

        let rows = super::related_rows_for_catalog(&conn, "seed", 10).unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].canonical_ref, "git:docs/target.md");
        assert_eq!(
            related_reason(&rows[0].rel, &rows[0].to_type, &rows[0].to_id),
            "shared link target"
        );
    }

    #[test]
    fn related_includes_direct_incoming_doc_edges() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();
        insert_doc(&conn, "seed", "docs/seed.md", "Seed");
        insert_doc(&conn, "source", "docs/source.md", "Source");
        conn_insert_edge(&conn, "e1", "source", "doc", "seed", "links_to");

        let rows = super::related_rows_for_catalog(&conn, "seed", 10).unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].canonical_ref, "git:docs/source.md");
        assert_eq!(
            related_reason(&rows[0].rel, &rows[0].to_type, &rows[0].to_id),
            "shared link source"
        );
    }

    #[test]
    fn section_merge_coalesces_adjacent_same_doc_hits() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::ensure_schema(&conn).unwrap();
        insert_doc_with_body(
            &conn,
            "doc1",
            "docs/a.md",
            "A",
            "first needle line\nsecond needle line\nthird other line\n",
        );
        for (chunk_id, idx, start, end, start_line, end_line, text) in [
            ("c1", 0i64, 0i64, 18i64, 1i64, 1i64, "first needle line\n"),
            ("c2", 1, 18, 37, 2, 2, "second needle line\n"),
        ] {
            conn.execute(
                "INSERT INTO chunks (chunk_id, doc_id, chunk_index, heading_path, section_anchor, start_byte, end_byte, start_line, end_line, token_count, text, text_sha256, metadata_json)
                 VALUES (?1, 'doc1', ?2, '(root)', NULL, ?3, ?4, ?5, ?6, 3, ?7, ?8, '{}')",
                params![chunk_id, idx, start, end, start_line, end_line, text, format!("sha-{chunk_id}")],
            )
            .unwrap();
        }

        let mut hits = super::search_hits(
            &conn,
            "needle",
            &weighted_terms("needle", &[]),
            &[],
            None,
            None,
            None,
            None,
            None,
            10,
            None,
            true,
            60.0,
        )
        .unwrap();
        super::merge_sections(&conn, &mut hits).unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].start_line, 1);
        assert_eq!(hits[0].end_line, 2);
        assert!(hits[0]
            .text
            .contains("first needle line\nsecond needle line"));
    }

    fn insert_doc(conn: &Connection, doc_id: &str, path: &str, title: &str) {
        insert_doc_with_body(conn, doc_id, path, title, "# Doc");
    }

    fn insert_doc_with_body(conn: &Connection, doc_id: &str, path: &str, title: &str, body: &str) {
        conn.execute(
            "INSERT INTO documents (doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind, author_id, author_name, created_at, updated_at, observed_at, version, content_sha256, body, metadata_json)
             VALUES (?1, 'git', ?2, ?3, ?4, NULL, ?2, 'text/markdown', 'file', NULL, NULL, NULL, 'now', 'now', 'v', 'sha', ?5, '{}')",
            params![doc_id, path, format!("git:{path}"), title, body],
        )
        .unwrap();
    }

    fn insert_doc_with_chunk(conn: &Connection, doc_id: &str, path: &str, title: &str, text: &str) {
        insert_doc_with_body(conn, doc_id, path, title, text);
        conn.execute(
            "INSERT INTO chunks (chunk_id, doc_id, chunk_index, heading_path, section_anchor, start_byte, end_byte, start_line, end_line, token_count, text, text_sha256, metadata_json)
             VALUES (?1, ?2, 0, '(root)', NULL, 0, ?3, 1, 1, 3, ?4, ?5, '{}')",
            params![
                format!("{doc_id}:0"),
                doc_id,
                text.len() as i64,
                text,
                format!("sha-{doc_id}")
            ],
        )
        .unwrap();
    }

    fn conn_insert_edge(
        conn: &Connection,
        edge_id: &str,
        from_id: &str,
        to_type: &str,
        to_id: &str,
        rel: &str,
    ) {
        conn.execute(
            "INSERT INTO edges (edge_id, from_type, from_id, to_type, to_id, rel, confidence, evidence_span_id, created_by, created_at, metadata_json)
             VALUES (?1, 'doc', ?2, ?3, ?4, ?5, 1.0, NULL, 'test', 'now', '{}')",
            params![edge_id, from_id, to_type, to_id, rel],
        )
        .unwrap();
    }

    fn search_args(query: &str) -> SearchArgs {
        SearchArgs {
            query: query.into(),
            full: None,
            variants: Vec::new(),
            k: None,
            source: None,
            doc: None,
            author: None,
            since: None,
            before: None,
            tag: None,
            no_tag: None,
            expand: None,
            json: false,
        }
    }

    fn test_hit(id: &str, score: i64, tag: Option<&str>) -> Hit {
        Hit {
            doc_id: id.into(),
            chunk_id: format!("{id}:0"),
            chunk_index: 0,
            source: "git".into(),
            canonical_ref: format!("git:{id}.md"),
            title: id.into(),
            path: Some(format!("{id}.md")),
            url: None,
            author: None,
            updated_at: None,
            heading_path: "(root)".into(),
            start_byte: 0,
            end_byte: 1,
            start_line: 1,
            end_line: 1,
            score,
            score_parts: ScoreParts {
                keyword: score,
                vector: 0,
                tag_multiplier: 1.0,
                recency_multiplier: 1.0,
            },
            tag: tag.map(str::to_string),
            tag_note: None,
            replacement: None,
            matched_terms: vec!["duckdb".into()],
            text: "duckdb".into(),
            context: Vec::new(),
        }
    }
}
