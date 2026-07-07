//! Whole-project docs validation (SPEC §5.6).

use crate::{assets, config, surface, workspace};
use anyhow::Result;
use ignore::WalkBuilder;
use pulldown_cmark::{Event, Options, Parser, Tag};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
struct Finding {
    rule_id: &'static str,
    severity: &'static str,
    path: String,
    message: String,
    target: Option<String>,
}

pub fn run(
    json: bool,
    strict: bool,
    deep: bool,
    limit: Option<usize>,
    _threshold: Option<f64>,
) -> Result<i32> {
    if deep {
        eprintln!("note: check --deep attention grounding is not available in this build; running deterministic API-surface checks only");
    }

    let root = workspace::work_root();
    let cfg = config::resolve(Some(&root));
    let ignored = ignored_rules(&cfg);
    let mut findings = Vec::new();

    if !ignored.contains("community-missing-file") {
        findings.extend(community_findings(&root));
    }
    if !ignored.contains("link-broken") {
        let mut docs = markdown_files(&root);
        if let Some(limit) = limit {
            docs.truncate(limit);
        }
        findings.extend(link_findings(&root, &docs));
    }
    if !ignored.contains("nav-missing-target") || !ignored.contains("nav-orphan-page") {
        let docs = markdown_files(&root);
        findings.extend(nav_findings(
            &root,
            &docs,
            !ignored.contains("nav-missing-target"),
            !ignored.contains("nav-orphan-page"),
        ));
    }
    if !ignored.contains("asset-missing-section") || !ignored.contains("postmortem-blame") {
        let docs = markdown_files(&root);
        findings.extend(asset_findings(&root, &docs, &ignored));
    }
    if deep && !ignored.contains("undocumented-symbol") {
        findings.extend(undocumented_symbol_findings(&root, limit));
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&findings)?);
    } else if findings.is_empty() {
        println!("check: ok");
    } else {
        for finding in &findings {
            let target = finding
                .target
                .as_ref()
                .map(|t| format!(" -> {t}"))
                .unwrap_or_default();
            println!(
                "{} {} {}{}",
                finding.severity, finding.rule_id, finding.path, target
            );
            println!("  {}", finding.message);
        }
    }

    let has_error = findings.iter().any(|f| f.severity == "error");
    if has_error || (strict && !findings.is_empty()) {
        Ok(1)
    } else {
        Ok(0)
    }
}

fn ignored_rules(cfg: &serde_json::Value) -> BTreeSet<String> {
    cfg["detector"]["ignoreRules"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .map(|s| s.to_string())
        .collect()
}

fn community_findings(root: &Path) -> Vec<Finding> {
    let required = ["README.md", "LICENSE", "CONTRIBUTING.md"];
    let recommended = ["CODE_OF_CONDUCT.md", "SECURITY.md", "CHANGELOG.md"];
    let mut out = Vec::new();
    for file in required {
        if !root.join(file).exists() {
            out.push(Finding {
                rule_id: "community-missing-file",
                severity: "error",
                path: file.into(),
                message: format!("required community-health file is missing: {file}"),
                target: None,
            });
        }
    }
    for file in recommended {
        if !root.join(file).exists() {
            out.push(Finding {
                rule_id: "community-missing-file",
                severity: "warn",
                path: file.into(),
                message: format!("recommended community-health file is missing: {file}"),
                target: None,
            });
        }
    }
    out
}

fn markdown_files(root: &Path) -> Vec<PathBuf> {
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
                        | "coverage"
                        | "fixtures"
                        | "__fixtures__"
                        | "vendor"
                        | "vendored"
                ))
        })
        .build()
        .flatten()
    {
        let path = entry.path();
        if path.is_file() && is_markdown(path) && !is_generated_doc(path) {
            out.push(path.to_path_buf());
        }
    }
    out.sort();
    out
}

fn link_findings(root: &Path, docs: &[PathBuf]) -> Vec<Finding> {
    let mut out = Vec::new();
    for doc in docs {
        let Ok(text) = std::fs::read_to_string(doc) else {
            continue;
        };
        let doc_anchors = anchors(&text);
        for link in links(&text) {
            if is_external(&link) {
                continue;
            }
            if let Some(anchor) = link.strip_prefix('#') {
                if !anchor.is_empty() && !doc_anchors.contains(&normalize_anchor(anchor)) {
                    out.push(link_finding(
                        root,
                        doc,
                        &link,
                        "anchor does not exist in this file",
                    ));
                }
                continue;
            }
            let (target_path, anchor) = split_target(&link);
            if target_path.is_empty() {
                continue;
            }
            let target = doc
                .parent()
                .unwrap_or(root)
                .join(percent_decode(target_path));
            let target = normalize_doc_target(&target);
            if !target.exists() {
                out.push(link_finding(root, doc, &link, "linked file does not exist"));
                continue;
            }
            if let Some(anchor) = anchor {
                if is_markdown(&target) {
                    let Ok(target_text) = std::fs::read_to_string(&target) else {
                        continue;
                    };
                    if !anchors(&target_text).contains(&normalize_anchor(anchor)) {
                        out.push(link_finding(
                            root,
                            doc,
                            &link,
                            "linked anchor does not exist",
                        ));
                    }
                }
            }
        }
    }
    out
}

fn nav_findings(
    root: &Path,
    docs: &[PathBuf],
    check_missing: bool,
    check_orphans: bool,
) -> Vec<Finding> {
    let mut out = Vec::new();
    for nav in ["mkdocs.yml", "mkdocs.yaml", "_toc.yml"] {
        let path = root.join(nav);
        if !path.exists() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut nav_targets = BTreeSet::new();
        for target in nav_markdown_targets(&text) {
            let resolved = root.join(&target);
            let normalized = normalize_doc_target(&resolved);
            if let Ok(rel) = normalized.strip_prefix(root) {
                nav_targets.insert(rel.to_string_lossy().to_string());
            }
            if check_missing && !normalized.exists() {
                out.push(Finding {
                    rule_id: "nav-missing-target",
                    severity: "error",
                    path: nav.into(),
                    message: "navigation references a markdown file that does not exist".into(),
                    target: Some(target),
                });
            }
        }
        if check_orphans {
            for doc in docs {
                if !is_nav_candidate(root, doc) {
                    continue;
                }
                let rel_doc = rel(root, doc);
                if !nav_targets.contains(&rel_doc) {
                    out.push(Finding {
                        rule_id: "nav-orphan-page",
                        severity: "warn",
                        path: rel_doc,
                        message: format!("markdown page is not referenced by {nav}"),
                        target: Some(nav.into()),
                    });
                }
            }
        }
    }
    out
}

fn asset_findings(root: &Path, docs: &[PathBuf], ignored: &BTreeSet<String>) -> Vec<Finding> {
    let mut out = Vec::new();
    for doc in docs {
        let Ok(findings) = assets::findings_for_path(doc) else {
            continue;
        };
        for finding in findings {
            if ignored.contains(finding.rule_id) {
                continue;
            }
            out.push(Finding {
                rule_id: finding.rule_id,
                severity: finding.severity,
                path: rel(root, doc),
                message: finding.message,
                target: Some(finding.asset_type),
            });
        }
    }
    out
}

fn undocumented_symbol_findings(root: &Path, limit: Option<usize>) -> Vec<Finding> {
    let docs = markdown_files(root);
    let docs_text = docs
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();
    if docs_text.trim().is_empty() {
        return Vec::new();
    }

    let mut symbols = surface::collect_surface(root, root)
        .into_iter()
        .filter(|item| is_code_surface_kind(&item.kind))
        .filter(|item| !is_test_or_fixture_path(&item.file))
        .filter(|item| !item.name.starts_with('_'))
        .collect::<Vec<_>>();
    symbols.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.name.cmp(&b.name))
    });
    if let Some(limit) = limit {
        if limit > 0 {
            symbols.truncate(limit);
        }
    }

    symbols
        .into_iter()
        .filter(|item| !docs_text.contains(&item.name.to_ascii_lowercase()))
        .map(|item| Finding {
            rule_id: "undocumented-symbol",
            severity: "warn",
            path: item.file,
            message: format!(
                "exported {} symbol `{}` is not mentioned in project markdown",
                item.kind, item.name
            ),
            target: Some(format!("{}:{}", item.name, item.line)),
        })
        .collect()
}

fn is_code_surface_kind(kind: &str) -> bool {
    matches!(kind, "rust" | "js-ts" | "python" | "go")
}

fn is_test_or_fixture_path(path: &str) -> bool {
    path.contains("/tests/")
        || path.starts_with("tests/")
        || path.contains("/fixtures/")
        || path.starts_with("fixtures/")
        || path.contains("__fixtures__")
        || path.contains("/testdata/")
        || path.starts_with("testdata/")
}

fn is_nav_candidate(root: &Path, doc: &Path) -> bool {
    let rel = rel(root, doc);
    if matches!(
        rel.as_str(),
        "README.md"
            | "CONTRIBUTING.md"
            | "CODE_OF_CONDUCT.md"
            | "SECURITY.md"
            | "CHANGELOG.md"
            | "LICENSE.md"
    ) {
        return false;
    }
    !rel.starts_with(".")
}

fn nav_markdown_targets(text: &str) -> Vec<String> {
    text.split(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '[' | ']' | '(' | ')'))
        .filter(|part| part.ends_with(".md") || part.contains(".md#"))
        .map(|part| {
            part.trim_matches(|c: char| matches!(c, ':' | ',' | '-'))
                .to_string()
        })
        .filter(|part| !part.starts_with("http://") && !part.starts_with("https://"))
        .collect()
}

fn link_finding(root: &Path, doc: &Path, link: &str, message: &str) -> Finding {
    Finding {
        rule_id: "link-broken",
        severity: "error",
        path: rel(root, doc),
        message: message.into(),
        target: Some(link.into()),
    }
}

fn links(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let parser = Parser::new_ext(text, Options::ENABLE_TABLES | Options::ENABLE_FOOTNOTES);
    for event in parser {
        if let Event::Start(Tag::Link { dest_url, .. }) = event {
            let dest = dest_url.trim();
            if !dest.is_empty() {
                out.push(dest.to_string());
            }
        }
    }
    out
}

fn anchors(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut seen = std::collections::BTreeMap::<String, usize>::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(' ') {
            let base = slug(&trimmed[hashes..]);
            let count = seen.entry(base.clone()).or_insert(0);
            if *count == 0 {
                out.insert(base);
            } else {
                out.insert(format!("{base}-{count}"));
            }
            *count += 1;
        }
    }
    out
}

fn slug(heading: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in heading.trim().chars().flat_map(|c| c.to_lowercase()) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_dash = false;
        } else if (c.is_whitespace() || c == '-') && !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn normalize_anchor(anchor: &str) -> String {
    percent_decode(anchor)
        .trim_start_matches('#')
        .trim()
        .to_ascii_lowercase()
}

fn split_target(link: &str) -> (&str, Option<&str>) {
    let link = link.split('?').next().unwrap_or(link);
    match link.split_once('#') {
        Some((path, anchor)) => (path, Some(anchor)),
        None => (link, None),
    }
}

fn normalize_doc_target(path: &Path) -> PathBuf {
    if path.is_dir() {
        let readme = path.join("README.md");
        if readme.exists() {
            return readme;
        }
        let index = path.join("index.md");
        if index.exists() {
            return index;
        }
    }
    path.to_path_buf()
}

fn percent_decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s)
        .decode_utf8_lossy()
        .to_string()
}

fn is_external(link: &str) -> bool {
    let lower = link.to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with("tel:")
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "md" | "mdx" | "mdc" | "markdown"
            )
        })
        .unwrap_or(false)
}

fn is_generated_doc(path: &Path) -> bool {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| {
            matches!(
                s.to_ascii_uppercase().as_str(),
                "CHANGELOG" | "HISTORY" | "LICENSE" | "NOTICE"
            )
        })
        .unwrap_or(false)
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn anchors_match_github_style_slug() {
        let set = anchors("# Getting Started!\n## API Reference\n");
        assert!(set.contains("getting-started"));
        assert!(set.contains("api-reference"));
    }

    #[test]
    fn anchors_include_duplicate_heading_suffixes() {
        let set = anchors("# Intro\n## Intro\n### Intro\n");
        assert!(set.contains("intro"));
        assert!(set.contains("intro-1"));
        assert!(set.contains("intro-2"));
    }

    #[test]
    fn link_check_reports_missing_file_and_anchor() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let doc = root.join("README.md");
        let target = root.join("target.md");
        std::fs::write(
            &doc,
            "# Home\n[missing](missing.md)\n[bad anchor](target.md#nope)\n",
        )
        .unwrap();
        std::fs::write(&target, "# Exists\n").unwrap();

        let findings = link_findings(root, &[doc]);
        assert_eq!(findings.len(), 2);
        assert!(findings.iter().any(|f| f.message.contains("file")));
        assert!(findings.iter().any(|f| f.message.contains("anchor")));
    }

    #[test]
    fn link_check_resolves_reference_style_links() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let doc = root.join("README.md");
        std::fs::write(
            &doc,
            "# Home\n\nSee [setup][setup-ref].\n\n[setup-ref]: docs/setup.md#install\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(root.join("docs/setup.md"), "# Setup\n").unwrap();

        let findings = link_findings(root, &[doc]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "link-broken");
        assert_eq!(findings[0].target.as_deref(), Some("docs/setup.md#install"));
        assert!(findings[0].message.contains("anchor"));
    }

    #[test]
    fn link_check_resolves_duplicate_heading_anchors() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let doc = root.join("README.md");
        std::fs::write(&doc, "# Intro\n\n## Intro\n\n[second](#intro-1)\n").unwrap();

        let findings = link_findings(root, &[doc]);

        assert!(findings.is_empty());
    }

    #[test]
    fn nav_check_reports_missing_targets_and_orphans() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("mkdocs.yml"),
            "nav:\n  - Home: docs/index.md\n  - Missing: docs/missing.md\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(root.join("docs/index.md"), "# Home\n").unwrap();
        std::fs::write(root.join("docs/orphan.md"), "# Orphan\n").unwrap();

        let docs = markdown_files(root);
        let findings = nav_findings(root, &docs, true, true);
        assert!(findings.iter().any(|f| f.rule_id == "nav-missing-target"));
        assert!(findings
            .iter()
            .any(|f| { f.rule_id == "nav-orphan-page" && f.path == "docs/orphan.md" }));
        assert!(!findings
            .iter()
            .any(|f| { f.rule_id == "nav-orphan-page" && f.path == "docs/index.md" }));
    }

    #[test]
    fn project_check_reports_asset_findings() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let doc = root.join("POSTMORTEM.md");
        std::fs::write(
            &doc,
            "# Incident Postmortem\n\n## Summary\n\nHuman error caused it.\n",
        )
        .unwrap();

        let findings = asset_findings(root, &[doc], &BTreeSet::new());
        assert!(findings
            .iter()
            .any(|f| f.rule_id == "asset-missing-section" && f.path == "POSTMORTEM.md"));
        assert!(findings
            .iter()
            .any(|f| f.rule_id == "postmortem-blame" && f.path == "POSTMORTEM.md"));
    }

    #[test]
    fn project_check_asset_findings_respect_ignore_rules() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let doc = root.join("POSTMORTEM.md");
        std::fs::write(
            &doc,
            "# Incident Postmortem\n\n## Summary\n\nHuman error caused it.\n",
        )
        .unwrap();
        let ignored = BTreeSet::from(["postmortem-blame".to_string()]);

        let findings = asset_findings(root, &[doc], &ignored);
        assert!(findings
            .iter()
            .any(|f| f.rule_id == "asset-missing-section"));
        assert!(!findings.iter().any(|f| f.rule_id == "postmortem-blame"));
    }

    #[test]
    fn deep_check_reports_undocumented_exported_symbols() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn documented_api() {}\npub fn missing_api() {}\n",
        )
        .unwrap();
        std::fs::write(root.join("README.md"), "# API\n\nUse documented_api.\n").unwrap();

        let findings = undocumented_symbol_findings(root, None);
        assert!(findings
            .iter()
            .any(|f| f.rule_id == "undocumented-symbol" && f.path == "src/lib.rs"));
        assert!(findings.iter().any(|f| f.message.contains("missing_api")));
        assert!(!findings
            .iter()
            .any(|f| f.message.contains("documented_api")));
    }
}
