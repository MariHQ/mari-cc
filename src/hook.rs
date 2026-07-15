//! Post-edit hook. It reports findings and maintenance notices without editing files.

use crate::{config, detector, i18n, workspace};
use globset::Glob;
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};

pub fn run(args: &[String]) -> i32 {
    if let Err(error) = run_inner(args) {
        if std::env::var("MARI_HOOK_DEBUG").is_ok() {
            eprintln!("mari hook internal error: {error:#}");
        }
    }
    0
}

fn run_inner(args: &[String]) -> anyhow::Result<()> {
    let root = workspace::work_root();
    let cfg = config::resolve(Some(&root));
    if cfg["hook"]["quiet"].as_bool().unwrap_or(false) {
        return Ok(());
    }
    let mut settings = detector::runner::settings(false, None);
    if cfg["hook"]["grammar"].as_bool().unwrap_or(false) {
        settings.grammar = true;
    }
    let max = cfg["hook"]["maxFindings"].as_u64().unwrap_or(20) as usize;
    for file in edited_files(args, &root)? {
        if !file.exists() {
            continue;
        }
        prose_lint(&root, &file, &settings, max);
        i18n_notice(&root, &file);
        rule_notices(&root, &cfg, &file);
        nudge_notices(&root, &cfg, &file);
    }
    Ok(())
}

fn edited_files(args: &[String], root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut paths = BTreeSet::new();
    for arg in args.iter().filter(|arg| arg.as_str() != "run") {
        paths.insert(resolve(root, arg));
    }
    let mut stdin = String::new();
    let _ = std::io::stdin().read_to_string(&mut stdin);
    if let Ok(value) = serde_json::from_str::<Value>(&stdin) {
        collect_paths(&value, &mut paths, root);
    }
    Ok(paths.into_iter().collect())
}

fn collect_paths(value: &Value, paths: &mut BTreeSet<PathBuf>, root: &Path) {
    match value {
        Value::Object(map) => {
            for key in ["file_path", "filePath", "path", "uri"] {
                if let Some(raw) = map
                    .get(key)
                    .and_then(Value::as_str)
                    .filter(|s| looks_like_path(s))
                {
                    paths.insert(resolve(root, raw.strip_prefix("file://").unwrap_or(raw)));
                }
            }
            for child in map.values() {
                collect_paths(child, paths, root);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_paths(child, paths, root);
            }
        }
        Value::String(raw) if looks_like_path(raw) => {
            paths.insert(resolve(root, raw.strip_prefix("file://").unwrap_or(raw)));
        }
        _ => {}
    }
}

fn prose_lint(root: &Path, file: &Path, settings: &detector::runner::DetectorSettings, max: usize) {
    if !detector::runner::is_markdown(file) {
        return;
    }
    let relative = rel(root, file);
    if detector::runner::file_ignored(settings, &relative) {
        return;
    }
    let Ok(text) = std::fs::read_to_string(file) else {
        return;
    };
    if detector::runner::skip_file(file, &text) {
        return;
    }
    let result = detector::runner::detect_text(&relative, &text, settings);
    for finding in result.findings.iter().take(max) {
        println!(
            "{}:{}:{} {} {}: {}",
            relative,
            finding.line,
            finding.col,
            finding.severity.label(),
            finding.rule_id,
            finding.message
        );
    }
    if result.findings.len() > max {
        println!(
            "{}: {} more finding(s) suppressed by hook.maxFindings",
            relative,
            result.findings.len() - max
        );
    }
}

fn i18n_notice(root: &Path, file: &Path) {
    let siblings = i18n::source_language_siblings(file);
    if siblings.is_empty() {
        return;
    }
    let rendered = siblings
        .iter()
        .map(|path| rel(root, path))
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "i18n: {} changed; check translation sibling(s): {rendered}",
        rel(root, file)
    );
}

fn rule_notices(root: &Path, cfg: &Value, file: &Path) {
    let relative = rel(root, file);
    for rule in cfg["rules"].as_array().into_iter().flatten() {
        if matches_any(&relative, rule["paths"].as_array())
            && !matches_any(&relative, rule["exclude"].as_array())
        {
            if let Some(message) = rule["notify"].as_str() {
                println!(
                    "notify {}: {message}",
                    rule["name"].as_str().unwrap_or("rule")
                );
            }
        }
    }
}

fn nudge_notices(root: &Path, cfg: &Value, file: &Path) {
    let relative = rel(root, file);
    for nudge in cfg["nudges"].as_array().into_iter().flatten() {
        let Some(pattern) = nudge["when"].get("file").and_then(Value::as_str) else {
            continue;
        };
        if !glob_match(pattern, &relative) || matches_any(&relative, nudge["exclude"].as_array()) {
            continue;
        }
        let edits = nudge["edit"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|endpoint| endpoint.get("file").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(", ");
        let message = nudge["message"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| format!(" — {s}"))
            .unwrap_or_default();
        println!(
            "✎ nudge {}: {relative} changed — edit {edits}{message}",
            nudge["name"].as_str().unwrap_or("unnamed")
        );
    }
}

fn matches_any(relative: &str, values: Option<&Vec<Value>>) -> bool {
    values
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .any(|pattern| glob_match(pattern, relative))
}

fn glob_match(pattern: &str, relative: &str) -> bool {
    Glob::new(pattern)
        .map(|glob| glob.compile_matcher().is_match(relative))
        .unwrap_or(false)
}

fn looks_like_path(value: &str) -> bool {
    value.ends_with(".md")
        || value.ends_with(".mdx")
        || value.ends_with(".mdc")
        || value.ends_with(".txt")
        || value.contains('/')
}

fn resolve(root: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
