//! Deterministic grounding against FACTS.md or an explicit source (SPEC §5.5/§11.10).

use crate::{config, index, workspace};
use anyhow::{Context, Result};
use regex::Regex;
use serde::{Serialize, Serializer};
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

pub struct FactcheckArgs {
    pub file: String,
    pub source: Option<String>,
    pub kb: bool,
    pub models: bool,
    pub decompose: bool,
    pub claims: Option<String>,
    pub emit_claim_targets: bool,
    pub deep: bool,
    pub ground: Option<String>,
    pub threshold: Option<f64>,
    pub json: bool,
    pub strict: bool,
    pub quiet: bool,
    pub lookback: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
struct Finding {
    rule_id: &'static str,
    severity: &'static str,
    sentence: String,
    span: String,
    message: String,
    fact: Option<String>,
}

#[derive(Debug, Clone)]
struct Fact {
    text: String,
    spans: BTreeSet<TypedSpan>,
    terms: BTreeSet<String>,
    entities: BTreeSet<String>,
    source_ref: Option<String>,
    source_status: Option<String>,
    can_support: bool,
}

#[derive(Debug, Clone, Serialize)]
struct Candidate {
    sentence: String,
    spans: Vec<TypedSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    claim: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroundingMode {
    Facts,
    Source,
    Kb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SpanKind {
    Percent,
    Money,
    Date,
    Year,
    Count,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct TypedSpan {
    value: String,
    kind: SpanKind,
}

impl Serialize for TypedSpan {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.value)
    }
}

fn attention_finding(source_path: &str, sentence: &str, score: f64) -> Finding {
    Finding {
        rule_id: "ungrounded-span",
        severity: "advisory",
        sentence: sentence.chars().take(160).collect(),
        span: sentence.chars().take(80).collect(),
        message: format!(
            "reads as ungrounded against {source_path} ({:.0}% of peak attention) — a lead, not an assertion of falsehood",
            score * 100.0
        ),
        fact: None,
    }
}

pub fn run(args: FactcheckArgs) -> Result<i32> {
    if args.models {
        eprintln!("note: local NLI model tier is not available in this build; running deterministic factcheck only");
    }
    if let Some(ground) = args.ground.as_deref() {
        if ground != "attention" {
            eprintln!("✗ unknown grounding mode `{ground}`; expected attention");
            return Ok(2);
        }
    }
    let attention_grounding = args.deep || args.ground.as_deref() == Some("attention");
    if attention_grounding && args.source.is_none() {
        eprintln!("✗ --deep/--ground=attention requires --source <file> (§5.5)");
        return Ok(2);
    }
    if args.decompose {
        eprintln!("note: --decompose is agent-side; use --emit-claim-targets, decompose externally, then pass --claims <file>");
    }
    let _ = args.lookback;

    let target_text = std::fs::read_to_string(&args.file)
        .with_context(|| format!("reading target file {}", args.file))?;
    let candidates = candidate_sentences(&target_text);
    if args.emit_claim_targets {
        println!(
            "{}",
            serde_json::to_string_pretty(&claim_target_sentences(&target_text))?
        );
        return Ok(0);
    }

    let target_sentences = if let Some(claims) = args.claims.as_deref() {
        let text =
            std::fs::read_to_string(claims).with_context(|| format!("reading claims {claims}"))?;
        align_claims_to_parents(&target_text, &text)
    } else {
        candidates
    };

    let (facts, mode) = if args.kb {
        (load_kb_facts()?, GroundingMode::Kb)
    } else if args.source.is_some() {
        (load_facts(args.source.as_deref())?, GroundingMode::Source)
    } else {
        (load_facts(None)?, GroundingMode::Facts)
    };
    let mut findings = check_sentences(&target_sentences, &facts, mode);
    if attention_grounding {
        // Attention tier (§11.10): sentences that barely attend to the
        // source read as ungrounded — advisory, never an assertion of
        // falsehood. Threshold default 0.10.
        let source_path = args.source.as_deref().unwrap();
        let source_text = std::fs::read_to_string(source_path)
            .with_context(|| format!("reading grounding source {source_path}"))?;
        // The spec's 0.10 was calibrated against the prototype's row-normalized
        // scores; this port preserves absolute attention mass (flatter peaks),
        // so the shared attention.threshold (default 0.3) applies here too.
        let threshold = args.threshold.unwrap_or_else(|| {
            crate::config::resolve(Some(&crate::workspace::work_root()))["attention"]["threshold"]
                .as_f64()
                .unwrap_or(0.3)
        });
        match crate::attn::analyze(
            &source_text,
            &target_text,
            crate::attn::Mode::Grounding,
            threshold,
            None,
        ) {
            Ok(flagged) => {
                for f in flagged {
                    let sentence: String = f.text.split_whitespace().collect::<Vec<_>>().join(" ");
                    findings.push(attention_finding(source_path, &sentence, f.score));
                }
            }
            Err(e) => eprintln!("✗ attention grounding failed: {e:#}"),
        }
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&findings)?);
    } else if findings.is_empty() {
        if !args.quiet {
            println!(
                "factcheck: ok ({} candidate sentence(s), {} fact(s))",
                target_sentences.len(),
                facts.len()
            );
        }
    } else if !args.quiet {
        for f in &findings {
            println!("{} {}: {}", f.severity, f.rule_id, f.message);
            println!("  sentence: {}", f.sentence);
            if let Some(fact) = &f.fact {
                println!("  fact: {fact}");
            }
        }
    }

    let has_error = findings.iter().any(|f| f.severity == "error");
    if has_error || (args.strict && !findings.is_empty()) {
        Ok(1)
    } else {
        Ok(0)
    }
}

fn load_facts(source: Option<&str>) -> Result<Vec<Fact>> {
    let root = workspace::work_root();
    let path = source.map(PathBuf::from).unwrap_or_else(|| {
        root.join(
            config::resolve(Some(&root))["facts"]["file"]
                .as_str()
                .unwrap_or("FACTS.md"),
        )
    });
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    if source.is_some() {
        return Ok(sentences(&text)
            .into_iter()
            .map(|s| fact_from_text(&s))
            .collect());
    }
    Ok(parse_fact_ledger(&text))
}

fn parse_fact_ledger(text: &str) -> Vec<Fact> {
    text.lines()
        .filter_map(|line| {
            let mut t = line.trim();
            if t.is_empty() || t.starts_with('#') || t.starts_with("<!--") {
                return None;
            }
            t = t.trim_start_matches(['-', '*']).trim();
            let (claim, source_ref) = split_fact_source(t);
            let claim = claim.trim();
            (!claim.is_empty()).then(|| {
                let mut fact = fact_from_text(claim);
                fact.source_ref = source_ref.map(str::to_string);
                fact
            })
        })
        .collect()
}

fn split_fact_source(s: &str) -> (&str, Option<&str>) {
    let trimmed = s.trim_end();
    for (open, close) in [('(', ')'), ('[', ']')] {
        if trimmed.ends_with(close) {
            if let Some(idx) = trimmed.rfind(open) {
                let inside = &trimmed[idx + 1..trimmed.len() - 1];
                if is_fact_source_attribution(inside) {
                    return (trimmed[..idx].trim_end(), Some(inside.trim()));
                }
            }
        }
    }
    (trimmed, None)
}

fn is_fact_source_attribution(text: &str) -> bool {
    text.contains("source:")
        || text.contains("http://")
        || text.contains("https://")
        || Regex::new(r"\b(19|20)\d{2}\b").unwrap().is_match(text)
}

fn fact_from_text(text: &str) -> Fact {
    Fact {
        text: text.to_string(),
        spans: typed_spans(text).into_iter().collect(),
        terms: salient_terms(text),
        entities: entities(text),
        source_ref: None,
        source_status: None,
        can_support: true,
    }
}

fn kb_fact_from_text(text: &str, source_ref: String, source_status: Option<String>) -> Fact {
    let mut fact = fact_from_text(text);
    fact.source_ref = Some(source_ref);
    fact.can_support = matches!(source_status.as_deref(), Some("canonical"));
    fact.source_status = source_status;
    fact
}

fn load_kb_facts() -> Result<Vec<Fact>> {
    let mut paths = vec![
        index::catalog_path(false),
        workspace::global_workspace_dir().join(index::CATALOG_FILE),
    ];
    paths.sort();
    paths.dedup();
    let paths: Vec<PathBuf> = paths
        .into_iter()
        .filter(|p| index::warehouse_published_at(p))
        .collect();
    load_kb_facts_from_paths(&paths)
}

fn load_kb_facts_from_paths(paths: &[PathBuf]) -> Result<Vec<Fact>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let mut facts = Vec::new();
    for path in paths {
        let Some(conn) = index::open_readonly_path(path)? else {
            continue;
        };
        let mut stmt = conn.prepare(
            "SELECT d.canonical_ref, d.body, t.status
               FROM documents d
               JOIN tags t
                 ON t.target_type = 'doc'
                AND (
                  t.target_id = d.doc_id
                  OR t.target_id = d.canonical_ref
                  OR t.target_id = d.path
                  OR t.target_id = d.external_id
                )
              WHERE t.status IN ('canonical', 'stale', 'deprecated', 'draft')",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;
        for row in rows.flatten() {
            for sentence in sentences(&row.1) {
                if !typed_spans(&sentence).is_empty() {
                    facts.push(kb_fact_from_text(&sentence, row.0.clone(), row.2.clone()));
                }
            }
        }
    }
    Ok(facts)
}

fn check_sentences(candidates: &[Candidate], facts: &[Fact], mode: GroundingMode) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut decomposed_seen = HashSet::new();
    for candidate in candidates {
        let analysis_text = candidate.analysis_text();
        let terms = salient_terms(analysis_text);
        let candidate_entities = entities(analysis_text);
        let mut unsupported_spans = Vec::new();
        for span in &candidate.spans {
            if span_supported_by_facts(span, &terms, &candidate_entities, facts) {
                continue;
            }
            if let Some(fact) = disallowed_fact_for_span(span, &terms, &candidate_entities, facts) {
                let status = fact.source_status.as_deref().unwrap_or("untrusted");
                let source = fact.source_ref.as_deref().unwrap_or("knowledge base");
                push_finding(
                    &mut findings,
                    &mut decomposed_seen,
                    candidate,
                    Finding {
                        rule_id: "unsupported-claim",
                        severity: "warn",
                        sentence: candidate.sentence.clone(),
                        span: span.value.clone(),
                        message: candidate.message(format!(
                            "typed span `{}` is only supported by {status} source {source}",
                            span.value
                        )),
                        fact: Some(fact.text.clone()),
                    },
                );
                continue;
            }
            if let Some(fact) = closest_conflicting_fact(span, &terms, &candidate_entities, facts) {
                push_finding(
                    &mut findings,
                    &mut decomposed_seen,
                    candidate,
                    Finding {
                        rule_id: conflict_rule_id(span),
                        severity: "error",
                        sentence: candidate.sentence.clone(),
                        span: span.value.clone(),
                        message: candidate.message(format!(
                            "typed span `{}` is not supported and conflicts with a nearby fact",
                            span.value
                        )),
                        fact: Some(fact.text.clone()),
                    },
                );
            } else {
                unsupported_spans.push(span.clone());
            }
        }
        if let Some(span) = highest_value_span(&unsupported_spans) {
            let severity = if mode == GroundingMode::Source {
                "warn"
            } else {
                "advisory"
            };
            push_finding(
                &mut findings,
                &mut decomposed_seen,
                candidate,
                Finding {
                    rule_id: "unsupported-claim",
                    severity,
                    sentence: candidate.sentence.clone(),
                    span: span.value.clone(),
                    message: candidate.message(unsupported_message(span, mode)),
                    fact: None,
                },
            );
        }
    }
    findings
}

fn push_finding(
    findings: &mut Vec<Finding>,
    decomposed_seen: &mut HashSet<(String, &'static str, String, String)>,
    candidate: &Candidate,
    finding: Finding,
) {
    if candidate.claim.is_some() {
        let key = (
            finding.sentence.clone(),
            finding.rule_id,
            finding.span.clone(),
            finding.message.clone(),
        );
        if !decomposed_seen.insert(key) {
            return;
        }
    }
    findings.push(finding);
}

impl Candidate {
    fn analysis_text(&self) -> &str {
        self.claim.as_deref().unwrap_or(&self.sentence)
    }

    fn message(&self, message: String) -> String {
        if let Some(claim) = &self.claim {
            format!("{message}; atomic claim: {claim}")
        } else {
            message
        }
    }
}

fn unsupported_message(span: &TypedSpan, mode: GroundingMode) -> String {
    if mode == GroundingMode::Source {
        format!(
            "typed span `{}` is not supported by the grounding source",
            span.value
        )
    } else {
        format!(
            "typed span `{}` is not supported by the grounding facts",
            span.value
        )
    }
}

fn highest_value_span(spans: &[TypedSpan]) -> Option<&TypedSpan> {
    spans.iter().min_by_key(|span| span_priority(span))
}

fn span_priority(span: &TypedSpan) -> u8 {
    match span.kind {
        SpanKind::Money => 0,
        SpanKind::Percent => 1,
        SpanKind::Year | SpanKind::Date => 2,
        SpanKind::Count => 3,
    }
}

fn span_supported_by_facts(
    span: &TypedSpan,
    terms: &BTreeSet<String>,
    candidate_entities: &BTreeSet<String>,
    facts: &[Fact],
) -> bool {
    facts.iter().filter(|f| f.can_support).any(|f| {
        relevant_fact(terms, candidate_entities, f)
            && f.spans
                .iter()
                .any(|fact_span| spans_compatible(span, fact_span))
    })
}

fn closest_conflicting_fact<'a>(
    span: &TypedSpan,
    terms: &BTreeSet<String>,
    candidate_entities: &BTreeSet<String>,
    facts: &'a [Fact],
) -> Option<&'a Fact> {
    facts
        .iter()
        .filter(|f| f.can_support)
        .filter(|f| relevant_fact(terms, candidate_entities, f))
        .filter(|f| {
            f.spans
                .iter()
                .any(|fact_span| !spans_compatible(span, fact_span) && fact_span.kind == span.kind)
        })
        .map(|f| {
            let score = relevance_score(terms, candidate_entities, f);
            (score, f)
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, f)| f)
}

fn relevant_fact(
    terms: &BTreeSet<String>,
    candidate_entities: &BTreeSet<String>,
    fact: &Fact,
) -> bool {
    let shared_terms = terms.intersection(&fact.terms).count();
    shared_terms >= 1 && relevance_score(terms, candidate_entities, fact) >= 3
}

fn relevance_score(
    terms: &BTreeSet<String>,
    candidate_entities: &BTreeSet<String>,
    fact: &Fact,
) -> usize {
    let shared_terms = terms.intersection(&fact.terms).count();
    let shared_entities = candidate_entities.intersection(&fact.entities).count();
    shared_terms + 2 * shared_entities
}

fn conflict_rule_id(span: &TypedSpan) -> &'static str {
    match span.kind {
        SpanKind::Date | SpanKind::Year => "number-date-mismatch",
        SpanKind::Percent | SpanKind::Money | SpanKind::Count => "contradicts-fact",
    }
}

fn spans_compatible(a: &TypedSpan, b: &TypedSpan) -> bool {
    if a.kind == b.kind && a.value == b.value {
        return true;
    }
    if a.kind == SpanKind::Date && b.kind == SpanKind::Date {
        return date_spans_compatible(&a.value, &b.value);
    }
    false
}

fn date_spans_compatible(a: &str, b: &str) -> bool {
    let shorter = if a.len() <= b.len() { a } else { b };
    let longer = if a.len() <= b.len() { b } else { a };
    matches!(shorter.len(), 7 | 10) && longer.starts_with(shorter)
}

fn disallowed_fact_for_span<'a>(
    span: &TypedSpan,
    terms: &BTreeSet<String>,
    candidate_entities: &BTreeSet<String>,
    facts: &'a [Fact],
) -> Option<&'a Fact> {
    facts.iter().find(|f| {
        !f.can_support
            && relevant_fact(terms, candidate_entities, f)
            && f.spans
                .iter()
                .any(|fact_span| spans_compatible(span, fact_span))
    })
}

fn candidate_sentences(text: &str) -> Vec<Candidate> {
    sentences(text)
        .into_iter()
        .filter_map(|sentence| {
            let spans = typed_spans(&sentence);
            is_checkable_sentence(&sentence, &spans).then_some(Candidate {
                sentence,
                spans,
                claim: None,
            })
        })
        .collect()
}

fn is_checkable_sentence(sentence: &str, spans: &[TypedSpan]) -> bool {
    !spans.is_empty() && (content_token_count(sentence) >= 2 || !entities(sentence).is_empty())
}

fn claim_target_sentences(text: &str) -> Vec<Candidate> {
    sentences(text)
        .into_iter()
        .filter_map(|sentence| {
            let spans = typed_spans(&sentence);
            (!spans.is_empty() || content_token_count(&sentence) >= 4).then_some(Candidate {
                sentence,
                spans,
                claim: None,
            })
        })
        .collect()
}

fn align_claims_to_parents(target_text: &str, claims_text: &str) -> Vec<Candidate> {
    let parents = claim_target_sentences(target_text);
    let claims = claim_target_sentences(claims_text);
    claims
        .into_iter()
        .enumerate()
        .filter_map(|(idx, claim)| {
            let parent = parents.get(idx)?;
            Some(Candidate {
                sentence: parent.sentence.clone(),
                spans: claim.spans,
                claim: Some(claim.sentence),
            })
        })
        .collect()
}

fn content_token_count(text: &str) -> usize {
    const STOP: &[&str] = &[
        "the", "a", "an", "and", "or", "to", "of", "in", "on", "for", "with", "by", "is", "are",
        "was", "were", "be", "been", "this", "that", "it", "we", "our", "from", "after", "before",
        "per", "as", "at",
    ];
    let stop: BTreeSet<&str> = STOP.iter().copied().collect();
    Regex::new(r"[A-Za-z][A-Za-z0-9_-]{2,}")
        .unwrap()
        .find_iter(text)
        .map(|m| m.as_str().to_ascii_lowercase())
        .filter(|w| !stop.contains(w.as_str()))
        .count()
}

fn entities(text: &str) -> BTreeSet<String> {
    let token_re = Regex::new(r"\b[A-Za-z][A-Za-z0-9]*\b").unwrap();
    let tokens: Vec<(String, usize)> = token_re
        .find_iter(text)
        .map(|m| (m.as_str().to_string(), m.start()))
        .collect();
    let mut out = BTreeSet::new();
    let mut i = 0;
    while i < tokens.len() {
        let (token, start) = &tokens[i];
        if !is_entity_word(token) {
            i += 1;
            continue;
        }

        let mut parts = vec![token.clone()];
        let mut entity_words = 1;
        let mut j = i + 1;
        while j < tokens.len() {
            let next = &tokens[j].0;
            if is_entity_word(next) {
                parts.push(next.clone());
                entity_words += 1;
                j += 1;
                continue;
            }
            if is_entity_connector(next) && j + 1 < tokens.len() && is_entity_word(&tokens[j + 1].0)
            {
                parts.push(next.clone());
                parts.push(tokens[j + 1].0.clone());
                entity_words += 1;
                j += 2;
                continue;
            }
            break;
        }

        if entity_words > 1
            || !is_sentence_initial_word(text, *start)
            || is_acronym(token)
            || token.chars().any(|c| c.is_ascii_digit())
        {
            out.insert(parts.join(" "));
        }
        i = j.max(i + 1);
    }
    out
}

fn is_entity_word(token: &str) -> bool {
    is_acronym(token)
        || token.chars().any(|c| c.is_ascii_digit())
        || token.chars().next().is_some_and(|c| c.is_uppercase())
}

fn is_acronym(token: &str) -> bool {
    (2..=6).contains(&token.len()) && token.chars().all(|c| c.is_ascii_uppercase())
}

fn is_entity_connector(token: &str) -> bool {
    matches!(token.to_ascii_lowercase().as_str(), "of" | "the" | "and")
}

fn is_sentence_initial_word(text: &str, start: usize) -> bool {
    text[..start]
        .chars()
        .all(|c| c.is_whitespace() || matches!(c, '"' | '\'' | '(' | '['))
}

fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence || t.is_empty() || t.starts_with('#') || t.starts_with('|') {
            continue;
        }
        for s in t.split_inclusive(['.', '!', '?']) {
            let s = s.trim().trim_start_matches(['-', '*']).trim();
            if s.chars().filter(|c| c.is_alphabetic()).count() >= 8 {
                out.push(s.to_string());
            }
        }
    }
    out
}

fn typed_spans(text: &str) -> Vec<TypedSpan> {
    let re = Regex::new(
        r"(?x)
        (?:[$€£]\s?\d[\d,]*(?:\.\d+)?\s?(?i: million|billion|thousand|k|m|b)?)
        |(?:\b\d[\d,]*(?:\.\d+)?\s?%)
        |(?:\b(?:19|20)\d{2}-\d{2}(?:-\d{2})?\b)
        |(?:\b\d{1,2}[/-]\d{1,2}[/-]\d{2,4}\b)
        |(?:\b\d{1,2}(?:st|nd|rd|th)?\s+(?i:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Sept|Oct|Nov|Dec)[a-z]*,?\s+(?:19|20)\d{2}\b)
        |(?:\b(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Sept|Oct|Nov|Dec)[a-z]*\.?\s+\d{1,2},?\s+(?:19|20)\d{2}\b)
        |(?:\b(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Sept|Oct|Nov|Dec)[a-z]*\.?\s+(?:19|20)\d{2}\b)
        |(?:\b(?:19|20)\d{2}\b)
        |(?:\b\d[\d,]*(?:\.\d+)?\b)
        ",
    )
    .unwrap();
    let mut spans: Vec<TypedSpan> = re
        .find_iter(text)
        .map(|m| typed_span_from_raw(m.as_str()))
        .collect();
    spans.sort();
    spans.dedup();
    spans
}

fn typed_span_from_raw(raw: &str) -> TypedSpan {
    TypedSpan {
        value: normalize_span(raw),
        kind: raw_span_kind(raw),
    }
}

fn raw_span_kind(raw: &str) -> SpanKind {
    let trimmed = raw.trim().trim_end_matches(['.', ',', ';', ':']);
    if trimmed.starts_with(['$', '€', '£']) {
        return SpanKind::Money;
    }
    if trimmed.ends_with('%') {
        return SpanKind::Percent;
    }
    if normalize_date_span(trimmed).is_some()
        && !Regex::new(r"^(19|20)\d{2}$").unwrap().is_match(trimmed)
    {
        return SpanKind::Date;
    }
    if Regex::new(r"^(19|20)\d{2}$").unwrap().is_match(trimmed) {
        return SpanKind::Year;
    }
    SpanKind::Count
}

fn normalize_span(s: &str) -> String {
    if let Some(money) = normalize_money_span(s) {
        return money;
    }
    if let Some(date) = normalize_date_span(s) {
        return date;
    }
    if let Some(percent) = normalize_percent_span(s) {
        return percent;
    }
    if let Some(count) = normalize_count_span(s) {
        return count;
    }
    s.trim()
        .trim_end_matches(['.', ',', ';', ':'])
        .replace(' ', "")
        .to_ascii_lowercase()
}

fn normalize_percent_span(s: &str) -> Option<String> {
    let trimmed = s.trim().trim_end_matches(['.', ',', ';', ':']);
    let number = trimmed.strip_suffix('%')?.trim().replace(',', "");
    let value: f64 = number.parse().ok()?;
    Some(format!("{}%", format_scaled_number(value)))
}

fn normalize_count_span(s: &str) -> Option<String> {
    let trimmed = s.trim().trim_end_matches(['.', ',', ';', ':']);
    if !Regex::new(r"^\d[\d,]*(?:\.\d+)?$")
        .unwrap()
        .is_match(trimmed)
    {
        return None;
    }
    let value = trimmed.replace(',', "");
    if value.contains('.') {
        let parsed: f64 = value.parse().ok()?;
        return Some(format_scaled_number(parsed));
    }
    Some(value)
}

fn normalize_date_span(s: &str) -> Option<String> {
    let trimmed = s.trim().trim_end_matches(['.', ',', ';', ':']);
    if Regex::new(r"^(19|20)\d{2}-\d{2}(-\d{2})?$")
        .unwrap()
        .is_match(trimmed)
    {
        return Some(trimmed.to_string());
    }

    let cleaned = trimmed.replace(',', "");
    let parts: Vec<&str> = cleaned.split_whitespace().collect();
    match parts.as_slice() {
        [month, year] => {
            let month = month_number(month)?;
            Some(format!("{year}-{month:02}"))
        }
        [first, second, year] => {
            let (month, day) = if let Some(month) = month_number(first) {
                (month, ordinal_day(second)?)
            } else {
                (month_number(second)?, ordinal_day(first)?)
            };
            Some(format!("{year}-{month:02}-{day:02}"))
        }
        _ => None,
    }
}

fn month_number(raw: &str) -> Option<u8> {
    let lower = raw.trim_end_matches('.').to_ascii_lowercase();
    let prefix = if lower.starts_with("sept") {
        "sep"
    } else {
        lower.get(0..3)?
    };
    match prefix {
        "jan" => Some(1),
        "feb" => Some(2),
        "mar" => Some(3),
        "apr" => Some(4),
        "may" => Some(5),
        "jun" => Some(6),
        "jul" => Some(7),
        "aug" => Some(8),
        "sep" => Some(9),
        "oct" => Some(10),
        "nov" => Some(11),
        "dec" => Some(12),
        _ => None,
    }
}

fn ordinal_day(raw: &str) -> Option<u8> {
    let day = raw
        .trim_end_matches("st")
        .trim_end_matches("nd")
        .trim_end_matches("rd")
        .trim_end_matches("th")
        .parse::<u8>()
        .ok()?;
    (1..=31).contains(&day).then_some(day)
}

fn normalize_money_span(s: &str) -> Option<String> {
    let trimmed = s.trim().trim_end_matches(['.', ',', ';', ':']);
    let symbol = trimmed.chars().next()?;
    if !matches!(symbol, '$' | '€' | '£') {
        return None;
    }

    let rest = trimmed[symbol.len_utf8()..]
        .trim()
        .replace(',', "")
        .to_ascii_lowercase();
    let number_len = rest
        .char_indices()
        .take_while(|(_, c)| c.is_ascii_digit() || *c == '.')
        .map(|(idx, c)| idx + c.len_utf8())
        .last()
        .unwrap_or(0);
    if number_len == 0 {
        return None;
    }

    let value: f64 = rest[..number_len].parse().ok()?;
    let suffix = rest[number_len..].trim();
    let multiplier = match suffix {
        "" => 1.0,
        "k" | "thousand" => 1_000.0,
        "m" | "million" => 1_000_000.0,
        "b" | "billion" => 1_000_000_000.0,
        _ => return None,
    };
    Some(format!(
        "{symbol}{}",
        format_scaled_number(value * multiplier)
    ))
}

fn format_scaled_number(value: f64) -> String {
    let mut out = format!("{value:.3}");
    while out.contains('.') && out.ends_with('0') {
        out.pop();
    }
    if out.ends_with('.') {
        out.pop();
    }
    out
}

fn salient_terms(text: &str) -> BTreeSet<String> {
    const STOP: &[&str] = &[
        "the", "a", "an", "and", "or", "to", "of", "in", "on", "for", "with", "by", "is", "are",
        "was", "were", "be", "been", "this", "that", "it", "we", "our", "from", "after", "before",
        "per", "as", "at",
    ];
    let stop: BTreeSet<&str> = STOP.iter().copied().collect();
    Regex::new(r"[A-Za-z][A-Za-z0-9_-]{2,}")
        .unwrap()
        .find_iter(text)
        .map(|m| m.as_str().to_ascii_lowercase())
        .filter(|w| !stop.contains(w.as_str()))
        .collect()
}

#[allow(dead_code)]
fn _is_source_file(path: &Path) -> bool {
    path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use duckdb::params;

    fn span_values(text: &str) -> Vec<String> {
        typed_spans(text).into_iter().map(|s| s.value).collect()
    }

    fn has_span(spans: &BTreeSet<TypedSpan>, value: &str, kind: SpanKind) -> bool {
        spans
            .iter()
            .any(|span| span.value == value && span.kind == kind)
    }

    #[test]
    fn parses_fact_ledger_and_strips_sources() {
        let facts = parse_fact_ledger(
            "- Latency dropped 40%. (source: postmortem)\n* Revenue hit $5M. [2025 report]\n",
        );
        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].text, "Latency dropped 40%.");
        assert_eq!(facts[0].source_ref.as_deref(), Some("source: postmortem"));
        assert!(has_span(&facts[0].spans, "40%", SpanKind::Percent));
        assert_eq!(facts[1].text, "Revenue hit $5M.");
        assert_eq!(facts[1].source_ref.as_deref(), Some("2025 report"));
        assert!(has_span(&facts[1].spans, "$5000000", SpanKind::Money));
    }

    #[test]
    fn unsupported_fact_claim_is_advisory() {
        let candidates = candidate_sentences("Latency dropped 50% after launch.");
        let findings = check_sentences(&candidates, &[], GroundingMode::Facts);
        assert_eq!(findings[0].rule_id, "unsupported-claim");
        assert_eq!(findings[0].severity, "advisory");
    }

    #[test]
    fn source_mode_unsupported_span_is_warning() {
        let candidates = candidate_sentences("Latency dropped 50% after launch.");
        let findings = check_sentences(&candidates, &[], GroundingMode::Source);
        assert_eq!(findings[0].rule_id, "unsupported-claim");
        assert_eq!(findings[0].severity, "warn");
    }

    #[test]
    fn unsupported_claim_anchors_highest_value_span_once() {
        let candidates = candidate_sentences("Revenue reached $5M in 2025 after 12 launches.");
        let findings = check_sentences(&candidates, &[], GroundingMode::Facts);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "unsupported-claim");
        assert_eq!(findings[0].span, "$5000000");
    }

    #[test]
    fn unknown_grounding_mode_is_usage_error() {
        let code = run(FactcheckArgs {
            file: "missing.md".into(),
            source: None,
            kb: false,
            models: false,
            decompose: false,
            claims: None,
            emit_claim_targets: false,
            deep: false,
            ground: Some("semantic".into()),
            threshold: None,
            json: false,
            strict: false,
            quiet: false,
            lookback: None,
        })
        .unwrap();

        assert_eq!(code, 2);
    }

    #[test]
    fn money_suffixes_normalize_to_scaled_values() {
        let compact = typed_spans("Revenue reached $5M.");
        let words = typed_spans("Revenue reached $5 million.");
        assert!(compact
            .iter()
            .any(|span| span.value == "$5000000" && span.kind == SpanKind::Money));
        assert_eq!(compact, words);
    }

    #[test]
    fn percent_and_count_spans_normalize_values() {
        assert_eq!(span_values("Conversion reached 50.0%."), vec!["50%"]);
        assert_eq!(span_values("Conversion reached 50 %."), vec!["50%"]);
        assert_eq!(span_values("Processed 1,000 rows."), vec!["1000"]);
    }

    #[test]
    fn comma_count_fact_supports_plain_count_claim() {
        let candidates = candidate_sentences("Batch processed 1000 rows successfully.");
        let facts = vec![fact_from_text("Batch processed 1,000 rows successfully.")];
        let findings = check_sentences(&candidates, &facts, GroundingMode::Facts);
        assert!(findings.is_empty());
    }

    #[test]
    fn count_conflict_uses_contradicts_fact() {
        let candidates = candidate_sentences("Batch processed 1000 rows successfully.");
        let facts = vec![fact_from_text("Batch processed 2,000 rows successfully.")];
        let findings = check_sentences(&candidates, &facts, GroundingMode::Facts);
        assert_eq!(findings[0].rule_id, "contradicts-fact");
    }

    #[test]
    fn comma_count_keeps_count_kind_after_normalization() {
        let spans = typed_spans("Processed 2,000 rows.");
        assert!(spans
            .iter()
            .any(|span| span.value == "2000" && span.kind == SpanKind::Count));
    }

    #[test]
    fn date_forms_normalize_to_iso_spans() {
        assert_eq!(
            span_values("Shipped on March 15, 2024."),
            vec!["2024-03-15"]
        );
        assert_eq!(
            span_values("Shipped on 15th March 2024."),
            vec!["2024-03-15"]
        );
        assert_eq!(span_values("Shipped in March 2024."), vec!["2024-03"]);
        assert_eq!(span_values("Shipped on 2024-03-15."), vec!["2024-03-15"]);
    }

    #[test]
    fn coarser_date_fact_supports_specific_date_claim() {
        let candidates = candidate_sentences("The launch happened on March 15, 2024.");
        let facts = vec![fact_from_text("The launch happened in March 2024.")];
        let findings = check_sentences(&candidates, &facts, GroundingMode::Facts);
        assert!(findings.is_empty());
    }

    #[test]
    fn incompatible_dates_use_number_date_mismatch() {
        let candidates = candidate_sentences("The launch happened publicly on March 15, 2024.");
        let facts = vec![fact_from_text(
            "The launch happened publicly in April 2024.",
        )];
        let findings = check_sentences(&candidates, &facts, GroundingMode::Facts);
        assert_eq!(findings[0].rule_id, "number-date-mismatch");
    }

    #[test]
    fn exact_span_requires_relevant_fact_support() {
        let candidates = candidate_sentences("Latency dropped 50% after launch.");
        let facts = vec![fact_from_text("Pricing changed 50% for renewals.")];
        let findings = check_sentences(&candidates, &facts, GroundingMode::Facts);
        assert_eq!(findings[0].rule_id, "unsupported-claim");
    }

    #[test]
    fn relevant_entity_boost_can_make_fact_conflict() {
        let candidates = candidate_sentences("API latency was 50%.");
        let facts = vec![fact_from_text("API latency was 40%.")];
        let findings = check_sentences(&candidates, &facts, GroundingMode::Facts);
        assert_eq!(findings[0].rule_id, "contradicts-fact");
    }

    #[test]
    fn emit_claim_targets_include_content_claims_without_typed_spans() {
        let targets = claim_target_sentences(
            "The rollout simplified regional billing for enterprise admins. Tiny ok.",
        );

        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].sentence,
            "The rollout simplified regional billing for enterprise admins."
        );
        assert!(targets[0].spans.is_empty());
    }

    #[test]
    fn supplied_claims_align_to_parent_sentences_by_index() {
        let aligned = align_claims_to_parents(
            "The launch note says latency and pricing changed in one paragraph.",
            "Latency dropped 50% after launch.",
        );

        assert_eq!(aligned.len(), 1);
        assert_eq!(
            aligned[0].sentence,
            "The launch note says latency and pricing changed in one paragraph."
        );
        assert_eq!(
            aligned[0].claim.as_deref(),
            Some("Latency dropped 50% after launch.")
        );
        assert_eq!(span_values(aligned[0].analysis_text()), vec!["50%"]);
    }

    #[test]
    fn decomposed_findings_anchor_parent_and_carry_atomic_claim() {
        let candidates = align_claims_to_parents(
            "The launch note says latency and pricing changed in one paragraph.",
            "Latency dropped 50% after launch.",
        );
        let facts = vec![fact_from_text("Latency dropped 40% after launch.")];
        let findings = check_sentences(&candidates, &facts, GroundingMode::Facts);

        assert_eq!(
            findings[0].sentence,
            "The launch note says latency and pricing changed in one paragraph."
        );
        assert!(findings[0]
            .message
            .contains("atomic claim: Latency dropped 50% after launch."));
    }

    #[test]
    fn duplicate_decomposed_sibling_findings_dedupe() {
        let parent = "The launch note says latency changed in one paragraph.".to_string();
        let claim = "Latency dropped 50% after launch.".to_string();
        let candidates = vec![
            Candidate {
                sentence: parent.clone(),
                spans: typed_spans(&claim),
                claim: Some(claim.clone()),
            },
            Candidate {
                sentence: parent,
                spans: typed_spans(&claim),
                claim: Some(claim),
            },
        ];
        let findings = check_sentences(&candidates, &[], GroundingMode::Facts);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "unsupported-claim");
    }

    #[test]
    fn deterministic_candidates_still_require_typed_spans() {
        let candidates = candidate_sentences(
            "The rollout simplified regional billing for enterprise admins. Latency dropped 50%.",
        );

        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].sentence.contains("Latency dropped 50%"));
    }

    #[test]
    fn deterministic_candidates_require_content_or_entity_context() {
        let candidates = candidate_sentences("The marginal 2024. Latency dropped 50%.");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].sentence, "Latency dropped 50%.");
    }

    #[test]
    fn checkable_sentences_allow_entity_context() {
        let acronym_spans = typed_spans("API and 2024.");
        let digit_spans = typed_spans("Q3 and 2024.");

        assert!(is_checkable_sentence("API and 2024.", &acronym_spans));
        assert!(is_checkable_sentence("Q3 and 2024.", &digit_spans));
    }

    #[test]
    fn sentence_initial_lone_capitalized_word_is_not_entity() {
        assert!(entities("Latency 2024.").is_empty());
        assert!(entities("API 2024.").contains("API"));
        assert!(entities("Q3 2024.").contains("Q3"));
    }

    #[test]
    fn percent_conflict_uses_contradicts_fact() {
        let candidates = candidate_sentences("Latency dropped 50% after launch.");
        let facts = vec![fact_from_text("Latency dropped 40% after launch.")];
        let findings = check_sentences(&candidates, &facts, GroundingMode::Facts);
        assert_eq!(findings[0].rule_id, "contradicts-fact");
        assert_eq!(findings[0].severity, "error");
    }

    #[test]
    fn year_conflict_uses_number_date_mismatch() {
        let candidates = candidate_sentences("The program launched publicly in 2025.");
        let facts = vec![fact_from_text("The program launched publicly in 2024.")];
        let findings = check_sentences(&candidates, &facts, GroundingMode::Facts);
        assert_eq!(findings[0].rule_id, "number-date-mismatch");
        assert_eq!(findings[0].severity, "error");
    }

    #[test]
    fn nearby_fact_must_share_span_kind_to_conflict() {
        let candidates = candidate_sentences("Latency dropped 50% after launch.");
        let facts = vec![fact_from_text("Latency dropped in 2024 after launch.")];
        let findings = check_sentences(&candidates, &facts, GroundingMode::Facts);
        assert_eq!(findings[0].rule_id, "unsupported-claim");
        assert_eq!(findings[0].severity, "advisory");
    }

    #[test]
    fn stale_kb_fact_cannot_support_claim() {
        let candidates = candidate_sentences("Latency dropped 50% after launch.");
        let facts = vec![kb_fact_from_text(
            "Latency dropped 50% after launch.",
            "git:old.md".into(),
            Some("stale".into()),
        )];
        let findings = check_sentences(&candidates, &facts, GroundingMode::Kb);
        assert_eq!(findings[0].rule_id, "unsupported-claim");
        assert!(findings[0].message.contains("stale source git:old.md"));
    }

    #[test]
    fn kb_facts_load_from_multiple_catalogs_and_doc_ref_tags() {
        let dir = tempfile::tempdir().unwrap();
        let local = dir.path().join("local").join("catalog.duckdb");
        let global = dir.path().join("global").join("catalog.duckdb");
        for (path, doc_id, canonical_ref, status, body) in [
            (
                &local,
                "doc-local",
                "git:docs/local.md",
                "canonical",
                "Latency dropped 50% after launch.",
            ),
            (
                &global,
                "doc-global",
                "slack:C123/1",
                "stale",
                "Pricing changed 20% in 2025.",
            ),
        ] {
            let conn = duckdb::Connection::open_in_memory().unwrap();
            index::ensure_schema(&conn).unwrap();
            conn.execute(
                "INSERT INTO documents (doc_id, source_id, external_id, canonical_ref, title, url, path, mime_type, kind, author_id, author_name, created_at, updated_at, observed_at, version, content_sha256, body, metadata_json)
                 VALUES (?1, 'test', ?2, ?3, ?3, NULL, ?2, 'text/markdown', 'file', NULL, NULL, NULL, NULL, 'now', 'v', 'sha', ?4, '{}')",
                params![doc_id, canonical_ref, canonical_ref, body],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO tags (target_type, target_id, status, note, \"by\", \"at\", metadata_json)
                 VALUES ('doc', ?1, ?2, '', 'test', 'now', '{}')",
                params![canonical_ref, status],
            )
            .unwrap();
            index::publish_to_path(&conn, path).unwrap();
        }

        let facts = load_kb_facts_from_paths(&[local, global]).unwrap();
        assert!(facts
            .iter()
            .any(|f| f.source_ref.as_deref() == Some("git:docs/local.md")
                && f.can_support
                && has_span(&f.spans, "50%", SpanKind::Percent)));
        assert!(facts
            .iter()
            .any(|f| f.source_ref.as_deref() == Some("slack:C123/1")
                && !f.can_support
                && f.source_status.as_deref() == Some("stale")
                && has_span(&f.spans, "20%", SpanKind::Percent)));
    }
}
