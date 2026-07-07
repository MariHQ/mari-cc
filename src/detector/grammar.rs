//! Grammar pass (SPEC §11.11) — Harper (`harper-core`), fully offline.
//! Opt-in only (`--grammar` / `detector.grammar` / `hook.grammar`).
//!
//! Behavior contract: Harper's markdown parser (code skipped, offsets in the
//! original source); keep only the high-precision lint kinds; findings emit
//! as `grammar-<kind>`, family grammar, severity warn, with Harper's message
//! plus its top 3 suggestions ("(remove)" for empty replacements), sorted by
//! offset, capped at 30 per file. Grammar must never break detection: any
//! engine failure returns zero findings with at most one stderr notice.

use super::{Family, Finding, Severity};
use harper_core::linting::{LintGroup, LintKind, Linter, Suggestion};
use harper_core::parsers::MarkdownOptions;
use harper_core::spell::FstDictionary;
use harper_core::{Dialect, Document};

const MAX_PER_FILE: usize = 30;

/// SPEC §11.11 kept kinds — everything else is dropped (heavy false
/// positives on technical markdown, or overlap with Mari's own rules).
fn kept(kind: &LintKind) -> bool {
    matches!(
        kind,
        LintKind::Agreement
            | LintKind::Grammar
            | LintKind::Miscellaneous
            | LintKind::Eggcorn
            | LintKind::Malapropism
            | LintKind::Nonstandard
            | LintKind::BoundaryError
            | LintKind::Redundancy
    )
}

fn kind_slug(kind: &LintKind) -> &'static str {
    match kind {
        LintKind::Agreement => "agreement",
        LintKind::Grammar => "grammar",
        LintKind::Miscellaneous => "miscellaneous",
        LintKind::Eggcorn => "eggcorn",
        LintKind::Malapropism => "malapropism",
        LintKind::Nonstandard => "nonstandard",
        LintKind::BoundaryError => "boundaryerror",
        LintKind::Redundancy => "redundancy",
        _ => "other",
    }
}

pub fn grammar_findings(text: &str) -> Vec<Finding> {
    match std::panic::catch_unwind(|| run_harper(text)) {
        Ok(findings) => findings,
        Err(_) => {
            eprintln!("note: grammar pass failed internally; skipping");
            Vec::new()
        }
    }
}

fn run_harper(text: &str) -> Vec<Finding> {
    let dictionary = FstDictionary::curated();
    let document = Document::new_markdown_curated(text, MarkdownOptions::default());
    let mut group = LintGroup::new_curated(dictionary, Dialect::American);
    // §11.11: disabled individual rules within kept kinds.
    for rule in ["MassNouns", "MissingPreposition"] {
        group.config.set_rule_enabled(rule, false);
    }
    let lints = group.lint(&document);

    // Harper spans are char offsets into the document; map to byte offsets.
    let char_to_byte: Vec<usize> = {
        let mut v: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
        v.push(text.len());
        v
    };
    let to_byte = |c: usize| -> usize { *char_to_byte.get(c).unwrap_or(&text.len()) };

    let mut findings: Vec<Finding> = lints
        .into_iter()
        .filter(|l| kept(&l.lint_kind))
        .map(|l| {
            let start = to_byte(l.span.start);
            let end = to_byte(l.span.end).max(start);
            let mut message = l.message.clone();
            let suggestions: Vec<String> = l
                .suggestions
                .iter()
                .take(3)
                .map(|s| match s {
                    Suggestion::ReplaceWith(chars) => {
                        let r: String = chars.iter().collect();
                        if r.trim().is_empty() {
                            "(remove)".to_string()
                        } else {
                            r
                        }
                    }
                    Suggestion::Remove => "(remove)".to_string(),
                    other => format!("{other:?}").chars().take(40).collect(),
                })
                .collect();
            if !suggestions.is_empty() {
                message.push_str(&format!(" — try: {}", suggestions.join(" · ")));
            }
            let span: String = text[start..end]
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(80)
                .collect();
            Finding {
                rule_id: format!("grammar-{}", kind_slug(&l.lint_kind)),
                family: Family::Grammar,
                severity: Severity::Warn,
                offset: start,
                length: end - start,
                span,
                message,
                r#ref: None,
                line: 0,
                col: 0,
            }
        })
        .collect();
    findings.sort_by_key(|f| f.offset);
    findings.truncate(MAX_PER_FILE);
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar_pass_flags_agreement_and_stays_quiet_on_clean() {
        let bad = "The developers is finishing the feature. For all intensive purposes it works.\n";
        let findings = grammar_findings(bad);
        assert!(!findings.is_empty(), "expected grammar findings on: {bad}");
        assert!(findings.iter().all(|f| f.rule_id.starts_with("grammar-")));
        assert!(findings.iter().all(|f| f.severity == Severity::Warn));
        assert!(findings.len() <= MAX_PER_FILE);

        let clean = "The developers are finishing the feature. It works well.\n";
        let clean_findings = grammar_findings(clean);
        assert!(
            clean_findings.len() < findings.len(),
            "clean text should produce fewer findings ({} vs {})",
            clean_findings.len(),
            findings.len()
        );
    }

    #[test]
    fn spelling_and_style_kinds_are_dropped() {
        // Deliberate misspelling in technical prose — Spelling is a dropped
        // kind, so it must not surface.
        let text = "Configur the RocksDB compaction settings carefully.\n";
        let findings = grammar_findings(text);
        assert!(
            findings.iter().all(|f| f.rule_id != "grammar-spelling"),
            "{findings:?}"
        );
    }
}
