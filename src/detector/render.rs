//! Output rendering per SPEC §18: colorized on TTY, grouped by family;
//! --json for agents; --summary for large trees.

use super::runner::FileResult;
use super::score::Score;
use super::Severity;
use is_terminal::IsTerminal;
use owo_colors::OwoColorize;
use std::collections::BTreeMap;

fn tty() -> bool {
    std::io::stdout().is_terminal()
}

fn sev_str(sev: Severity, color: bool) -> String {
    if !color {
        return sev.label().to_string();
    }
    match sev {
        Severity::Error => sev.label().red().bold().to_string(),
        Severity::Warn => sev.label().yellow().to_string(),
        Severity::Advisory => sev.label().dimmed().to_string(),
    }
}

pub fn render_human(results: &[FileResult], quiet: bool) {
    let color = tty();
    let mut total = 0usize;
    for r in results {
        if r.findings.is_empty() {
            continue;
        }
        total += r.findings.len();
        if color {
            println!("{}", r.path.bold());
        } else {
            println!("{}", r.path);
        }
        for f in &r.findings {
            println!(
                "  {}:{} {} {} — {}  [{}]",
                f.line,
                f.col,
                sev_str(f.severity, color),
                f.rule_id,
                f.message,
                f.span
            );
        }
        println!();
    }
    if !quiet {
        if total == 0 {
            println!("✓ clean — no findings");
        } else {
            let files = results.iter().filter(|r| !r.findings.is_empty()).count();
            println!("{total} finding(s) in {files} file(s)");
        }
    }
}

pub fn render_json(results: &[FileResult], with_score: bool, machine: &[Option<f64>]) {
    let files: Vec<serde_json::Value> = results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut v = serde_json::json!({
                "path": r.path,
                "words": r.word_count,
                "findings": r.findings,
            });
            if with_score {
                let m = machine.get(i).copied().flatten();
                let sc = super::score::compute(&r.text, &r.findings, m);
                v["score"] = serde_json::to_value(&sc).unwrap();
            }
            v
        })
        .collect();
    let summary = serde_json::json!({
        "files": results.len(),
        "findings": results.iter().map(|r| r.findings.len()).sum::<usize>(),
        "errors": count_sev(results, Severity::Error),
        "warns": count_sev(results, Severity::Warn),
        "advisories": count_sev(results, Severity::Advisory),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "files": files,
            "summary": summary
        }))
        .unwrap()
    );
}

fn count_sev(results: &[FileResult], sev: Severity) -> usize {
    results
        .iter()
        .flat_map(|r| &r.findings)
        .filter(|f| f.severity == sev)
        .count()
}

pub fn render_summary(results: &[FileResult]) {
    let mut worst: Vec<(&str, usize)> = results
        .iter()
        .filter(|r| !r.findings.is_empty())
        .map(|r| (r.path.as_str(), r.findings.len()))
        .collect();
    worst.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    println!("Worst files:");
    for (p, n) in worst.iter().take(15) {
        println!("  {n:>4}  {p}");
    }
    let mut hist: BTreeMap<String, usize> = BTreeMap::new();
    for f in results.iter().flat_map(|r| &r.findings) {
        *hist.entry(f.rule_id.clone()).or_default() += 1;
    }
    let mut hist: Vec<_> = hist.into_iter().collect();
    hist.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    println!("\nRule histogram:");
    for (rule, n) in hist {
        println!("  {n:>4}  {rule}");
    }
}

pub fn render_score(path: &str, sc: &Score) {
    println!(
        "{path}: slop score {} ({}) — {} findings / {} words, density {:.1}/1k, discount {:.1}",
        sc.score, sc.band, sc.finding_count, sc.words, sc.per1k, sc.discount
    );
}

/// `mari audit`: grouped by family, each finding paired with a bad→good
/// example fix (SPEC §5.4).
pub fn render_audit(results: &[FileResult]) {
    let color = tty();
    for r in results {
        if r.findings.is_empty() {
            continue;
        }
        if color {
            println!("{}", r.path.bold().underline());
        } else {
            println!("== {} ==", r.path);
        }
        let mut by_family: BTreeMap<&str, Vec<&super::Finding>> = BTreeMap::new();
        for f in &r.findings {
            by_family.entry(f.family.label()).or_default().push(f);
        }
        for (family, findings) in by_family {
            println!("\n  [{family}]");
            for f in findings {
                println!(
                    "  {}:{} {} {} — {}",
                    f.line,
                    f.col,
                    sev_str(f.severity, color),
                    f.rule_id,
                    f.message
                );
                println!("      bad:  {}", f.span);
                if let Some(fix) = example_fix(&f.rule_id, &f.message) {
                    println!("      good: {fix}");
                }
            }
        }
        println!();
    }
    if results.iter().all(|r| r.findings.is_empty()) {
        println!("✓ clean — no findings");
    }
}

/// Derive a "good" example from map-rule messages ("… use 'X'"), else None.
fn example_fix(_rule_id: &str, message: &str) -> Option<String> {
    // Map rules embed the replacement in quotes in the message.
    let re = regex::Regex::new(r"[\u{2018}\u{2019}'\u{201C}\u{201D}\x22]([^'\u{2018}\u{2019}\u{201C}\u{201D}\x22]{1,60})['\u{2018}\u{2019}\u{201C}\u{201D}\x22]\s*$").ok()?;
    re.captures(message.trim_end_matches('.'))
        .map(|c| c[1].to_string())
}
