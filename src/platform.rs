//! Doc-platform detection and scaffolding (SPEC §5.6).

use crate::workspace;
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::path::Path;

#[derive(Clone, Copy, Serialize)]
struct Platform {
    id: &'static str,
    name: &'static str,
    scaffoldable: bool,
    markers: &'static [&'static str],
}

const PLATFORMS: &[Platform] = &[
    p("mkdocs", "MkDocs", true, &["mkdocs.yml", "mkdocs.yaml"]),
    p(
        "docusaurus",
        "Docusaurus",
        true,
        &["docusaurus.config.js", "docusaurus.config.ts"],
    ),
    p("sphinx", "Sphinx", true, &["docs/conf.py", "conf.py"]),
    p(
        "hugo",
        "Hugo",
        true,
        &["hugo.toml", "hugo.yaml", "config.toml", "config.yaml"],
    ),
    p("jekyll", "Jekyll", true, &["_config.yml", "_config.yaml"]),
    p("mdbook", "mdBook", true, &["book.toml"]),
    p(
        "antora",
        "Antora",
        true,
        &["antora.yml", "antora-playbook.yml"],
    ),
    p("docsify", "Docsify", true, &["index.html", ".nojekyll"]),
    p(
        "vitepress",
        "VitePress",
        false,
        &["docs/.vitepress/config.ts", ".vitepress/config.ts"],
    ),
    p(
        "starlight",
        "Starlight",
        false,
        &["astro.config.mjs", "astro.config.ts"],
    ),
    p(
        "gitbook",
        "GitBook",
        false,
        &[".gitbook.yaml", "SUMMARY.md"],
    ),
    p(
        "readthedocs",
        "Read the Docs",
        false,
        &[".readthedocs.yaml", "readthedocs.yml"],
    ),
];

const fn p(
    id: &'static str,
    name: &'static str,
    scaffoldable: bool,
    markers: &'static [&'static str],
) -> Platform {
    Platform {
        id,
        name,
        scaffoldable,
        markers,
    }
}

#[derive(Serialize)]
struct Detection {
    id: &'static str,
    name: &'static str,
    scaffoldable: bool,
    marker: String,
}

pub fn run(args: &[String], json: bool, name: Option<&str>, force: bool) -> Result<i32> {
    match args.first().map(|s| s.as_str()) {
        None | Some("detect") => detect(json),
        Some("list") => list(json),
        Some("scaffold") => {
            let id = args.get(1).ok_or_else(|| {
                anyhow!("usage: mari platform scaffold <id> [--name \"Title\"] [--force]")
            })?;
            scaffold(id, name, force)
        }
        Some(other) => {
            eprintln!("unknown platform subcommand: {other}");
            Ok(2)
        }
    }
}

fn detect(json: bool) -> Result<i32> {
    let found = detections(&workspace::work_root());
    if json {
        println!("{}", serde_json::to_string_pretty(&found)?);
    } else if found.is_empty() {
        println!("none");
    } else {
        for d in &found {
            let mode = if d.scaffoldable {
                "scaffoldable"
            } else {
                "detect-only"
            };
            println!("{}  {}  {}", d.id, mode, d.marker);
        }
    }
    Ok(if found.is_empty() { 1 } else { 0 })
}

fn list(json: bool) -> Result<i32> {
    if json {
        println!("{}", serde_json::to_string_pretty(PLATFORMS)?);
    } else {
        for p in PLATFORMS {
            println!(
                "{:<12} {:<16} {}",
                p.id,
                if p.scaffoldable {
                    "scaffoldable"
                } else {
                    "detect-only"
                },
                p.name
            );
        }
    }
    Ok(0)
}

fn scaffold(id: &str, name: Option<&str>, force: bool) -> Result<i32> {
    let platform = PLATFORMS
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| anyhow!("unknown platform: {id}"))?;
    if !platform.scaffoldable {
        return Err(anyhow!("{id} is detect-only and cannot be scaffolded"));
    }
    let root = workspace::work_root();
    let existing = detections(&root);
    if !existing.is_empty() && !force {
        let names = existing.iter().map(|d| d.id).collect::<Vec<_>>().join(", ");
        return Err(anyhow!(
            "refusing to scaffold a second docs platform; detected {names}. Use --force to override"
        ));
    }
    write_scaffold(&root, platform, name.unwrap_or("Mari Docs"), force)?;
    println!("✓ scaffolded {}", platform.id);
    Ok(0)
}

fn detections(root: &Path) -> Vec<Detection> {
    let mut out = Vec::new();
    for platform in PLATFORMS {
        for marker in platform.markers {
            let path = root.join(marker);
            if path.exists() {
                out.push(Detection {
                    id: platform.id,
                    name: platform.name,
                    scaffoldable: platform.scaffoldable,
                    marker: marker.to_string(),
                });
                break;
            }
        }
    }
    out
}

fn write_scaffold(root: &Path, platform: &Platform, title: &str, force: bool) -> Result<()> {
    match platform.id {
        "mkdocs" => write_many(
            root,
            force,
            &[
                ("mkdocs.yml", format!("site_name: {title}\nnav:\n  - Home: docs/index.md\n")),
                ("docs/index.md", format!("# {title}\n\n<TODO>\n")),
            ],
        ),
        "docusaurus" => write_many(
            root,
            force,
            &[
                (
                    "docusaurus.config.js",
                    format!("module.exports = {{ title: {title:?}, url: 'https://example.com', baseUrl: '/' }};\n"),
                ),
                ("docs/intro.md", format!("# {title}\n\n<TODO>\n")),
            ],
        ),
        "sphinx" => write_many(
            root,
            force,
            &[
                ("docs/conf.py", format!("project = {title:?}\nextensions = []\n")),
                ("docs/index.rst", format!("{title}\n{}\n\n.. toctree::\n   :maxdepth: 2\n\n", "=".repeat(title.len()))),
            ],
        ),
        "hugo" => write_many(
            root,
            force,
            &[
                ("hugo.toml", format!("title = {title:?}\n")),
                ("content/_index.md", format!("---\ntitle: {title:?}\n---\n\n<TODO>\n")),
            ],
        ),
        "jekyll" => write_many(
            root,
            force,
            &[
                ("_config.yml", format!("title: {title}\n")),
                ("index.md", format!("---\ntitle: {title}\n---\n\n# {title}\n\n<TODO>\n")),
            ],
        ),
        "mdbook" => write_many(
            root,
            force,
            &[
                ("book.toml", format!("[book]\ntitle = {title:?}\n")),
                ("src/SUMMARY.md", "# Summary\n\n- [Home](./index.md)\n".into()),
                ("src/index.md", format!("# {title}\n\n<TODO>\n")),
            ],
        ),
        "antora" => write_many(
            root,
            force,
            &[
                ("antora.yml", format!("name: docs\ntitle: {title}\nversion: ~\nstart_page: ROOT:index.adoc\n")),
                ("modules/ROOT/pages/index.adoc", format!("= {title}\n\nTODO\n")),
            ],
        ),
        "docsify" => write_many(
            root,
            force,
            &[
                ("index.html", docsify_index(title)),
                ("README.md", format!("# {title}\n\n<TODO>\n")),
                (".nojekyll", String::new()),
            ],
        ),
        _ => Err(anyhow!("unsupported scaffold platform: {}", platform.id)),
    }
}

fn write_many(root: &Path, force: bool, files: &[(&str, String)]) -> Result<()> {
    for (rel, _) in files {
        let path = root.join(rel);
        if path.exists() && !force {
            return Err(anyhow!("refusing to overwrite {}", path.display()));
        }
    }
    for (rel, content) in files {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
    }
    Ok(())
}

fn docsify_index(title: &str) -> String {
    format!(
        r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <title>{title}</title>
</head>
<body>
  <div id="app"></div>
  <script>
    window.$docsify = {{ name: {title:?}, repo: '' }};
  </script>
  <script src="//cdn.jsdelivr.net/npm/docsify@4"></script>
</body>
</html>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn detects_mkdocs_marker() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("mkdocs.yml"), "site_name: Test\n").unwrap();
        let found = detections(dir.path());
        assert_eq!(found[0].id, "mkdocs");
    }

    #[test]
    fn write_many_refuses_overwrite() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("book.toml"), "old").unwrap();
        let err = write_many(dir.path(), false, &[("book.toml", "new".into())]).unwrap_err();
        assert!(err.to_string().contains("refusing to overwrite"));
    }
}
