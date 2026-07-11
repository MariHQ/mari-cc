//! Detector pipeline per SPEC §5.4 / §11.0.1 / §11.0.6: walk → skip checks →
//! ctx → rules → waivers → sort → render.

use super::{ctx::Ctx, render, score, Emitter, Finding, Severity};
use crate::{config, workspace};
use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct DetectArgs {
    pub paths: Vec<String>,
    pub stdin: bool,
    pub json: bool,
    pub summary: bool,
    pub score: bool,
    pub strict: bool,
    pub quiet: bool,
    pub style: Option<String>,
    pub models: bool,
    pub slop_spans: bool,
    pub grammar: bool,
    pub no_config: bool,
    /// `--strings <dir>`: lint user-facing copy extracted from code (§5.4).
    pub strings: Option<String>,
    /// `--labels`: treat each input line as its own unit (§5.4).
    pub labels: bool,
}

pub struct FileResult {
    pub path: String,
    pub findings: Vec<Finding>,
    pub word_count: usize,
    pub text: String,
}

const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "dist",
    "build",
    ".next",
    "coverage",
    ".mari",
    "testdata",
    "test-data",
    "fixtures",
    "__fixtures__",
    "golden",
    "snapshots",
    "__snapshots__",
    "target",
    "out",
    "vendor",
    "vendored",
    "3rdparty",
    "thirdparty",
    "third_party",
    "third-party",
];

const MD_EXTS: &[&str] = &["md", "markdown", "mdx", "mdc"];

pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| MD_EXTS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn is_generated(path: &Path) -> bool {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_uppercase();
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(
        stem.as_str(),
        "CHANGELOG" | "HISTORY" | "LICENSE" | "NOTICE"
    ) || name == "llms.txt"
}

/// Non-Latin prose skip: ≥25% of letters non-Latin (SPEC §11.0.6).
fn is_non_latin(text: &str) -> bool {
    let mut latin = 0usize;
    let mut non_latin = 0usize;
    for c in text.chars() {
        if c.is_ascii_alphabetic() || matches!(c, 'À'..='ÿ') {
            latin += 1;
        } else if matches!(c,
            '\u{4E00}'..='\u{9FFF}' | '\u{3040}'..='\u{30FF}' |   // CJK, kana
            '\u{0400}'..='\u{04FF}' |                              // Cyrillic
            '\u{0600}'..='\u{06FF}' |                              // Arabic
            '\u{0E00}'..='\u{0E7F}' |                              // Thai
            '\u{AC00}'..='\u{D7AF}'                                // Hangul
        ) {
            non_latin += 1;
        }
    }
    non_latin > 80 && non_latin * 3 > latin
}

/// Data-like: many words with almost no terminal punctuation, or huge lines.
fn is_data_like(text: &str) -> bool {
    if text.lines().any(|l| l.len() >= 2000) {
        return true;
    }
    let words = super::ctx::count_words(text);
    if words < 200 {
        return false;
    }
    let terminals = text.matches(['.', '!', '?']).count();
    terminals * 100 < words // fewer than 1 terminal per 100 words
}

pub struct DetectorSettings {
    pub style_guide: String,
    pub ignore_rules: HashSet<String>,
    pub ignore_files: GlobSet,
    #[allow(dead_code)] // raw globs kept for status/reporting surfaces
    pub ignore_file_globs: Vec<String>,
    pub ignore_values: serde_json::Value,
    /// {path: [[startLine,endLine], …]} — waive findings within line ranges (§4.5).
    pub ignore_spans: serde_json::Value,
    pub zero_tolerance: HashSet<String>,
    pub grammar: bool,
    pub glossary_groups: Vec<Vec<String>>,
    pub reading_grade_target: Option<f64>,
    /// Config-resolved word/phrase lists (`detector.lists`), shared per run.
    pub lists: std::sync::Arc<super::lists::Lists>,
}

pub fn settings(no_config: bool, style_override: Option<&str>) -> DetectorSettings {
    let cfg = if no_config {
        config::defaults()
    } else {
        config::resolve(Some(&workspace::work_root()))
    };
    let det = &cfg["detector"];
    let style = style_override.map(|s| s.to_string()).unwrap_or_else(|| {
        det["styleGuide"]
            .as_str()
            .unwrap_or("microsoft")
            .to_string()
    });
    let ignore_rules: HashSet<String> = det["ignoreRules"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let globs: Vec<String> = det["ignoreFiles"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let mut b = GlobSetBuilder::new();
    for g in &globs {
        if let Ok(glob) = Glob::new(g) {
            b.add(glob);
        }
    }
    let zero: HashSet<String> = det["zeroTolerance"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    DetectorSettings {
        style_guide: style,
        ignore_rules,
        ignore_files: b.build().unwrap_or_else(|_| GlobSet::empty()),
        ignore_file_globs: globs,
        ignore_values: det["ignoreValues"].clone(),
        ignore_spans: det["ignoreSpans"].clone(),
        zero_tolerance: zero,
        grammar: det["grammar"].as_bool().unwrap_or(false),
        glossary_groups: crate::curation::glossary_groups(&workspace::work_root(), &cfg),
        reading_grade_target: None,
        lists: super::lists::Lists::from_config(&det["lists"]),
    }
}

/// Plain settings for unit tests (no config, no waivers).
#[cfg(test)]
pub fn test_settings(style: &str) -> DetectorSettings {
    DetectorSettings {
        style_guide: style.to_string(),
        ignore_rules: HashSet::new(),
        ignore_files: GlobSet::empty(),
        ignore_file_globs: Vec::new(),
        ignore_values: serde_json::json!({}),
        ignore_spans: serde_json::json!({}),
        zero_tolerance: HashSet::new(),
        grammar: false,
        glossary_groups: Vec::new(),
        reading_grade_target: None,
        lists: super::lists::Lists::defaults(),
    }
}

/// True when the repo-relative path OR basename matches an ignoreFiles glob.
pub fn file_ignored(s: &DetectorSettings, rel: &str) -> bool {
    if s.ignore_files.is_match(rel) {
        return true;
    }
    if let Some(base) = Path::new(rel).file_name().and_then(|b| b.to_str()) {
        return s.ignore_files.is_match(base);
    }
    false
}

/// Collect markdown files under the given args (files or trees).
pub fn collect_files(paths: &[String]) -> Vec<PathBuf> {
    collect_files_checked(paths).0
}

/// Like `collect_files`, but also reports paths that don't exist so callers
/// can distinguish "no findings" from "you typed a path that isn't there".
pub fn collect_files_checked(paths: &[String]) -> (Vec<PathBuf>, Vec<String>) {
    let mut out = Vec::new();
    let mut missing = Vec::new();
    let roots: Vec<String> = if paths.is_empty() {
        vec![".".into()]
    } else {
        paths.to_vec()
    };
    for root in roots {
        let p = PathBuf::from(&root);
        if !p.exists() {
            missing.push(root.clone());
            continue;
        }
        if p.is_file() {
            if is_markdown(&p) {
                out.push(p);
            } else {
                eprintln!("note: {} is not markdown — skipped", p.display());
            }
            continue;
        }
        let walker = ignore::WalkBuilder::new(&p)
            .hidden(false)
            .git_ignore(true)
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !(e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                    && SKIP_DIRS.contains(&name.as_ref()))
            })
            .build();
        for entry in walker.flatten() {
            let path = entry.path();
            if path.is_file() && is_markdown(path) {
                out.push(path.to_path_buf());
            }
        }
    }
    out.sort();
    (out, missing)
}

/// Run the detector over one text. The heart of `detect`, `audit`, the hook,
/// and the editorial verbs.
pub fn detect_text(path: &str, text: &str, s: &DetectorSettings) -> FileResult {
    let mut ctx = Ctx::build(path, text, &s.style_guide);
    ctx.glossary_groups = s.glossary_groups.clone();
    ctx.reading_grade_target = s.reading_grade_target;
    ctx.lists = s.lists.clone();
    let mut em = Emitter::new(s.zero_tolerance.clone());
    let active_pack = s.style_guide.as_str();
    for rule in super::registry() {
        if s.ignore_rules.contains(rule.id) {
            continue;
        }
        match rule.pack {
            Some(p) if p != active_pack => continue,
            _ => {}
        }
        (rule.run)(&ctx, &mut em);
    }
    // ignoreSpans: {path: [[startLine,endLine], …]} — waive findings whose line
    // falls within any listed range for this file (§4.5). Config-only, finer
    // than ignoreFiles; lets a file that deliberately demonstrates slop waive
    // just those spans while genuine violations elsewhere stay visible.
    let waived_ranges: Vec<(usize, usize)> = s
        .ignore_spans
        .get(path)
        .and_then(|v| v.as_array())
        .map(|ranges| {
            ranges
                .iter()
                .filter_map(|r| {
                    let pair = r.as_array()?;
                    let start = pair.first()?.as_u64()? as usize;
                    let end = pair.get(1).and_then(|v| v.as_u64()).unwrap_or(start as u64) as usize;
                    Some((start.min(end), start.max(end)))
                })
                .collect()
        })
        .unwrap_or_default();
    // ignoreValues: {ruleId: [exact span values to waive]}
    let mut findings: Vec<Finding> = em
        .findings
        .into_iter()
        .filter(|f| {
            if let Some(vals) = s.ignore_values.get(&f.rule_id).and_then(|v| v.as_array()) {
                !vals.iter().any(|v| v.as_str() == Some(f.span.as_str()))
            } else {
                true
            }
        })
        .filter(|f| {
            !waived_ranges
                .iter()
                .any(|(lo, hi)| f.line >= *lo && f.line <= *hi)
        })
        .collect();
    findings.sort_by_key(|f| (f.offset, f.rule_id.clone()));
    FileResult {
        path: path.to_string(),
        findings,
        word_count: ctx.word_count,
        text: text.to_string(),
    }
}

/// Run the detector over a set of independent units (a `(source_line, text)`
/// each), remapping every finding's line back to its source line. This backs
/// both `--labels` (one line per unit) and `--strings` (one extracted copy
/// string per unit), so a whole-document rule like `long-sentence` evaluates
/// per unit instead of across unrelated units.
pub fn detect_units(
    path: &str,
    full_text: &str,
    units: &[(usize, String)],
    s: &DetectorSettings,
) -> FileResult {
    let mut findings = Vec::new();
    let mut words = 0usize;
    for (line, unit) in units {
        let r = detect_text(path, unit, s);
        words += r.word_count;
        for mut f in r.findings {
            // detect_text located findings within the unit; shift onto the
            // source line the unit started at (multi-line units add the rest).
            f.line = line + f.line.saturating_sub(1);
            findings.push(f);
        }
    }
    findings.sort_by(|a, b| (a.line, a.col, &a.rule_id).cmp(&(b.line, b.col, &b.rule_id)));
    FileResult {
        path: path.to_string(),
        findings,
        word_count: words,
        text: full_text.to_string(),
    }
}

/// `--strings <dir>`: one `FileResult` per source file whose extracted copy
/// produced findings, with lines pointing back into the real source.
pub fn run_strings(dir: &str, s: &DetectorSettings) -> Vec<FileResult> {
    let root = workspace::work_root();
    let start = {
        let p = PathBuf::from(dir);
        if p.is_absolute() {
            p
        } else {
            root.join(p)
        }
    };
    super::strings::extract_dir(&root, &start)
        .into_iter()
        .map(|(rel, raw, copies)| {
            let units: Vec<(usize, String)> =
                copies.into_iter().map(|c| (c.line, c.text)).collect();
            detect_units(&rel, &raw, &units, s)
        })
        .collect()
}

/// Why an explicitly-named markdown file produced no findings — so a file that
/// scanned as zero units reads as an explanation, never silence (§5.4).
pub fn skip_reason(
    s: &DetectorSettings,
    rel: &str,
    path: &Path,
    text: &str,
) -> Option<&'static str> {
    if file_ignored(s, rel) {
        return Some("waived by detector.ignoreFiles");
    }
    if is_generated(path) {
        return Some("generated file (CHANGELOG/HISTORY/LICENSE/NOTICE/llms.txt)");
    }
    if crate::i18n::is_translation_file(path) {
        return Some("localized translation file");
    }
    if is_non_latin(text) {
        return Some("non-Latin/CJK prose");
    }
    if is_data_like(text) {
        return Some("data-like or link-only (few sentences; e.g. a nav list such as SUMMARY.md)");
    }
    if super::ctx::count_words(text) == 0 {
        return Some("no prose content");
    }
    None
}

/// Split text into `(line_number, line_text)` units for `--labels`, dropping
/// blank lines.
fn line_units(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(idx, l)| (idx + 1, l.to_string()))
        .collect()
}

/// For each explicitly-named markdown file that will be skipped, print why.
fn report_skips(paths: &[String], s: &DetectorSettings) {
    let root = workspace::work_root();
    for arg in paths {
        let p = Path::new(arg);
        if !p.is_file() || !is_markdown(p) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(p) else {
            continue;
        };
        let rel = p
            .strip_prefix(&root)
            .unwrap_or(p)
            .to_string_lossy()
            .to_string();
        if let Some(reason) = skip_reason(s, &rel, p, &text) {
            eprintln!("note: {arg} scanned as zero units — {reason}");
        }
    }
}

/// Should this file be skipped at file level (after markdown check)?
pub fn skip_file(path: &Path, text: &str) -> bool {
    is_generated(path)
        || is_non_latin(text)
        || is_data_like(text)
        || crate::i18n::is_translation_file(path)
}

pub fn run_over(paths: &[String], s: &DetectorSettings) -> Vec<FileResult> {
    let files = collect_files(paths);
    let root = workspace::work_root();
    files
        .par_iter()
        .filter_map(|p| {
            let rel = p
                .strip_prefix(&root)
                .unwrap_or(p)
                .to_string_lossy()
                .to_string();
            if file_ignored(s, &rel) {
                return None;
            }
            let text = std::fs::read_to_string(p).ok()?;
            if skip_file(p, &text) {
                return None;
            }
            Some(detect_text(&rel, &text, s))
        })
        .collect()
}

pub fn cmd_detect(args: DetectArgs) -> Result<i32> {
    let s = settings(args.no_config, args.style.as_deref());
    if args.slop_spans {
        eprintln!("note: zero-shot slop-span extraction is not available in this build (--slop-spans ignored)");
    }
    // A named path that doesn't exist is a usage error, not silent "clean" —
    // otherwise a typo'd path reads as a passing file.
    if !args.stdin && !args.paths.is_empty() {
        let (_, missing) = collect_files_checked(&args.paths);
        if !missing.is_empty() {
            for m in &missing {
                eprintln!("✗ path does not exist: {m}");
            }
            if missing.len() == args.paths.len() {
                return Ok(2);
            }
        }
    }
    let mut results: Vec<FileResult> = if let Some(dir) = args.strings.as_deref() {
        run_strings(dir, &s)
    } else if args.stdin {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        if args.labels {
            vec![detect_units("<stdin>", &buf, &line_units(&buf), &s)]
        } else {
            vec![detect_text("<stdin>", &buf, &s)]
        }
    } else if args.labels {
        let root = workspace::work_root();
        collect_files(&args.paths)
            .iter()
            .filter_map(|p| {
                let rel = p
                    .strip_prefix(&root)
                    .unwrap_or(p)
                    .to_string_lossy()
                    .to_string();
                let text = std::fs::read_to_string(p).ok()?;
                Some(detect_units(&rel, &text, &line_units(&text), &s))
            })
            .collect()
    } else {
        // Explain why an explicitly-named markdown file scanned as zero units
        // rather than dropping it silently (§5.4).
        report_skips(&args.paths, &s);
        run_over(&args.paths, &s)
    };
    if args.grammar || s.grammar {
        // Opt-in Harper pass (§11.11); locates findings and merges them in.
        for r in &mut results {
            let mut extra = super::grammar::grammar_findings(&r.text);
            for f in &mut extra {
                let ctx = super::ctx::Ctx::build(&r.path, &r.text, &s.style_guide);
                let (line, col) = ctx.locate(f.offset);
                f.line = line;
                f.col = col;
            }
            r.findings.extend(extra);
            r.findings.sort_by_key(|f| (f.offset, f.rule_id.clone()));
        }
    }
    results.sort_by(|a, b| a.path.cmp(&b.path));

    // Machine-likelihood blend (§12 step 5): compute per-file only when the
    // model tier is requested with the score. One model load reused per file.
    let machine: Vec<Option<f64>> = if args.models && args.score {
        results
            .iter()
            .map(|r| super::super::attn::machine_likelihood(&r.text))
            .collect()
    } else {
        if args.models && !args.score {
            eprintln!(
                "note: --models augments --score; add --score to see the machine-likelihood blend"
            );
        }
        vec![None; results.len()]
    };

    let has_error = results
        .iter()
        .flat_map(|r| &r.findings)
        .any(|f| f.severity == Severity::Error);
    let has_warn = results
        .iter()
        .flat_map(|r| &r.findings)
        .any(|f| f.severity >= Severity::Warn);

    if args.json {
        render::render_json(&results, args.score, &machine);
    } else if args.summary {
        render::render_summary(&results);
    } else {
        render::render_human(&results, args.quiet);
        if args.score {
            for (i, r) in results.iter().enumerate() {
                let sc = score::compute(&r.text, &r.findings, machine.get(i).copied().flatten());
                render::render_score(&r.path, &sc);
            }
        }
    }

    Ok(if has_error || (args.strict && has_warn) {
        1
    } else {
        0
    })
}

/// `mari audit [path]` — human-facing report grouped by family with
/// bad→good example fixes. Report only; no edits.
pub fn audit(args: &[String], json: bool) -> Result<i32> {
    let s = settings(false, None);
    let results = run_over(args, &s);
    if json {
        render::render_json(&results, false, &[]);
    } else {
        render::render_audit(&results);
    }
    Ok(0)
}

#[cfg(test)]
mod self_test {
    use super::*;

    /// SPEC §19: the deliberate-slop fixture must produce a known finding set.
    #[test]
    fn sloppy_fixture_fires_known_rules() {
        let text = include_str!("../../fixtures/sloppy.md");
        let s = test_settings("microsoft");
        let r = detect_text("fixtures/sloppy.md", text, &s);
        let ids: std::collections::HashSet<&str> =
            r.findings.iter().map(|f| f.rule_id.as_str()).collect();
        for expected in [
            "cliche-opener",
            "filler-phrase",
            "marketing-buzzword",
            "overused-word",
            "assistant-meta",
            "sycophancy",
            "conclusion-restate",
            "vague-attribution",
            "hedge-overuse",
            "wordy-phrase",
            "redundant-pair",
            "vague-link-text",
            "tracking-param-in-citation",
        ] {
            assert!(
                ids.contains(expected),
                "expected {expected} to fire; got {ids:?}"
            );
        }
        // And the clean scaffold must stay clean.
        let clean = detect_text(
            "clean.md",
            "# Notes\n\nWe shipped the fix today. Tests pass.\n",
            &s,
        );
        assert!(
            clean.findings.is_empty(),
            "clean text produced findings: {:?}",
            clean.findings
        );
    }

    /// §4.5 ignoreSpans: a finding whose line is inside a waived range for that
    /// path is dropped; the same finding elsewhere in the file survives.
    #[test]
    fn ignore_spans_waives_findings_by_line_range() {
        let text = "# Notes\nWe utilize the tool.\nWe utilize the tool.\n";
        let mut s = test_settings("microsoft");
        // Baseline: both "utilize" lines fire complex-word.
        let base = detect_text("docs/x.md", text, &s);
        let complex = |r: &FileResult| {
            r.findings
                .iter()
                .filter(|f| f.rule_id == "complex-word")
                .count()
        };
        assert_eq!(
            complex(&base),
            2,
            "expected two complex-word hits, got {:?}",
            base.findings
        );

        // Waive line 2 for this path only.
        s.ignore_spans = serde_json::json!({ "docs/x.md": [[2, 2]] });
        let waived = detect_text("docs/x.md", text, &s);
        assert_eq!(
            complex(&waived),
            1,
            "line-2 finding should be waived; got {:?}",
            waived.findings
        );
        assert!(waived.findings.iter().all(|f| f.line != 2));

        // A different path is unaffected by the waiver.
        let other = detect_text("docs/y.md", text, &s);
        assert_eq!(complex(&other), 2);
    }

    /// §5.4 detect_units remaps each finding onto the source line the unit
    /// started at, so `--strings`/`--labels` report real locations.
    #[test]
    fn detect_units_remap_findings_to_source_lines() {
        let s = test_settings("microsoft");
        let units = vec![
            (5, "We utilize the parser.".to_string()),
            (9, "Please utilize it again.".to_string()),
        ];
        let r = detect_units("src/App.tsx", "raw source text", &units, &s);
        let lines: Vec<usize> = r
            .findings
            .iter()
            .filter(|f| f.rule_id == "complex-word")
            .map(|f| f.line)
            .collect();
        assert_eq!(lines, vec![5, 9], "got {:?}", r.findings);
        assert_eq!(r.text, "raw source text");
    }
}
