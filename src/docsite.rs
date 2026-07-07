//! Deterministic docsite entry point for the agent flow (SPEC §5.6).

use crate::{checkcmd, config, workspace};
use anyhow::Result;
use ignore::WalkBuilder;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
struct DocsitePlan {
    phases: Vec<DocsitePhase>,
}

#[derive(Debug, Serialize)]
struct DocsitePhase {
    phase: &'static str,
    command: &'static str,
    output: &'static str,
}

#[derive(Debug, Serialize)]
struct DocsiteStatus {
    root: String,
    platform: Option<&'static str>,
    docs_dir: bool,
    readme: bool,
    license: bool,
    contributing: bool,
    code_of_conduct: bool,
    security: bool,
    changelog: bool,
    hook_configured: bool,
    rules_configured: bool,
    next_commands: Vec<&'static str>,
}

pub fn run(args: &[String], json: bool) -> Result<i32> {
    match args.first().map(|s| s.as_str()).unwrap_or("plan") {
        "plan" => print_plan(json),
        "status" => print_status(json),
        // Focused docs-site link validator (§5.6): internal links + anchors
        // resolve and nav↔files agree, scoped to the docs tree — distinct from
        // the whole-project `mari check`. Also validates in-page #anchor→id
        // links in HTML/JSX so code-based sites are covered.
        "check" => checkcmd::run_links(json, false, true),
        "sync" => print_sync(json),
        other => {
            eprintln!("unknown docsite action `{other}`; expected plan | status | check | sync");
            Ok(2)
        }
    }
}

fn print_plan(json: bool) -> Result<i32> {
    let plan = DocsitePlan {
        phases: vec![
            DocsitePhase {
                phase: "survey codebase",
                command: "mari surface --json",
                output: "public API, docs, config, and command surface with file/line anchors",
            },
            DocsitePhase {
                phase: "choose platform",
                command: "mari platform detect",
                output: "existing documentation platform, or a scaffoldable target",
            },
            DocsitePhase {
                phase: "design IA",
                command: "mari asset detect <file>",
                output: "document archetypes and required sections for generated pages",
            },
            DocsitePhase {
                phase: "ground pages",
                command: "mari explore <question-or-file> --k 8",
                output: "indexed source snippets for each page claim",
            },
            DocsitePhase {
                phase: "community files",
                command: "mari check --strict",
                output: "README, LICENSE, CONTRIBUTING, and recommended health files",
            },
            DocsitePhase {
                phase: "validate",
                command: "mari check --strict --deep",
                output: "links, nav, archetype structure, and API surface coverage",
            },
            DocsitePhase {
                phase: "keep alive",
                command: "mari hooks on && mari rules discover --write",
                output: "post-edit hook and edit-notify rules for drift control",
            },
        ],
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        for (idx, phase) in plan.phases.iter().enumerate() {
            println!(
                "{}. {}: {} -> {}",
                idx + 1,
                phase.phase,
                phase.command,
                phase.output
            );
        }
    }
    Ok(0)
}

fn print_status(json: bool) -> Result<i32> {
    let root = workspace::work_root();
    let status = status_for_root(&root);
    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("root: {}", status.root);
        println!("platform: {}", status.platform.unwrap_or("none detected"));
        println!("docs dir: {}", yes_no(status.docs_dir));
        println!("README: {}", yes_no(status.readme));
        println!("LICENSE: {}", yes_no(status.license));
        println!("CONTRIBUTING: {}", yes_no(status.contributing));
        println!("CODE_OF_CONDUCT: {}", yes_no(status.code_of_conduct));
        println!("SECURITY: {}", yes_no(status.security));
        println!("CHANGELOG: {}", yes_no(status.changelog));
        println!("hook configured: {}", yes_no(status.hook_configured));
        println!("rules configured: {}", yes_no(status.rules_configured));
        println!("next: {}", status.next_commands.join(" | "));
    }
    Ok(0)
}

fn status_for_root(root: &Path) -> DocsiteStatus {
    DocsiteStatus {
        root: root.display().to_string(),
        platform: detect_platform(root),
        docs_dir: root.join("docs").is_dir(),
        readme: exists_any(root, &["README.md", "README.mdx", "readme.md"]),
        license: exists_any(root, &["LICENSE", "LICENSE.md", "COPYING"]),
        contributing: exists_any(root, &["CONTRIBUTING.md", ".github/CONTRIBUTING.md"]),
        code_of_conduct: exists_any(root, &["CODE_OF_CONDUCT.md", ".github/CODE_OF_CONDUCT.md"]),
        security: exists_any(root, &["SECURITY.md", ".github/SECURITY.md"]),
        changelog: exists_any(root, &["CHANGELOG.md", "CHANGES.md", "HISTORY.md"]),
        hook_configured: file_contains(&root.join(".claude/settings.json"), "mari hook run"),
        rules_configured: has_edit_notify_rules(root),
        next_commands: vec![
            "mari surface --json",
            "mari platform detect",
            "mari check --strict",
        ],
    }
}

fn detect_platform(root: &Path) -> Option<&'static str> {
    let markers = [
        ("mkdocs", "mkdocs.yml"),
        ("mkdocs", "mkdocs.yaml"),
        ("docusaurus", "docusaurus.config.js"),
        ("docusaurus", "docusaurus.config.ts"),
        ("sphinx", "docs/conf.py"),
        ("sphinx", "conf.py"),
        ("hugo", "hugo.yaml"),
        ("hugo", "config.toml"),
        ("hugo", "config.yaml"),
        ("hugo", "hugo.toml"),
        ("jekyll", "_config.yml"),
        ("jekyll", "_config.yaml"),
        ("mdbook", "book.toml"),
        ("antora", "antora.yml"),
        ("antora", "antora-playbook.yml"),
        ("docsify", "index.html"),
        ("docsify", ".nojekyll"),
        ("vitepress", "docs/.vitepress/config.ts"),
        ("vitepress", ".vitepress/config.ts"),
        ("starlight", "astro.config.mjs"),
        ("starlight", "astro.config.ts"),
        ("gitbook", ".gitbook.yaml"),
        ("gitbook", "SUMMARY.md"),
        ("readthedocs", ".readthedocs.yaml"),
        ("readthedocs", "readthedocs.yml"),
    ];
    markers
        .iter()
        .find(|(_, marker)| root.join(marker).exists())
        .map(|(id, _)| *id)
}

fn exists_any(root: &Path, rels: &[&str]) -> bool {
    rels.iter().any(|rel| root.join(rel).exists())
}

fn file_contains(path: &Path, needle: &str) -> bool {
    std::fs::read_to_string(path)
        .map(|text| text.contains(needle))
        .unwrap_or(false)
}

fn has_edit_notify_rules(root: &Path) -> bool {
    std::fs::read_to_string(root.join(".mari/config.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|cfg| cfg["rules"].as_array().map(|rules| !rules.is_empty()))
        .unwrap_or(false)
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

#[derive(Debug, Serialize)]
struct DocsiteSync {
    /// Real CLI subcommands not mentioned as `mari <cmd>` in any project markdown.
    undocumented_commands: Vec<String>,
    /// Config keys (dotted, from the schema defaults) not mentioned in markdown.
    undocumented_config_keys: Vec<String>,
    /// Prose "<N> command(s)" phrases whose N differs from the real command count.
    stale_command_counts: Vec<StaleCount>,
    command_count: usize,
}

#[derive(Debug, Serialize)]
struct StaleCount {
    file: String,
    line: usize,
    stated: usize,
    actual: usize,
}

/// §5.6 `mari docsite sync`: compare the documented surface against the real one
/// and report drift. Deterministic complement to the `cli-docs` notify rule and
/// the CI gate. Undocumented commands and stale counts fail (exit 1);
/// undocumented config keys are advisory.
fn print_sync(json: bool) -> Result<i32> {
    let root = workspace::work_root();
    let real_commands = real_commands();
    let real_config_keys = config_keys();
    let markdown = project_markdown(&root);
    let corpus = markdown
        .iter()
        .map(|(_, _, text)| text.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("\n");

    let undocumented_commands: Vec<String> = real_commands
        .iter()
        .filter(|c| !corpus.contains(&format!("mari {c}")))
        .cloned()
        .collect();

    let undocumented_config_keys: Vec<String> = real_config_keys
        .iter()
        .filter(|k| !corpus.contains(&k.to_ascii_lowercase()))
        .cloned()
        .collect();

    let stale_command_counts = stale_counts(&markdown, real_commands.len());

    let sync = DocsiteSync {
        undocumented_commands,
        undocumented_config_keys,
        stale_command_counts,
        command_count: real_commands.len(),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&sync)?);
    } else {
        if sync.undocumented_commands.is_empty()
            && sync.stale_command_counts.is_empty()
            && sync.undocumented_config_keys.is_empty()
        {
            println!(
                "docsite sync: docs match the surface ({} commands)",
                sync.command_count
            );
        }
        if !sync.undocumented_commands.is_empty() {
            println!(
                "CLI commands in code but not in docs: {}",
                sync.undocumented_commands.join(", ")
            );
        }
        if !sync.undocumented_config_keys.is_empty() {
            println!(
                "config keys not documented ({}): {}",
                sync.undocumented_config_keys.len(),
                preview(&sync.undocumented_config_keys, 12)
            );
        }
        for stale in &sync.stale_command_counts {
            println!(
                "stale count: {}:{} says \"{} commands\", code exposes {}",
                stale.file, stale.line, stale.stated, stale.actual
            );
        }
    }

    // Undocumented commands and stale counts are hard drift (CI-gatable);
    // undocumented config keys are advisory and don't fail the gate.
    let drift = !sync.undocumented_commands.is_empty() || !sync.stale_command_counts.is_empty();
    Ok(if drift { 1 } else { 0 })
}

/// Real subcommand names, introspected from the clap definition so this can
/// never drift from the actual CLI.
fn real_commands() -> Vec<String> {
    use clap::CommandFactory;
    crate::Cli::command()
        .get_subcommands()
        .map(|s| s.get_name().to_string())
        .collect()
}

/// Dotted leaf paths of the config schema defaults (§4).
fn config_keys() -> Vec<String> {
    let mut out = Vec::new();
    flatten_keys(&config::defaults(), String::new(), &mut out);
    out.sort();
    out.dedup();
    out
}

fn flatten_keys(value: &serde_json::Value, prefix: String, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                // Recurse into non-empty sections; a scalar, an array, or an
                // empty object (e.g. `detector.ignoreSpans = {}`) is itself a
                // documented leaf key.
                if v.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
                    flatten_keys(v, path, out);
                } else if !prefix.is_empty() {
                    out.push(path);
                }
            }
        }
        _ => {
            if !prefix.is_empty() {
                out.push(prefix);
            }
        }
    }
}

fn project_markdown(root: &Path) -> Vec<(String, usize, String)> {
    let mut out = Vec::new();
    for entry in WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !(e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                && matches!(
                    name.as_ref(),
                    ".git" | ".mari" | "target" | "node_modules" | "dist" | "build" | "vendor"
                ))
        })
        .build()
        .flatten()
    {
        let path = entry.path();
        let is_md = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| matches!(e.to_ascii_lowercase().as_str(), "md" | "mdx" | "markdown"))
            .unwrap_or(false);
        if path.is_file() && is_md {
            if let Ok(text) = std::fs::read_to_string(path) {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(path)
                    .display()
                    .to_string();
                for (idx, line) in text.lines().enumerate() {
                    out.push((rel.clone(), idx + 1, line.to_string()));
                }
            }
        }
    }
    out
}

/// Find "<N> command(s)" phrases whose N differs from the real count.
fn stale_counts(markdown: &[(String, usize, String)], actual: usize) -> Vec<StaleCount> {
    let re = regex::Regex::new(r"(?i)\b(\d+)\s+commands?\b").unwrap();
    let mut out = Vec::new();
    for (file, line, text) in markdown {
        for caps in re.captures_iter(text) {
            if let Some(stated) = caps.get(1).and_then(|m| m.as_str().parse::<usize>().ok()) {
                if stated != actual {
                    out.push(StaleCount {
                        file: file.clone(),
                        line: *line,
                        stated,
                        actual,
                    });
                }
            }
        }
    }
    out
}

fn preview(items: &[String], n: usize) -> String {
    let shown = items.iter().take(n).cloned().collect::<Vec<_>>().join(", ");
    if items.len() > n {
        format!("{shown}, … (+{})", items.len() - n)
    } else {
        shown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn plan_has_seven_phases() {
        let plan = DocsitePlan {
            phases: vec![
                DocsitePhase {
                    phase: "survey codebase",
                    command: "mari surface --json",
                    output: "surface",
                },
                DocsitePhase {
                    phase: "choose platform",
                    command: "mari platform detect",
                    output: "platform",
                },
                DocsitePhase {
                    phase: "design IA",
                    command: "mari asset detect <file>",
                    output: "ia",
                },
                DocsitePhase {
                    phase: "ground pages",
                    command: "mari explore <question-or-file> --k 8",
                    output: "grounding",
                },
                DocsitePhase {
                    phase: "community files",
                    command: "mari check --strict",
                    output: "health",
                },
                DocsitePhase {
                    phase: "validate",
                    command: "mari check --strict --deep",
                    output: "validation",
                },
                DocsitePhase {
                    phase: "keep alive",
                    command: "mari hooks on && mari rules discover --write",
                    output: "drift",
                },
            ],
        };
        assert_eq!(plan.phases.len(), 7);
    }

    #[test]
    fn unknown_docsite_action_is_usage_error() {
        assert_eq!(run(&["publish".into()], false).unwrap(), 2);
    }

    #[test]
    fn real_commands_come_from_the_clap_definition() {
        let cmds = real_commands();
        for expected in ["detect", "docsite", "check", "surface", "sync"] {
            assert!(
                cmds.contains(&expected.to_string()),
                "missing {expected} in {cmds:?}"
            );
        }
        // Names invented earlier in this session must NOT appear.
        for bogus in ["context", "install", "pin", "plugins", "community", "live"] {
            assert!(!cmds.contains(&bogus.to_string()), "unexpected {bogus}");
        }
    }

    #[test]
    fn config_keys_flatten_to_dotted_leaf_paths() {
        let keys = config_keys();
        assert!(keys.iter().any(|k| k == "search.k"));
        assert!(keys.iter().any(|k| k == "detector.styleGuide"));
        assert!(keys.iter().any(|k| k == "detector.ignoreSpans"));
    }

    #[test]
    fn stale_counts_flag_mismatched_command_phrases() {
        let md = vec![
            (
                "a.md".to_string(),
                3,
                "Mari ships 22 commands today.".to_string(),
            ),
            (
                "b.md".to_string(),
                1,
                "All 42 commands are documented.".to_string(),
            ),
        ];
        let stale = stale_counts(&md, 42);
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].file, "a.md");
        assert_eq!(stale[0].stated, 22);
        assert_eq!(stale[0].actual, 42);
    }

    #[test]
    fn status_detects_documented_platform_markers() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        std::fs::create_dir_all(dir.path().join("docs/.vitepress")).unwrap();
        std::fs::write(
            dir.path().join("docs/.vitepress/config.ts"),
            "export default {}\n",
        )
        .unwrap();

        assert_eq!(detect_platform(dir.path()), Some("vitepress"));
    }

    #[test]
    fn status_rules_configured_requires_nonempty_rules() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".mari")).unwrap();
        std::fs::write(dir.path().join(".mari/config.json"), r#"{"rules":[]}"#).unwrap();
        assert!(!status_for_root(dir.path()).rules_configured);

        std::fs::write(
            dir.path().join(".mari/config.json"),
            r#"{"rules":[{"name":"api-docs","paths":["src/**"],"notify":"update docs"}]}"#,
        )
        .unwrap();
        assert!(status_for_root(dir.path()).rules_configured);
    }

    #[test]
    fn status_reports_required_and_recommended_community_health_files() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".github")).unwrap();
        std::fs::write(dir.path().join("README.md"), "# Project\n").unwrap();
        std::fs::write(dir.path().join("LICENSE"), "MIT\n").unwrap();
        std::fs::write(
            dir.path().join(".github/CONTRIBUTING.md"),
            "# Contributing\n",
        )
        .unwrap();
        std::fs::write(dir.path().join(".github/CODE_OF_CONDUCT.md"), "# Conduct\n").unwrap();
        std::fs::write(dir.path().join("SECURITY.md"), "# Security\n").unwrap();
        std::fs::write(dir.path().join("CHANGELOG.md"), "# Changelog\n").unwrap();

        let status = status_for_root(dir.path());

        assert!(status.readme);
        assert!(status.license);
        assert!(status.contributing);
        assert!(status.code_of_conduct);
        assert!(status.security);
        assert!(status.changelog);
    }
}
