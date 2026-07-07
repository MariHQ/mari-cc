//! Asset archetypes (SPEC §5.6/§14).

use crate::workspace;
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
struct Archetype {
    id: &'static str,
    title: &'static str,
    file: &'static str,
    sections: &'static [&'static str],
    basis: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Finding {
    pub(crate) rule_id: &'static str,
    pub(crate) severity: &'static str,
    pub(crate) asset_type: String,
    pub(crate) path: String,
    pub(crate) message: String,
}

const ARCHETYPES: &[Archetype] = &[
    Archetype {
        id: "runbook",
        title: "Runbook",
        file: "RUNBOOK.md",
        sections: &[
            "Overview",
            "Prerequisites",
            "Steps",
            "Rollback",
            "Escalation",
        ],
        basis: "incident-response 5 A's",
    },
    Archetype {
        id: "adr",
        title: "Architecture Decision Record",
        file: "ADR.md",
        sections: &["Status", "Context", "Decision", "Consequences"],
        basis: "Nygard / MADR",
    },
    Archetype {
        id: "postmortem",
        title: "Postmortem",
        file: "POSTMORTEM.md",
        sections: &[
            "Summary",
            "Impact",
            "Timeline",
            "Root Cause",
            "Action Items",
            "Lessons",
        ],
        basis: "Google SRE blameless postmortem",
    },
    Archetype {
        id: "rfc",
        title: "Request for Comments",
        file: "RFC.md",
        sections: &[
            "Summary",
            "Motivation",
            "Alternatives",
            "Drawbacks",
            "Rollout Plan",
            "Open Questions",
        ],
        basis: "Rust RFC / Oxide RFD",
    },
    Archetype {
        id: "contributing",
        title: "Contributing",
        file: "CONTRIBUTING.md",
        sections: &[
            "Getting Started",
            "Development",
            "Testing",
            "Pull Requests",
            "Code of Conduct",
        ],
        basis: "community health",
    },
    Archetype {
        id: "code-of-conduct",
        title: "Code of Conduct",
        file: "CODE_OF_CONDUCT.md",
        sections: &[
            "Our Pledge",
            "Our Standards",
            "Enforcement Responsibilities",
            "Enforcement",
        ],
        basis: "Contributor Covenant v2.1 structure",
    },
    Archetype {
        id: "governance",
        title: "Governance",
        file: "GOVERNANCE.md",
        sections: &["Roles", "Decision Process", "Membership", "Amendments"],
        basis: "CNCF / Apache patterns",
    },
    Archetype {
        id: "security",
        title: "Security Policy",
        file: "SECURITY.md",
        sections: &[
            "Supported Versions",
            "Reporting a Vulnerability",
            "Response Process",
        ],
        basis: "GitHub SECURITY.md",
    },
];

pub fn run(args: &[String], strict: bool, force: bool) -> Result<i32> {
    match args.first().map(|s| s.as_str()) {
        Some("detect") => {
            let Some(file) = args.get(1) else {
                eprintln!("usage: mari asset detect <file>");
                return Ok(2);
            };
            let text = std::fs::read_to_string(file)?;
            let root = workspace::work_root();
            match detect_type_at(&root, Path::new(file), &text) {
                Some(a) => {
                    println!("{}", a.id);
                    Ok(0)
                }
                None => {
                    println!("unknown");
                    Ok(1)
                }
            }
        }
        Some("check") => {
            let Some(file) = args.get(1) else {
                eprintln!("usage: mari asset check <file> [--strict]");
                return Ok(2);
            };
            check(Path::new(file), strict)
        }
        Some("scaffold") => {
            let Some(typ) = args.get(1) else {
                eprintln!("usage: mari asset scaffold <type> [title] [--force]");
                return Ok(2);
            };
            let title = args.get(2).map(String::as_str);
            scaffold(typ, title, force)
        }
        _ => {
            eprintln!("usage: mari asset detect <file> | check <file> [--strict] | scaffold <type> [title] [--force]");
            Ok(2)
        }
    }
}

fn check(path: &Path, strict: bool) -> Result<i32> {
    let Some(archetype) = detect_file_type(path)? else {
        eprintln!("unknown asset type: {}", path.display());
        return Ok(2);
    };
    let root = workspace::work_root();
    let findings = findings_for_archetype_at(&root, path, archetype)?;

    if findings.is_empty() {
        println!("asset: ok ({})", archetype.id);
        return Ok(0);
    }
    for f in &findings {
        println!("{} {} {}", f.severity, f.rule_id, f.path);
        println!("  {}", f.message);
    }
    if strict || findings.iter().any(|f| f.severity == "error") {
        Ok(1)
    } else {
        Ok(0)
    }
}

pub(crate) fn findings_for_path(path: &Path) -> Result<Vec<Finding>> {
    let Some(archetype) = detect_file_type(path)? else {
        return Ok(Vec::new());
    };
    let root = workspace::work_root();
    findings_for_archetype_at(&root, path, archetype)
}

fn detect_file_type(path: &Path) -> Result<Option<&'static Archetype>> {
    let text = std::fs::read_to_string(path)?;
    let root = workspace::work_root();
    Ok(detect_type_at(&root, path, &text))
}

fn findings_for_archetype_at(
    root: &Path,
    path: &Path,
    archetype: &Archetype,
) -> Result<Vec<Finding>> {
    let text = std::fs::read_to_string(path)?;
    let expected =
        override_sections(root, archetype).unwrap_or_else(|| archetype.sections.to_vec());
    let headings = headings(&text);
    let mut findings = Vec::new();
    for section in expected {
        if !headings.iter().any(|h| heading_eq(h, section)) {
            findings.push(Finding {
                rule_id: "asset-missing-section",
                severity: "error",
                asset_type: archetype.id.into(),
                path: path.display().to_string(),
                message: format!("missing required section: {section}"),
            });
        }
    }
    if archetype.id == "postmortem" {
        findings.extend(postmortem_blame(path, &text));
    }
    Ok(findings)
}

fn scaffold(typ: &str, title: Option<&str>, force: bool) -> Result<i32> {
    let archetype = archetype(typ).ok_or_else(|| anyhow!("unknown asset type: {typ}"))?;
    let root = workspace::work_root();
    scaffold_at(&root, archetype, title, force)
}

fn scaffold_at(
    root: &Path,
    archetype: &Archetype,
    title: Option<&str>,
    force: bool,
) -> Result<i32> {
    let path = root.join(archetype.file);
    if path.exists() && !force {
        return Err(anyhow!("refusing to overwrite {}", path.display()));
    }
    let text = if let Some(template) = override_template(root, archetype) {
        template.replace("{{title}}", title.unwrap_or(archetype.title))
    } else {
        built_in_template(archetype, title.unwrap_or(archetype.title))
    };
    std::fs::write(&path, text)?;
    println!("✓ wrote {}", path.display());
    Ok(0)
}

fn detect_type_at<'a>(root: &Path, path: &Path, text: &str) -> Option<&'a Archetype> {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    for a in ARCHETYPES {
        if name == a.file.to_ascii_lowercase() {
            return Some(a);
        }
    }
    let heading_text = headings(text).join(" ").to_ascii_lowercase();
    if name.contains("postmortem")
        || heading_text.contains("postmortem")
        || heading_text.contains("incident review")
    {
        return archetype("postmortem");
    }
    if name.contains("runbook") || heading_text.contains("runbook") {
        return archetype("runbook");
    }
    if name.contains("rfc") || heading_text.contains("request for comments") {
        return archetype("rfc");
    }
    if name.contains("adr") || heading_text.contains("architecture decision") {
        return archetype("adr");
    }
    if name.contains("contributing") {
        return archetype("contributing");
    }
    if name.contains("code_of_conduct") || name.contains("code-of-conduct") {
        return archetype("code-of-conduct");
    }
    if name.contains("governance") {
        return archetype("governance");
    }
    if name.contains("security") {
        return archetype("security");
    }
    ARCHETYPES
        .iter()
        .find(|a| {
            a.sections
                .iter()
                .filter(|s| headings(text).iter().any(|h| heading_eq(h, s)))
                .count()
                >= 3
        })
        .or_else(|| {
            ARCHETYPES.iter().find(|a| {
                override_sections(root, a)
                    .map(|sections| {
                        sections
                            .iter()
                            .filter(|s| headings(text).iter().any(|h| heading_eq(h, s)))
                            .count()
                            >= 2
                    })
                    .unwrap_or(false)
            })
        })
}

fn archetype(id: &str) -> Option<&'static Archetype> {
    ARCHETYPES.iter().find(|a| a.id == id)
}

fn headings(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let t = line.trim();
            let n = t.chars().take_while(|c| *c == '#').count();
            ((1..=6).contains(&n) && t[n..].starts_with(' ')).then(|| t[n..].trim().to_string())
        })
        .collect()
}

fn heading_eq(actual: &str, expected: &str) -> bool {
    normalize(actual) == normalize(expected)
}

fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn postmortem_blame(path: &Path, text: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    let blame = [
        "human error",
        "operator error",
        "careless",
        "negligent",
        "should have known",
    ];
    let lower = text.to_ascii_lowercase();
    for phrase in blame {
        if lower.contains(phrase) {
            out.push(Finding {
                rule_id: "postmortem-blame",
                severity: "error",
                asset_type: "postmortem".into(),
                path: path.display().to_string(),
                message: format!("blame language found: {phrase}"),
            });
        }
    }
    out
}

fn override_template(root: &Path, archetype: &Archetype) -> Option<String> {
    std::fs::read_to_string(template_path(root, archetype)).ok()
}

fn override_sections(root: &Path, archetype: &Archetype) -> Option<Vec<&'static str>> {
    let text = override_template(root, archetype)?;
    let sections = section_headings(&text);
    if sections.is_empty() {
        return None;
    }
    // Override checks use static built-in sections unless the template has real headings.
    // Leaking here is acceptable for this short-lived CLI process and avoids inventing
    // a second owned-section check path.
    Some(
        sections
            .into_iter()
            .map(|s| Box::leak(s.into_boxed_str()) as &'static str)
            .collect(),
    )
}

fn section_headings(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let t = line.trim();
            let n = t.chars().take_while(|c| *c == '#').count();
            ((2..=6).contains(&n) && t[n..].starts_with(' ')).then(|| t[n..].trim().to_string())
        })
        .collect()
}

fn template_path(root: &Path, archetype: &Archetype) -> PathBuf {
    root.join(".mari")
        .join("templates")
        .join(format!("{}.md", archetype.id))
}

fn built_in_template(archetype: &Archetype, title: &str) -> String {
    let mut out = format!("# {title}\n\n");
    out.push_str(&format!(
        "<!-- Archetype: {}. Basis: {}. -->\n\n",
        archetype.id, archetype.basis
    ));
    if archetype.id == "code-of-conduct" {
        out.push_str("<!-- Fill from Contributor Covenant v2.1 and keep its attribution. -->\n\n");
    }
    for section in archetype.sections {
        out.push_str(&format!("## {section}\n\n<TODO>\n\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn detects_missing_sections() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("POSTMORTEM.md");
        std::fs::write(&path, "# Incident Postmortem\n\n## Summary\n\n## Impact\n").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let a = detect_type_at(dir.path(), &path, &text).unwrap();
        assert_eq!(a.id, "postmortem");
        let expected = a.sections;
        let got = headings(&text);
        assert!(expected
            .iter()
            .any(|s| !got.iter().any(|h| heading_eq(h, s))));
    }

    #[test]
    fn postmortem_blame_fires() {
        let findings = postmortem_blame(Path::new("POSTMORTEM.md"), "Root cause was human error.");
        assert_eq!(findings[0].rule_id, "postmortem-blame");
    }

    #[test]
    fn missing_asset_operands_return_usage_exit_2() {
        assert_eq!(run(&[String::from("detect")], false, false).unwrap(), 2);
        assert_eq!(run(&[String::from("check")], false, false).unwrap(), 2);
        assert_eq!(run(&[String::from("scaffold")], false, false).unwrap(), 2);
    }

    #[test]
    fn scaffold_refuses_overwrite_without_force() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("RUNBOOK.md"), "existing").unwrap();
        let archetype = archetype("runbook").unwrap();

        let err = scaffold_at(dir.path(), archetype, Some("New Runbook"), false).unwrap_err();

        assert!(err.to_string().contains("refusing to overwrite"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("RUNBOOK.md")).unwrap(),
            "existing"
        );
    }

    #[test]
    fn scaffold_force_overwrites_existing_file() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("RUNBOOK.md"), "existing").unwrap();
        let archetype = archetype("runbook").unwrap();

        scaffold_at(dir.path(), archetype, Some("New Runbook"), true).unwrap();

        let text = std::fs::read_to_string(dir.path().join("RUNBOOK.md")).unwrap();
        assert!(text.contains("# New Runbook"));
        assert_ne!(text, "existing");
    }

    #[test]
    fn scaffold_uses_workspace_template_override() {
        let dir = tempdir().unwrap();
        let template_dir = dir.path().join(".mari").join("templates");
        std::fs::create_dir_all(&template_dir).unwrap();
        std::fs::write(
            template_dir.join("runbook.md"),
            "# {{title}}\n\n## Custom Step\n\nTemplate body.\n",
        )
        .unwrap();
        let archetype = archetype("runbook").unwrap();

        scaffold_at(dir.path(), archetype, Some("Custom Runbook"), false).unwrap();

        let text = std::fs::read_to_string(dir.path().join("RUNBOOK.md")).unwrap();
        assert_eq!(
            text,
            "# Custom Runbook\n\n## Custom Step\n\nTemplate body.\n"
        );
    }

    #[test]
    fn check_uses_workspace_template_sections() {
        let dir = tempdir().unwrap();
        let template_dir = dir.path().join(".mari").join("templates");
        std::fs::create_dir_all(&template_dir).unwrap();
        std::fs::write(
            template_dir.join("runbook.md"),
            "# {{title}}\n\n## Custom Step\n\n## Verification\n",
        )
        .unwrap();
        let path = dir.path().join("RUNBOOK.md");
        std::fs::write(&path, "# Runbook\n\n## Custom Step\n").unwrap();
        let archetype = archetype("runbook").unwrap();

        let findings = findings_for_archetype_at(dir.path(), &path, archetype).unwrap();

        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].message,
            "missing required section: Verification"
        );
    }

    #[test]
    fn detect_uses_workspace_template_sections() {
        let dir = tempdir().unwrap();
        let template_dir = dir.path().join(".mari").join("templates");
        std::fs::create_dir_all(&template_dir).unwrap();
        std::fs::write(
            template_dir.join("runbook.md"),
            "# {{title}}\n\n## Custom Step\n\n## Verification\n",
        )
        .unwrap();
        let path = dir.path().join("OPERATIONS.md");
        let text = "# Operations\n\n## Custom Step\n\n## Verification\n";
        std::fs::write(&path, text).unwrap();

        let archetype = detect_type_at(dir.path(), &path, text).unwrap();

        assert_eq!(archetype.id, "runbook");
    }
}
