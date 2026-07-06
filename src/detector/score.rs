//! Slop score per SPEC §12 — explainable 0–100, deterministic, with an
//! optional machine-likelihood blend (absent in this build).

use super::{Family, Finding, Severity};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
pub struct Score {
    pub score: u32,
    pub band: &'static str,
    pub words: usize,
    #[serde(rename = "findingCount")]
    pub finding_count: usize,
    pub per1k: f64,
    #[serde(rename = "byFamily")]
    pub by_family: BTreeMap<String, usize>,
    pub contractions: usize,
    #[serde(rename = "firstPerson")]
    pub first_person: usize,
    pub discount: f64,
    #[serde(rename = "machineLikelihood", skip_serializing_if = "Option::is_none")]
    pub machine_likelihood: Option<f64>,
}

fn fam_weight(f: Family) -> f64 {
    match f {
        Family::AiSlop | Family::Grounding => 1.0,
        Family::Inclusive => 0.5,
        Family::Clarity => 0.4,
        Family::Style | Family::Grammar => 0.3,
    }
}

fn sev_weight(s: Severity) -> f64 {
    match s {
        Severity::Error => 3.0,
        Severity::Warn => 2.0,
        Severity::Advisory => 1.0,
    }
}

pub fn compute(text: &str, findings: &[Finding], machine: Option<f64>) -> Score {
    let words = super::ctx::count_words(text).max(1);
    let mass: f64 = findings
        .iter()
        .map(|f| sev_weight(f.severity) * fam_weight(f.family))
        .sum();
    let per1k = mass / words as f64 * 1000.0;
    let base = 100.0 * (1.0 - (-per1k / 35.0).exp());

    // Human-signal discount.
    let contraction_re =
        regex::Regex::new(r"\w+['’](t|s|re|ve|ll|d|m)\b").unwrap();
    let contractions = contraction_re.find_iter(text).count();
    let fp_re = regex::Regex::new(
        r"\b(I|I'm|I've|I'll|I'd|[Ww]e|[Ww]e're|[Ww]e've|[Ww]e'll|[Ww]e'd|[Mm]y|[Oo]ur|[Mm]e|[Uu]s)\b",
    )
    .unwrap();
    // Bare `I` must be case-sensitive so list markers and math `i` don't count;
    // the regex above is already case-sensitive for `I` forms.
    let first_person = fp_re.find_iter(text).count();
    let discount = ((contractions + first_person) as f64 / words as f64 * 1000.0 * 1.5).min(15.0);

    let deterministic = (base - discount).max(0.0);
    let blended = match machine {
        Some(m) => 0.8 * deterministic + 0.2 * (m * 100.0),
        None => deterministic,
    };
    let score = blended.round().clamp(0.0, 100.0) as u32;
    let band = match score {
        0..=11 => "clean",
        12..=29 => "light",
        30..=59 => "moderate",
        _ => "heavy",
    };
    let mut by_family: BTreeMap<String, usize> = BTreeMap::new();
    for f in findings {
        *by_family.entry(f.family.label().to_string()).or_default() += 1;
    }
    Score {
        score,
        band,
        words,
        finding_count: findings.len(),
        per1k,
        by_family,
        contractions,
        first_person,
        discount,
        machine_likelihood: machine,
    }
}
