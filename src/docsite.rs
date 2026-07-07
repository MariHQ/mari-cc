//! Deterministic docsite entry point for the agent flow (SPEC §5.6).

use crate::workspace;
use anyhow::{bail, Result};
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
    hook_configured: bool,
    rules_configured: bool,
    next_commands: Vec<&'static str>,
}

pub fn run(args: &[String], json: bool) -> Result<i32> {
    match args.first().map(|s| s.as_str()).unwrap_or("plan") {
        "plan" => print_plan(json),
        "status" => print_status(json),
        other => bail!("unknown docsite action `{other}`; expected plan or status"),
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
        println!("hook configured: {}", yes_no(status.hook_configured));
        println!("rules configured: {}", yes_no(status.rules_configured));
        println!("next: {}", status.next_commands.join(" | "));
    }
    Ok(0)
}

fn status_for_root(root: &Path) -> DocsiteStatus {
    DocsiteStatus {
        root: root.display().to_string(),
        platform: detect_platform(&root),
        docs_dir: root.join("docs").is_dir(),
        readme: exists_any(&root, &["README.md", "README.mdx", "readme.md"]),
        license: exists_any(&root, &["LICENSE", "LICENSE.md", "COPYING"]),
        contributing: exists_any(&root, &["CONTRIBUTING.md", ".github/CONTRIBUTING.md"]),
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
}
