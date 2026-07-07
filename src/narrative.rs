//! Whole-document narrative questionnaire (SPEC §13.3).

use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
struct Question {
    id: &'static str,
    label: &'static str,
    prompt: &'static str,
    registers: &'static [&'static str],
}

#[derive(Debug, Clone, Serialize)]
struct DimensionScore {
    id: &'static str,
    label: &'static str,
    score: u8,
    evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct NarrativeReport {
    file: String,
    words: usize,
    total: u8,
    max: u8,
    band: &'static str,
    dimensions: Vec<DimensionScore>,
}

pub fn run(action: Option<&str>, file: Option<&str>, json: bool) -> Result<i32> {
    match action.unwrap_or("questions") {
        "questions" => print_questions(json),
        "score" => {
            let Some(file) = file else {
                eprintln!("usage: mari narrative score <file> [--json]");
                return Ok(2);
            };
            let text = match std::fs::read_to_string(file) {
                Ok(text) => text,
                Err(err) => {
                    eprintln!("✗ failed to read narrative target {file}: {err}");
                    return Ok(1);
                }
            };
            print_score(file, &text, json)
        }
        other => {
            eprintln!("unknown narrative action `{other}`; expected questions | score");
            Ok(2)
        }
    }
}

fn questions() -> Vec<Question> {
    vec![
        Question {
            id: "stated-morals",
            label: "stated morals",
            prompt: "Does the document explain its lesson instead of letting evidence carry the point?",
            registers: &["docs", "marketing", "editorial", "microcopy"],
        },
        Question {
            id: "tidy-structure",
            label: "tidy structure",
            prompt: "Does the shape feel over-balanced, with every section carrying the same weight?",
            registers: &["marketing", "editorial"],
        },
        Question {
            id: "machine-parallelism",
            label: "machine parallelism",
            prompt: "Do adjacent headings, bullets, or sentences repeat the same grammatical frame?",
            registers: &["docs", "marketing", "editorial", "microcopy"],
        },
        Question {
            id: "performed-embodiment",
            label: "performed embodiment",
            prompt: "Does the text claim lived presence without concrete observed detail?",
            registers: &["marketing", "editorial"],
        },
        Question {
            id: "vague-allusion",
            label: "vague allusion",
            prompt: "Does the prose lean on unspecific references instead of named objects, actors, or constraints?",
            registers: &["docs", "marketing", "editorial", "microcopy"],
        },
        Question {
            id: "no-concession-no-reader",
            label: "no concession / no reader",
            prompt: "Does it avoid tradeoffs, limitations, objections, or reader context?",
            registers: &["marketing", "editorial"],
        },
        Question {
            id: "flat-time",
            label: "flat time",
            prompt: "Does everything happen in a timeless present with no sequence, before/after, or revision?",
            registers: &["marketing", "editorial"],
        },
    ]
}

fn print_questions(json: bool) -> Result<i32> {
    let questions = questions();
    if json {
        println!("{}", serde_json::to_string_pretty(&questions)?);
    } else {
        for q in questions {
            println!("{}: {}", q.id, q.prompt);
        }
    }
    Ok(0)
}

fn print_score(file: &str, text: &str, json: bool) -> Result<i32> {
    let report = score(file, text);
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "{}: {}/{} narrative risk ({})",
            report.file, report.total, report.max, report.band
        );
        for dim in &report.dimensions {
            let evidence = if dim.evidence.is_empty() {
                "no direct evidence".to_string()
            } else {
                dim.evidence.join("; ")
            };
            println!("- {}: {}/5 — {}", dim.label, dim.score, evidence);
        }
    }
    Ok(0)
}

fn score(file: &str, text: &str) -> NarrativeReport {
    let words = word_count(text);
    let paragraphs = paragraphs(text);
    let dimensions = vec![
        count_dimension(
            "stated-morals",
            "stated morals",
            text,
            &[
                "the key takeaway",
                "the takeaway",
                "the lesson",
                "ultimately",
                "in conclusion",
                "it is important to",
                "remember that",
            ],
        ),
        tidy_structure(&paragraphs),
        machine_parallelism(text),
        count_dimension(
            "performed-embodiment",
            "performed embodiment",
            text,
            &[
                "i've seen",
                "i have seen",
                "in my experience",
                "as someone who",
                "i know firsthand",
                "we've all",
            ],
        ),
        count_dimension(
            "vague-allusion",
            "vague allusion",
            text,
            &[
                "various",
                "a variety of",
                "many ways",
                "things",
                "stuff",
                "somewhere",
                "someone",
                "some people",
                "certain",
            ],
        ),
        absence_dimension(
            "no-concession-no-reader",
            "no concession / no reader",
            text,
            words,
            &[
                "but",
                "however",
                "although",
                "except",
                "unless",
                "tradeoff",
                "trade-off",
                "limitation",
                "risk",
                "reader",
                "user",
            ],
        ),
        absence_dimension(
            "flat-time",
            "flat time",
            text,
            words,
            &[
                "when",
                "after",
                "before",
                "then",
                "now",
                "today",
                "later",
                "previously",
                "next",
                "first",
                "finally",
            ],
        ),
    ];
    let total = dimensions.iter().map(|d| d.score).sum();
    let band = match total {
        0..=6 => "low",
        7..=15 => "medium",
        _ => "high",
    };
    NarrativeReport {
        file: file.into(),
        words,
        total,
        max: 35,
        band,
        dimensions,
    }
}

fn count_dimension(
    id: &'static str,
    label: &'static str,
    text: &str,
    needles: &[&str],
) -> DimensionScore {
    let lower = text.to_ascii_lowercase();
    let mut evidence = Vec::new();
    let mut count = 0usize;
    for needle in needles {
        let hits = lower.matches(needle).count();
        if hits > 0 {
            count += hits;
            evidence.push(format!("`{needle}` x{hits}"));
        }
    }
    DimensionScore {
        id,
        label,
        score: count.min(5) as u8,
        evidence,
    }
}

fn tidy_structure(paragraphs: &[String]) -> DimensionScore {
    let lengths: Vec<usize> = paragraphs.iter().map(|p| word_count(p)).collect();
    let comparable = lengths.iter().filter(|n| **n >= 20).count();
    let mut score = 0;
    let mut evidence = Vec::new();
    if comparable >= 4 {
        let min = lengths
            .iter()
            .filter(|n| **n >= 20)
            .min()
            .copied()
            .unwrap_or(0);
        let max = lengths
            .iter()
            .filter(|n| **n >= 20)
            .max()
            .copied()
            .unwrap_or(0);
        if min > 0 && max <= min * 2 {
            score = 3;
            evidence.push(format!(
                "{comparable} substantial paragraphs have similar length"
            ));
        }
    }
    DimensionScore {
        id: "tidy-structure",
        label: "tidy structure",
        score,
        evidence,
    }
}

fn machine_parallelism(text: &str) -> DimensionScore {
    let mut repeats = 0usize;
    let mut last_prefix = String::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let line = line.trim_start_matches([
            '-', '*', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '.', ')', ' ',
        ]);
        let prefix = line
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase();
        if prefix.len() >= 10 && prefix == last_prefix {
            repeats += 1;
        }
        last_prefix = prefix;
    }
    DimensionScore {
        id: "machine-parallelism",
        label: "machine parallelism",
        score: repeats.min(5) as u8,
        evidence: if repeats == 0 {
            Vec::new()
        } else {
            vec![format!("{repeats} adjacent repeated line openings")]
        },
    }
}

fn absence_dimension(
    id: &'static str,
    label: &'static str,
    text: &str,
    words: usize,
    markers: &[&str],
) -> DimensionScore {
    let lower = text.to_ascii_lowercase();
    let present = markers.iter().any(|m| contains_word_or_phrase(&lower, m));
    let score = if words >= 250 && !present { 5 } else { 0 };
    DimensionScore {
        id,
        label,
        score,
        evidence: if score > 0 {
            vec![format!("no marker found across {words} words")]
        } else {
            Vec::new()
        },
    }
}

fn contains_word_or_phrase(text: &str, marker: &str) -> bool {
    if marker.contains(' ') || marker.contains('-') {
        return text.contains(marker);
    }
    text.split(|c: char| !c.is_ascii_alphabetic())
        .any(|part| part == marker)
}

fn paragraphs(text: &str) -> Vec<String> {
    text.split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

fn word_count(text: &str) -> usize {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|s| !s.is_empty())
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn questions_cover_seven_dimensions() {
        assert_eq!(questions().len(), 7);
    }

    #[test]
    fn narrative_usage_errors_return_exit_2() {
        assert_eq!(run(Some("score"), None, false).unwrap(), 2);
        assert_eq!(run(Some("publish"), None, false).unwrap(), 2);
    }

    #[test]
    fn narrative_score_missing_file_returns_runtime_error() {
        assert_eq!(
            run(
                Some("score"),
                Some("definitely-missing-narrative.md"),
                false
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn score_reports_named_evidence() {
        let report = score(
            "draft.md",
            "Ultimately, the key takeaway is that various things matter.\n\nBut users still need details.",
        );
        assert!(report
            .dimensions
            .iter()
            .any(|d| d.id == "stated-morals" && d.score >= 2));
        assert!(report
            .dimensions
            .iter()
            .any(|d| d.id == "vague-allusion" && d.score >= 2));
    }
}
