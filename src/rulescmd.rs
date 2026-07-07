//! Hooks management, waivers, zero tolerance, edit-notify rules, and nudges
//! (SPEC §5.1/§4.7/§15).

use crate::{config, workspace};
use anyhow::{anyhow, Result};
use regex::Regex;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

pub fn hooks(args: &[String], reason: Option<&str>) -> Result<i32> {
    let root = workspace::work_root();
    match args.first().map(|s| s.as_str()) {
        None | Some("status") => {
            let installed = hook_installed(&root);
            println!(
                "hook: {}",
                if installed {
                    "installed"
                } else {
                    "not installed"
                }
            );
            Ok(0)
        }
        Some("on") => {
            install_claude_hook(&root)?;
            println!("✓ installed Claude Code PostToolUse hook");
            Ok(0)
        }
        Some("off") => {
            uninstall_claude_hook(&root)?;
            println!("✓ disabled Claude Code hook");
            Ok(0)
        }
        Some("reset") => {
            uninstall_claude_hook(&root)?;
            install_claude_hook(&root)?;
            println!("✓ reset Claude Code hook");
            Ok(0)
        }
        // Opt-in commit-association git hook (SPEC §15.2).
        Some("commit-on") => {
            let hook_path = root.join(".git").join("hooks").join("post-commit");
            let Some(parent) = hook_path.parent() else {
                anyhow::bail!("not a git repository");
            };
            if !root.join(".git").exists() {
                eprintln!("✗ not a git repository");
                return Ok(1);
            }
            std::fs::create_dir_all(parent)?;
            std::fs::write(
                &hook_path,
                "#!/bin/sh
# installed by `mari hooks commit-on`
mari hook commit
",
            )?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&hook_path, std::fs::Permissions::from_mode(0o755))?;
            }
            println!(
                "✓ installed post-commit association hook at {}",
                hook_path.display()
            );
            Ok(0)
        }
        Some("commit-off") => {
            let hook_path = root.join(".git").join("hooks").join("post-commit");
            if hook_path.exists()
                && std::fs::read_to_string(&hook_path)
                    .map(|s| s.contains("mari hook commit"))
                    .unwrap_or(false)
            {
                std::fs::remove_file(&hook_path)?;
                println!("✓ removed post-commit association hook");
            } else {
                println!("post-commit hook not installed by mari — nothing removed");
            }
            Ok(0)
        }
        Some("ignore-rule") => {
            let id = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: mari hooks ignore-rule <id> [--reason ...]"))?;
            add_ignore_rule(&root, id, reason)?;
            println!("✓ ignored rule {id}");
            Ok(0)
        }
        Some("ignore-file") => {
            let glob = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: mari hooks ignore-file <glob> [--reason ...]"))?;
            add_ignore_file(&root, glob, reason)?;
            println!("✓ ignored file {glob}");
            Ok(0)
        }
        Some("ignore-value") => {
            let rule = args.get(1).ok_or_else(|| {
                anyhow!("usage: mari hooks ignore-value <rule> <value> [--reason ...]")
            })?;
            let value = args.get(2).ok_or_else(|| {
                anyhow!("usage: mari hooks ignore-value <rule> <value> [--reason ...]")
            })?;
            add_ignore_value(&root, rule, value, reason)?;
            println!("✓ ignored value for {rule}: {value}");
            Ok(0)
        }
        Some(other) => {
            eprintln!("unknown hooks subcommand: {other}");
            Ok(2)
        }
    }
}

pub fn ignores(args: &[String], reason: Option<&str>) -> Result<i32> {
    let root = workspace::work_root();
    match args.first().map(|s| s.as_str()) {
        None | Some("list") => {
            let cfg = config::resolve(Some(&root));
            println!("{}", serde_json::to_string_pretty(&cfg["detector"])?);
            Ok(0)
        }
        Some("add-rule") => {
            let id = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: mari ignores add-rule <id> [--reason ...]"))?;
            add_ignore_rule(&root, id, reason)?;
            println!("✓ ignored rule {id}");
            Ok(0)
        }
        Some("add-file") => {
            let glob = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: mari ignores add-file <glob> [--reason ...]"))?;
            add_ignore_file(&root, glob, reason)?;
            println!("✓ ignored file {glob}");
            Ok(0)
        }
        Some("add-value") => {
            let rule = args.get(1).ok_or_else(|| {
                anyhow!("usage: mari ignores add-value <rule> <value> [--reason ...]")
            })?;
            let value = args.get(2).ok_or_else(|| {
                anyhow!("usage: mari ignores add-value <rule> <value> [--reason ...]")
            })?;
            add_ignore_value(&root, rule, value, reason)?;
            println!("✓ ignored value for {rule}: {value}");
            Ok(0)
        }
        Some(other) => {
            eprintln!("unknown ignores subcommand: {other}");
            Ok(2)
        }
    }
}

pub fn zero(args: &[String]) -> Result<i32> {
    let root = workspace::work_root();
    match args.first().map(|s| s.as_str()) {
        None | Some("list") => {
            for id in string_array(&config::resolve(Some(&root))["detector"]["zeroTolerance"]) {
                println!("{id}");
            }
            Ok(0)
        }
        Some("add") => {
            let id = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: mari zero add <rule-id>"))?;
            mutate_repo(&root, |cfg| {
                push_unique_path(cfg, "detector.zeroTolerance", id)
            })?;
            println!("✓ zero-tolerance: {id}");
            Ok(0)
        }
        Some("remove") => {
            let id = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: mari zero remove <rule-id>"))?;
            mutate_repo(&root, |cfg| {
                remove_from_path(cfg, "detector.zeroTolerance", id)
            })?;
            println!("✓ removed zero-tolerance rule {id}");
            Ok(0)
        }
        Some(other) => {
            eprintln!("unknown zero subcommand: {other}");
            Ok(2)
        }
    }
}

pub fn rules(
    args: &[String],
    json_out: bool,
    write: bool,
    paths: Option<&str>,
    notify: Option<&str>,
    exclude: Option<&str>,
) -> Result<i32> {
    let root = workspace::work_root();
    match args.first().map(|s| s.as_str()) {
        None | Some("list") => print_array(&config::resolve(Some(&root))["rules"], json_out),
        Some("discover") => discover_rules(&root, json_out, write),
        Some("add") => {
            let name = args.get(1).ok_or_else(|| {
                anyhow!("usage: mari rules add <name> --paths <globs> --notify <msg>")
            })?;
            let paths = paths.ok_or_else(|| anyhow!("missing --paths"))?;
            let notify = notify.ok_or_else(|| anyhow!("missing --notify"))?;
            let rule = json!({
                "name": name,
                "paths": split_csv(paths),
                "notify": notify,
                "exclude": exclude.map(split_csv).unwrap_or_default(),
            });
            upsert_named(&root, "rules", name, rule)?;
            println!("✓ added rule {name}");
            Ok(0)
        }
        Some("remove") => {
            let name = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: mari rules remove <name>"))?;
            remove_named(&root, "rules", name)?;
            println!("✓ removed rule {name}");
            Ok(0)
        }
        Some(other) => {
            eprintln!("unknown rules subcommand: {other}");
            Ok(2)
        }
    }
}

pub fn nudge(
    args: &[String],
    json_out: bool,
    when: Option<&str>,
    edit: &[String],
    message: Option<&str>,
    exclude: Option<&str>,
) -> Result<i32> {
    let root = workspace::work_root();
    match args.first().map(|s| s.as_str()) {
        None | Some("list") => print_array(&config::resolve(Some(&root))["nudges"], json_out),
        Some("add") => {
            let name = args.get(1).ok_or_else(|| {
                anyhow!(
                    "usage: mari nudge add <name> --when <glob[#symbol]> --edit <file[#symbol]>..."
                )
            })?;
            let when = when.ok_or_else(|| anyhow!("missing --when"))?;
            if edit.is_empty() {
                return Err(anyhow!("missing --edit"));
            }
            let nudge = json!({
                "name": name,
                "when": endpoint_json(when),
                "edit": edit.iter().map(|e| endpoint_json(e)).collect::<Vec<_>>(),
                "message": message.unwrap_or(""),
                "exclude": exclude.map(split_csv).unwrap_or_default(),
            });
            let errors = validate_nudge(&root, &nudge);
            if !errors.is_empty() {
                for e in errors {
                    eprintln!("✗ {e}");
                }
                return Ok(1);
            }
            upsert_named(&root, "nudges", name, nudge)?;
            println!("✓ added nudge {name}");
            Ok(0)
        }
        Some("remove") => {
            let name = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: mari nudge remove <name>"))?;
            remove_named(&root, "nudges", name)?;
            println!("✓ removed nudge {name}");
            Ok(0)
        }
        Some("check") => check_nudges(&root, json_out),
        Some(other) => {
            eprintln!("unknown nudge subcommand: {other}");
            Ok(2)
        }
    }
}

fn discover_rules(root: &Path, json_out: bool, write: bool) -> Result<i32> {
    let mut proposed = Vec::new();
    if root.join("src").is_dir() && (root.join("docs").is_dir() || root.join("SPEC.md").exists()) {
        proposed.push(json!({
            "name": "code-docs",
            "paths": ["src/**"],
            "notify": "Code changed; check docs, SPEC.md, and examples for drift.",
            "exclude": []
        }));
    }
    if root.join("Cargo.toml").exists() {
        proposed.push(json!({
            "name": "rust-manifest",
            "paths": ["Cargo.toml", "Cargo.lock"],
            "notify": "Rust manifest changed; check install, build, and dependency docs.",
            "exclude": []
        }));
    }
    if json_out {
        println!("{}", serde_json::to_string_pretty(&proposed)?);
    } else if proposed.is_empty() {
        println!("no edit-notify candidates discovered");
    } else {
        for rule in &proposed {
            println!(
                "{}  {}",
                rule["name"].as_str().unwrap_or("?"),
                rule["notify"].as_str().unwrap_or("")
            );
        }
    }
    if write {
        for rule in proposed {
            let name = rule["name"].as_str().unwrap().to_string();
            upsert_named(root, "rules", &name, rule)?;
        }
    }
    Ok(0)
}

fn add_ignore_rule(root: &Path, id: &str, reason: Option<&str>) -> Result<()> {
    mutate_repo(root, |cfg| {
        push_unique_path(cfg, "detector.ignoreRules", id);
        remember_reason(cfg, id, reason);
    })
}

fn add_ignore_file(root: &Path, glob: &str, reason: Option<&str>) -> Result<()> {
    mutate_repo(root, |cfg| {
        push_unique_path(cfg, "detector.ignoreFiles", glob);
        remember_reason(cfg, glob, reason);
    })
}

fn add_ignore_value(root: &Path, rule: &str, value: &str, reason: Option<&str>) -> Result<()> {
    mutate_repo(root, |cfg| {
        let mut vals = config::get_path(cfg, &format!("detector.ignoreValues.{rule}"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let v = Value::String(value.to_string());
        if !vals.contains(&v) {
            vals.push(v);
        }
        config::set_path(
            cfg,
            &format!("detector.ignoreValues.{rule}"),
            Value::Array(vals),
        );
        remember_reason(cfg, &format!("{rule}:{value}"), reason);
    })
}

fn remember_reason(cfg: &mut Value, key: &str, reason: Option<&str>) {
    if let Some(reason) = reason {
        config::set_path(
            cfg,
            &format!("detector.ignoreReasons.{key}"),
            Value::String(reason.to_string()),
        );
    }
}

fn install_claude_hook(root: &Path) -> Result<()> {
    let path = root.join(".claude").join("settings.json");
    let mut settings = config::read_json(&path);
    config::set_path(
        &mut settings,
        "hooks.PostToolUse",
        json!([{
            "matcher": "Edit|Write|MultiEdit",
            "hooks": [{ "type": "command", "command": "mari hook run", "timeout": 10 }]
        }]),
    );
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(path, serde_json::to_string_pretty(&settings)?)?;
    Ok(())
}

fn uninstall_claude_hook(root: &Path) -> Result<()> {
    let path = root.join(".claude").join("settings.json");
    if !path.exists() {
        return Ok(());
    }
    let mut settings = config::read_json(&path);
    if let Some(obj) = settings.as_object_mut() {
        if let Some(hooks) = obj.get_mut("hooks").and_then(|v| v.as_object_mut()) {
            hooks.remove("PostToolUse");
        }
    }
    std::fs::write(path, serde_json::to_string_pretty(&settings)?)?;
    Ok(())
}

fn hook_installed(root: &Path) -> bool {
    std::fs::read_to_string(root.join(".claude").join("settings.json"))
        .map(|s| s.contains("mari hook run"))
        .unwrap_or(false)
}

fn print_array(v: &Value, json_out: bool) -> Result<i32> {
    if json_out {
        println!("{}", serde_json::to_string_pretty(v)?);
    } else if let Some(arr) = v.as_array() {
        for item in arr {
            println!("{}", serde_json::to_string_pretty(item)?);
        }
        if arr.is_empty() {
            println!("none");
        }
    } else {
        println!("none");
    }
    Ok(0)
}

fn mutate_repo(root: &Path, f: impl FnOnce(&mut Value)) -> Result<()> {
    let path = config::repo_config_path(root);
    let mut cfg = config::read_json(&path);
    f(&mut cfg);
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(path, serde_json::to_string_pretty(&cfg)?)?;
    Ok(())
}

fn push_unique_path(cfg: &mut Value, dotted: &str, value: &str) {
    let mut arr = config::get_path(cfg, dotted)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let v = Value::String(value.to_string());
    if !arr.contains(&v) {
        arr.push(v);
    }
    config::set_path(cfg, dotted, Value::Array(arr));
}

fn remove_from_path(cfg: &mut Value, dotted: &str, value: &str) {
    let mut arr = config::get_path(cfg, dotted)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    arr.retain(|v| v.as_str() != Some(value));
    config::set_path(cfg, dotted, Value::Array(arr));
}

fn upsert_named(root: &Path, dotted: &str, name: &str, item: Value) -> Result<()> {
    mutate_repo(root, |cfg| {
        let mut arr = config::get_path(cfg, dotted)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        arr.retain(|v| v["name"].as_str() != Some(name));
        arr.push(item);
        config::set_path(cfg, dotted, Value::Array(arr));
    })
}

fn remove_named(root: &Path, dotted: &str, name: &str) -> Result<()> {
    mutate_repo(root, |cfg| {
        let mut arr = config::get_path(cfg, dotted)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        arr.retain(|v| v["name"].as_str() != Some(name));
        config::set_path(cfg, dotted, Value::Array(arr));
    })
}

fn check_nudges(root: &Path, json_out: bool) -> Result<i32> {
    let cfg = config::resolve(Some(root));
    let mut errors = Vec::new();
    for nudge in cfg["nudges"].as_array().into_iter().flatten() {
        errors.extend(validate_nudge(root, nudge));
    }
    if json_out {
        println!("{}", serde_json::to_string_pretty(&errors)?);
    } else if errors.is_empty() {
        println!("nudges: ok");
    } else {
        for e in &errors {
            println!("✗ {e}");
        }
    }
    Ok(if errors.is_empty() { 0 } else { 1 })
}

fn validate_nudge(root: &Path, nudge: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    if let Some(when) = nudge.get("when") {
        validate_endpoint(root, when, "when", &mut errors);
    }
    for edit in nudge["edit"].as_array().into_iter().flatten() {
        validate_endpoint(root, edit, "edit", &mut errors);
    }
    errors
}

fn validate_endpoint(root: &Path, endpoint: &Value, label: &str, errors: &mut Vec<String>) {
    let file = endpoint["file"].as_str().unwrap_or("");
    if file.contains('*') || file.contains('?') {
        return;
    }
    let path = root.join(file);
    if !path.exists() {
        errors.push(format!("{label} target does not exist: {file}"));
        return;
    }
    if let Some(symbol) = endpoint["symbol"].as_str() {
        if !symbol_exists(&path, symbol) {
            errors.push(format!("{label} symbol does not resolve: {file}#{symbol}"));
        }
    }
}

fn symbol_exists(path: &Path, symbol: &str) -> bool {
    resolve_symbol(path, symbol).is_some()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SymbolSpan {
    start_line: usize,
    end_line: usize,
}

fn resolve_symbol(path: &Path, symbol: &str) -> Option<SymbolSpan> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return None;
    };
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "md" | "mdx" | "mdc" | "markdown" => resolve_markdown_heading(&text, symbol),
        "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" => {
            resolve_code_symbol(&text, &ext, symbol)
        }
        _ => None,
    }
}

fn resolve_markdown_heading(text: &str, symbol: &str) -> Option<SymbolSpan> {
    let target = normalize_symbol(symbol);
    let mut found: Option<(usize, usize)> = None;
    for (idx, line) in text.lines().enumerate() {
        let t = line.trim();
        let level = t.chars().take_while(|c| *c == '#').count();
        if !(1..=6).contains(&level) || t.chars().nth(level) != Some(' ') {
            continue;
        }
        let heading = t[level..].trim().trim_matches('#').trim();
        if let Some((start, start_level)) = found {
            if level <= start_level {
                return Some(SymbolSpan {
                    start_line: start,
                    end_line: idx,
                });
            }
        }
        if normalize_symbol(heading) == target {
            found = Some((idx + 1, level));
        }
    }
    found.map(|(start, _)| SymbolSpan {
        start_line: start,
        end_line: text.lines().count().max(start),
    })
}

fn resolve_code_symbol(text: &str, ext: &str, symbol: &str) -> Option<SymbolSpan> {
    let patterns: &[&str] = match ext {
        "rs" => &[
            r"^\s*pub(?:\([^)]*\))?\s+(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)",
            r"^\s*pub(?:\([^)]*\))?\s+(?:struct|enum|trait|type|const|static|mod)\s+([A-Za-z_][A-Za-z0-9_]*)",
        ],
        "ts" | "tsx" | "js" | "jsx" => &[
            r"^\s*export\s+(?:default\s+)?(?:async\s+)?function\s+([A-Za-z_$][A-Za-z0-9_$]*)",
            r"^\s*export\s+(?:default\s+)?(?:const|let|var|class|interface|type|enum)\s+([A-Za-z_$][A-Za-z0-9_$]*)",
        ],
        "py" => &[
            r"^\s*(?:async\s+)?def\s+([A-Za-z_][A-Za-z0-9_]*)",
            r"^\s*class\s+([A-Za-z_][A-Za-z0-9_]*)",
        ],
        "go" => &[
            r"^\s*func\s+([A-Z][A-Za-z0-9_]*)",
            r"^\s*func\s+\([^)]*\)\s*([A-Z][A-Za-z0-9_]*)",
            r"^\s*type\s+([A-Z][A-Za-z0-9_]*)",
            r"^\s*(?:const|var)\s+([A-Z][A-Za-z0-9_]*)",
        ],
        _ => return None,
    };
    let regexes = patterns
        .iter()
        .map(|p| Regex::new(p).unwrap())
        .collect::<Vec<_>>();
    let lines = text.lines().collect::<Vec<_>>();
    for (idx, line) in lines.iter().enumerate() {
        if regexes.iter().any(|re| {
            re.captures(line)
                .and_then(|caps| caps.get(1))
                .map(|m| m.as_str() == symbol)
                .unwrap_or(false)
        }) {
            return Some(SymbolSpan {
                start_line: idx + 1,
                end_line: code_symbol_end(&lines, idx),
            });
        }
    }
    None
}

fn code_symbol_end(lines: &[&str], start_idx: usize) -> usize {
    let start_indent = leading_spaces(lines[start_idx]);
    for (idx, line) in lines.iter().enumerate().skip(start_idx + 1) {
        if line.trim().is_empty() {
            continue;
        }
        if leading_spaces(line) <= start_indent && looks_like_top_level_definition(line) {
            return idx;
        }
    }
    lines.len().max(start_idx + 1)
}

fn looks_like_top_level_definition(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("pub ")
        || t.starts_with("pub(")
        || t.starts_with("export ")
        || t.starts_with("def ")
        || t.starts_with("async def ")
        || t.starts_with("class ")
        || t.starts_with("func ")
        || t.starts_with("type ")
        || t.starts_with("const ")
        || t.starts_with("var ")
}

fn leading_spaces(line: &str) -> usize {
    line.chars().take_while(|c| c.is_whitespace()).count()
}

fn normalize_symbol(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || c.is_whitespace() || *c == '-' || *c == '_')
        .flat_map(|c| c.to_lowercase())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn endpoint_json(raw: &str) -> Value {
    let (file, symbol) = raw.split_once('#').unwrap_or((raw, ""));
    let mut obj = Map::new();
    obj.insert("file".into(), Value::String(file.to_string()));
    if !symbol.is_empty() {
        obj.insert("symbol".into(), Value::String(symbol.to_string()));
    }
    Value::Object(obj)
}

fn split_csv(s: &str) -> Vec<String> {
    s.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn string_array(v: &Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
}

#[allow(dead_code)]
fn _repo_config_path(root: &Path) -> PathBuf {
    config::repo_config_path(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn endpoint_parses_symbol() {
        let e = endpoint_json("docs/api.md#Rate limits");
        assert_eq!(e["file"], "docs/api.md");
        assert_eq!(e["symbol"], "Rate limits");
    }

    #[test]
    fn validate_endpoint_checks_symbol() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("api.md");
        std::fs::write(&file, "# API\n\n## Rate limits\n").unwrap();
        let nudge = json!({
            "when": { "file": "api.md", "symbol": "Rate limits" },
            "edit": [{ "file": "api.md", "symbol": "Missing" }]
        });
        let errors = validate_nudge(dir.path(), &nudge);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("Missing"));
    }

    #[test]
    fn markdown_symbol_resolves_section_span() {
        let text = "# API\n\n## Rate limits\nBody\n\n### Burst\nMore\n\n## Auth\n";
        let span = resolve_markdown_heading(text, "Rate limits").unwrap();
        assert_eq!(span.start_line, 3);
        assert_eq!(span.end_line, 8);
    }

    #[test]
    fn code_symbol_requires_exported_shape() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("lib.rs");
        std::fs::write(
            &file,
            "fn private() {}\n\npub(crate) mod api;\n\npub fn public_api() {\n    private();\n}\n\npub struct App;\n",
        )
        .unwrap();
        assert!(symbol_exists(&file, "api"));
        assert!(symbol_exists(&file, "public_api"));
        assert!(symbol_exists(&file, "App"));
        assert!(!symbol_exists(&file, "private"));
    }

    #[test]
    fn exported_typescript_symbol_resolves() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("api.ts");
        std::fs::write(
            &file,
            "const hidden = 1;\nexport default function createApp() {}\nexport enum Mode {}\nexport const visible = 2;\n",
        )
        .unwrap();
        assert!(symbol_exists(&file, "createApp"));
        assert!(symbol_exists(&file, "Mode"));
        assert!(symbol_exists(&file, "visible"));
        assert!(!symbol_exists(&file, "hidden"));
    }

    #[test]
    fn exported_go_symbol_resolves_methods_and_vars() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("main.go");
        std::fs::write(
            &file,
            "func helper() {}\nfunc (s *Server) Listen() {}\nvar DefaultPort = 8080\n",
        )
        .unwrap();
        assert!(symbol_exists(&file, "Listen"));
        assert!(symbol_exists(&file, "DefaultPort"));
        assert!(!symbol_exists(&file, "helper"));
    }
}
