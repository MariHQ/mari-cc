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
    pub zero_tolerance: HashSet<String>,
    pub grammar: bool,
    pub glossary_groups: Vec<Vec<String>>,
    pub reading_grade_target: Option<f64>,
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
        zero_tolerance: zero,
        grammar: det["grammar"].as_bool().unwrap_or(false),
        glossary_groups: crate::curation::glossary_groups(&workspace::work_root(), &cfg),
        reading_grade_target: None,
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
        zero_tolerance: HashSet::new(),
        grammar: false,
        glossary_groups: Vec::new(),
        reading_grade_target: None,
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
        .collect();
    findings.sort_by_key(|f| (f.offset, f.rule_id.clone()));
    FileResult {
        path: path.to_string(),
        findings,
        word_count: ctx.word_count,
        text: text.to_string(),
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
    let mut results: Vec<FileResult> = if args.stdin {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        vec![detect_text("<stdin>", &buf, &s)]
    } else {
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
}
