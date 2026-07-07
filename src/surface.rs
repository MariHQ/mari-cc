//! Repo surface extraction and explore wrapper (SPEC §5.2/§5.6).

use crate::{index, workspace};
use anyhow::Result;
use ignore::WalkBuilder;
use regex::Regex;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct SurfaceItem {
    pub kind: String,
    pub name: String,
    pub file: String,
    pub line: usize,
    pub signature: String,
}

pub fn surface(dir: Option<&str>, json: bool) -> Result<i32> {
    let root = workspace::work_root();
    let start = dir.map(PathBuf::from).unwrap_or_else(|| root.clone());
    let start = if start.is_absolute() {
        start
    } else {
        root.join(start)
    };
    if !start.exists() {
        eprintln!("✗ surface target does not exist: {}", start.display());
        return Ok(1);
    }
    let mut items = collect_surface(&root, &start);
    items.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.name.cmp(&b.name))
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&items)?);
    } else {
        for item in items {
            println!(
                "{}:{}  {}  {}  {}",
                item.file, item.line, item.kind, item.name, item.signature
            );
        }
    }
    Ok(0)
}

pub fn collect_surface(root: &Path, start: &Path) -> Vec<SurfaceItem> {
    let mut items = Vec::new();
    for file in files(start) {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        items.extend(extract_file(root, &file, &text));
    }
    items
}

pub fn explore(
    query_or_file: &str,
    k: Option<usize>,
    json: bool,
    deep: bool,
    focus: bool,
) -> Result<i32> {
    let query = if Path::new(query_or_file).exists() {
        file_query(Path::new(query_or_file))
    } else {
        query_or_file.to_string()
    };
    let rc = index::search::run(index::search::SearchArgs {
        query: query.clone(),
        full: Some(500),
        variants: Vec::new(),
        k,
        source: None,
        doc: None,
        author: None,
        since: None,
        before: None,
        tag: None,
        no_tag: None,
        expand: None,
        json,
    })?;
    if deep || focus {
        // §17 Tier 2 focus: for the top hits, show where in each document
        // the attention mass concentrates for this query.
        let threshold = crate::config::resolve(Some(&crate::workspace::work_root()))["attention"]
            ["threshold"]
            .as_f64()
            .unwrap_or(0.3);
        let top = index::search::top_docs(&query, k.unwrap_or(3).min(5))?;
        if top.is_empty() {
            eprintln!("(--focus: no indexed documents to attend over)");
        }
        for (cref, body) in top {
            println!("\n⌖ focus: {cref}");
            match crate::attn::analyze(&body, &query, crate::attn::Mode::Focus, threshold, None) {
                Ok(flagged) if flagged.is_empty() => {
                    println!("  (attention is diffuse — no concentrated region)")
                }
                Ok(flagged) => {
                    for f in flagged.iter().take(3) {
                        let snippet: String =
                            f.text.split_whitespace().collect::<Vec<_>>().join(" ");
                        println!(
                            "  {:.0}%  ≈L{}  {}",
                            f.score * 100.0,
                            crate::attn::line_of_offset(&body, f.offset),
                            snippet.chars().take(110).collect::<String>()
                        );
                    }
                }
                Err(e) => {
                    eprintln!("✗ focus attention failed: {e:#}");
                    break;
                }
            }
        }
    }
    Ok(rc)
}

fn files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !(e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                && matches!(
                    name.as_ref(),
                    ".git"
                        | ".mari"
                        | "target"
                        | "node_modules"
                        | "dist"
                        | "build"
                        | ".next"
                        | "vendor"
                ))
        })
        .build()
        .flatten()
    {
        let path = entry.path();
        if path.is_file() && is_surface_path(path) {
            out.push(path.to_path_buf());
        }
    }
    out.sort();
    out
}

fn is_surface_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "rs" | "ts"
                    | "tsx"
                    | "js"
                    | "jsx"
                    | "py"
                    | "go"
                    | "md"
                    | "mdx"
                    | "toml"
                    | "yaml"
                    | "yml"
                    | "json"
            )
        })
        .unwrap_or(false)
}

fn extract_file(root: &Path, path: &Path, text: &str) -> Vec<SurfaceItem> {
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "rs" => extract_regex(
            &rel,
            text,
            "rust",
            &[
                r"^\s*pub(?:\([^)]*\))?\s+(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)",
                r"^\s*pub(?:\([^)]*\))?\s+(?:struct|enum|trait|type|const|static|mod)\s+([A-Za-z_][A-Za-z0-9_]*)",
            ],
        ),
        "ts" | "tsx" | "js" | "jsx" => extract_regex(
            &rel,
            text,
            "js-ts",
            &[
                r"^\s*export\s+(?:default\s+)?(?:async\s+)?function\s+([A-Za-z_$][A-Za-z0-9_$]*)",
                r"^\s*export\s+(?:default\s+)?(?:const|let|var|class|interface|type|enum)\s+([A-Za-z_$][A-Za-z0-9_$]*)",
            ],
        ),
        "py" => extract_regex(
            &rel,
            text,
            "python",
            &[
                r"^\s*(?:async\s+)?def\s+([A-Za-z_][A-Za-z0-9_]*)",
                r"^\s*class\s+([A-Za-z_][A-Za-z0-9_]*)",
            ],
        ),
        "go" => extract_regex(
            &rel,
            text,
            "go",
            &[
                r"^\s*func\s+([A-Z][A-Za-z0-9_]*)",
                r"^\s*func\s+\([^)]*\)\s*([A-Z][A-Za-z0-9_]*)",
                r"^\s*type\s+([A-Z][A-Za-z0-9_]*)",
                r"^\s*(?:const|var)\s+([A-Z][A-Za-z0-9_]*)",
            ],
        ),
        "md" | "mdx" => extract_markdown(&rel, text),
        "toml" | "yaml" | "yml" | "json" => extract_config_keys(&rel, &ext, text),
        _ => Vec::new(),
    }
}

fn extract_regex(rel: &str, text: &str, kind: &str, patterns: &[&str]) -> Vec<SurfaceItem> {
    let regexes: Vec<Regex> = patterns.iter().map(|p| Regex::new(p).unwrap()).collect();
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        for re in &regexes {
            if let Some(caps) = re.captures(line) {
                let name = caps.get(1).unwrap().as_str().to_string();
                if kind == "python" && name.starts_with('_') {
                    continue;
                }
                out.push(SurfaceItem {
                    kind: kind.into(),
                    name,
                    file: rel.into(),
                    line: idx + 1,
                    signature: line.trim().into(),
                });
            }
        }
    }
    out
}

fn extract_headings(rel: &str, text: &str) -> Vec<SurfaceItem> {
    text.lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let t = line.trim();
            let n = t.chars().take_while(|c| *c == '#').count();
            ((1..=6).contains(&n) && t[n..].starts_with(' ')).then(|| SurfaceItem {
                kind: "heading".into(),
                name: t[n..].trim().into(),
                file: rel.into(),
                line: idx + 1,
                signature: t.into(),
            })
        })
        .collect()
}

fn extract_markdown(rel: &str, text: &str) -> Vec<SurfaceItem> {
    let mut out = extract_headings(rel, text);
    out.extend(extract_command_spans(rel, text));
    out
}

fn extract_command_spans(rel: &str, text: &str) -> Vec<SurfaceItem> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            let command = trimmed.strip_prefix("$ ").unwrap_or(trimmed);
            if is_command_like(command) {
                out.push(command_item(rel, idx + 1, command));
            }
            continue;
        }
        for span in inline_code_spans(line) {
            if is_command_like(&span) {
                out.push(command_item(rel, idx + 1, &span));
            }
        }
    }
    out
}

fn inline_code_spans(line: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('`') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('`') else {
            break;
        };
        let span = after_start[..end].trim();
        if !span.is_empty() {
            spans.push(span.to_string());
        }
        rest = &after_start[end + 1..];
    }
    spans
}

fn command_item(rel: &str, line: usize, command: &str) -> SurfaceItem {
    let command = command.trim();
    SurfaceItem {
        kind: "command".into(),
        name: command_name(command).into(),
        file: rel.into(),
        line,
        signature: command.into(),
    }
}

fn command_name(command: &str) -> &str {
    command.split_whitespace().next().unwrap_or(command)
}

fn is_command_like(command: &str) -> bool {
    let command = command.trim();
    let raw_name = command_name(command);
    let name = raw_name.strip_prefix("./").unwrap_or(raw_name);
    matches!(
        name,
        "mari"
            | "cargo"
            | "git"
            | "gh"
            | "npm"
            | "pnpm"
            | "yarn"
            | "make"
            | "docker"
            | "kubectl"
            | "python"
            | "python3"
            | "node"
    ) || raw_name.starts_with("./")
}

fn extract_config_keys(rel: &str, ext: &str, text: &str) -> Vec<SurfaceItem> {
    match ext {
        "toml" => extract_toml_keys(rel, text),
        "yaml" | "yml" => extract_yaml_keys(rel, text),
        "json" => extract_json_keys(rel, text),
        _ => extract_flat_config_keys(rel, text),
    }
}

fn extract_flat_config_keys(rel: &str, text: &str) -> Vec<SurfaceItem> {
    let key_re = Regex::new(r#"^\s*["']?([A-Za-z_][A-Za-z0-9_.-]*)["']?\s*[:=]"#).unwrap();
    text.lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            key_re.captures(line).map(|caps| SurfaceItem {
                kind: "config".into(),
                name: caps.get(1).unwrap().as_str().into(),
                file: rel.into(),
                line: idx + 1,
                signature: line.trim().into(),
            })
        })
        .collect()
}

fn extract_toml_keys(rel: &str, text: &str) -> Vec<SurfaceItem> {
    let section_re = Regex::new(r#"^\s*\[\[?\s*([A-Za-z0-9_.-]+)\s*\]?\]\s*$"#).unwrap();
    let key_re = Regex::new(r#"^\s*([A-Za-z_][A-Za-z0-9_-]*)\s*="#).unwrap();
    let mut section: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(caps) = section_re.captures(trimmed) {
            section = caps
                .get(1)
                .unwrap()
                .as_str()
                .split('.')
                .map(str::to_string)
                .collect();
            out.push(config_item(rel, idx + 1, &section.join("."), trimmed));
            continue;
        }
        if let Some(caps) = key_re.captures(trimmed) {
            let key = caps.get(1).unwrap().as_str();
            let name = dotted_name(&section, key);
            out.push(config_item(rel, idx + 1, &name, trimmed));
        }
    }
    out
}

fn extract_yaml_keys(rel: &str, text: &str) -> Vec<SurfaceItem> {
    let key_re = Regex::new(r#"^(\s*)([A-Za-z_][A-Za-z0-9_.-]*)\s*:"#).unwrap();
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
            continue;
        }
        let Some(caps) = key_re.captures(line) else {
            continue;
        };
        let indent = caps.get(1).unwrap().as_str().chars().count();
        let key = caps.get(2).unwrap().as_str().to_string();
        while stack.last().map(|(i, _)| *i >= indent).unwrap_or(false) {
            stack.pop();
        }
        let mut parts: Vec<String> = stack.iter().map(|(_, key)| key.clone()).collect();
        parts.push(key.clone());
        let name = parts.join(".");
        out.push(config_item(rel, idx + 1, &name, trimmed));
        if value_after_colon(trimmed).is_empty() {
            stack.push((indent, key));
        }
    }
    out
}

fn extract_json_keys(rel: &str, text: &str) -> Vec<SurfaceItem> {
    let key_re = Regex::new(r#"^(\s*)"([^"]+)"\s*:"#).unwrap();
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let Some(caps) = key_re.captures(line) else {
            continue;
        };
        let indent = caps.get(1).unwrap().as_str().chars().count();
        let key = caps.get(2).unwrap().as_str().to_string();
        while stack.last().map(|(i, _)| *i >= indent).unwrap_or(false) {
            stack.pop();
        }
        let mut parts: Vec<String> = stack.iter().map(|(_, key)| key.clone()).collect();
        parts.push(key.clone());
        let trimmed = line.trim();
        out.push(config_item(rel, idx + 1, &parts.join("."), trimmed));
        let value = value_after_colon(trimmed);
        if value.starts_with('{') || value.starts_with('[') {
            stack.push((indent, key));
        }
    }
    out
}

fn value_after_colon(line: &str) -> &str {
    line.split_once(':').map(|(_, v)| v.trim()).unwrap_or("")
}

fn dotted_name(prefix: &[String], key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{}.{}", prefix.join("."), key)
    }
}

fn config_item(rel: &str, line: usize, name: &str, signature: &str) -> SurfaceItem {
    SurfaceItem {
        kind: "config".into(),
        name: name.into(),
        file: rel.into(),
        line,
        signature: signature.into(),
    }
}

fn file_query(path: &Path) -> String {
    let mut parts = Vec::new();
    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
        parts.push(name.replace(['-', '_'], " "));
    }
    if let Ok(text) = std::fs::read_to_string(path) {
        parts.extend(
            extract_file(&workspace::work_root(), path, &text)
                .into_iter()
                .take(12)
                .map(|i| i.name),
        );
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_missing_target_returns_runtime_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        assert_eq!(surface(Some(&missing.to_string_lossy()), false).unwrap(), 1);
    }

    #[test]
    fn extracts_rust_pub_items() {
        let items = extract_file(
            Path::new("."),
            Path::new("src/lib.rs"),
            "pub fn run() {}\npub(crate) mod inner;\npub struct App;\n",
        );
        assert_eq!(
            items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
            vec!["run", "inner", "App"]
        );
    }

    #[test]
    fn extracts_exported_js_ts_and_go_forms() {
        let ts = extract_file(
            Path::new("."),
            Path::new("src/api.ts"),
            "export default function createApp() {}\nexport enum Mode {}\n",
        );
        assert_eq!(
            ts.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
            vec!["createApp", "Mode"]
        );

        let go = extract_file(
            Path::new("."),
            Path::new("main.go"),
            "func (s *Server) Listen() {}\nconst DefaultPort = 8080\n",
        );
        assert_eq!(
            go.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
            vec!["Listen", "DefaultPort"]
        );
    }

    #[test]
    fn extracts_only_exported_python_symbols() {
        let items = extract_file(
            Path::new("."),
            Path::new("pkg/api.py"),
            "def run(): pass\n\ndef _helper(): pass\nclass Client: pass\nclass _Internal: pass\n",
        );
        assert_eq!(
            items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
            vec!["run", "Client"]
        );
    }

    #[test]
    fn extracts_markdown_headings() {
        let items = extract_file(Path::new("."), Path::new("README.md"), "# One\n\n## Two\n");
        assert_eq!(
            items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
            vec!["One", "Two"]
        );
    }

    #[test]
    fn extracts_markdown_command_spans() {
        let items = extract_file(
            Path::new("."),
            Path::new("README.md"),
            "# Setup\nRun `mari check --strict`.\n\n```sh\n$ cargo test\n./tools/sync\nnot a command\n```\n",
        );
        let commands = items
            .iter()
            .filter(|i| i.kind == "command")
            .map(|i| i.signature.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            commands,
            vec!["mari check --strict", "cargo test", "./tools/sync"]
        );
    }

    #[test]
    fn extracts_dotted_config_paths() {
        let toml = extract_file(
            Path::new("."),
            Path::new("config.toml"),
            "[tool.mari]\nstrict = true\n\n[[tool.mari.rules]]\nname = \"docs\"\n",
        );
        assert_eq!(
            toml.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
            vec![
                "tool.mari",
                "tool.mari.strict",
                "tool.mari.rules",
                "tool.mari.rules.name"
            ]
        );

        let yaml = extract_file(
            Path::new("."),
            Path::new("mkdocs.yml"),
            "theme:\n  features:\n    search: true\nnav:\n  - Home: index.md\n",
        );
        assert!(yaml.iter().any(|i| i.name == "theme.features.search"));

        let json = extract_file(
            Path::new("."),
            Path::new("package.json"),
            "{\n  \"scripts\": {\n    \"test\": \"cargo test\"\n  }\n}\n",
        );
        assert!(json.iter().any(|i| i.name == "scripts.test"));
    }
}
