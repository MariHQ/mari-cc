//! `mari config [get PATH | set PATH VALUE | list] [--json]` per SPEC §5.1.

use crate::{config, workspace};
use anyhow::Result;
use serde_json::Value;
use std::path::Path;

pub fn run(
    action: Option<&str>,
    path: Option<&str>,
    value: Option<&str>,
    json: bool,
) -> Result<i32> {
    let root = workspace::work_root();
    match action {
        None | Some("list") => {
            let resolved = config::resolve(Some(&root));
            if json {
                println!("{}", serde_json::to_string_pretty(&resolved)?);
            } else {
                println!(
                    "# effective config (DEFAULTS → ~/.mari/config.json → {}/.mari/config.json → config.local.json)",
                    root.display()
                );
                for p in config::known_paths() {
                    if let Some(v) = config::get_path(&resolved, &p) {
                        println!("{}", format_config_line(&p, v, &root));
                    }
                }
            }
            Ok(0)
        }
        Some("get") => {
            let Some(p) = path else {
                eprintln!("usage: mari config get <dotted.path>");
                return Ok(2);
            };
            let resolved = config::resolve(Some(&root));
            match config::get_path(&resolved, p) {
                Some(v) => {
                    println!("{}", serde_json::to_string_pretty(v)?);
                    Ok(0)
                }
                None => {
                    unknown_path(p);
                    Ok(2)
                }
            }
        }
        Some("set") => {
            let (Some(p), Some(raw)) = (path, value) else {
                eprintln!("usage: mari config set <dotted.path> <value>");
                return Ok(2);
            };
            if p == "embedding.model" && raw != "jina-embeddings-v5-text-nano" {
                eprintln!(
                    "✗ embedding.model must be jina-embeddings-v5-text-nano; no other embedding identity is supported"
                );
                return Ok(2);
            }
            if !config::known_paths().iter().any(|k| k == p) {
                // Allow subtree paths that exist as objects (e.g. slack.chunking).
                let d = config::defaults();
                if config::get_path(&d, p).is_none() {
                    unknown_path(p);
                    return Ok(2);
                }
            }
            let coerced = config::coerce(p, raw)?;
            config::set_global(p, coerced.clone())?;
            println!("✓ {p} = {coerced}");
            if config::needs_rebuild_reminder(p) {
                println!("note: this changes indexing — run `mari sync --rebuild` to re-embed.");
            }
            Ok(0)
        }
        Some(other) => {
            eprintln!("unknown config action: {other}");
            Ok(2)
        }
    }
}

fn unknown_path(p: &str) {
    eprintln!("unknown config path: {p}\nknown paths:");
    for k in config::known_paths() {
        eprintln!("  {k}");
    }
}

fn format_config_line(path: &str, value: &Value, root: &Path) -> String {
    format!(
        "{path} = {value}  # source: {}; set: {}",
        effective_source(path, root),
        config::global_config_path().display()
    )
}

fn effective_source(path: &str, root: &Path) -> String {
    let mut layers = Vec::new();
    let defaults = config::defaults();
    if config::get_path(&defaults, path).is_some() {
        layers.push("defaults");
    }
    let global = config::read_json(&config::global_config_path());
    if config::get_path(&global, path).is_some() {
        layers.push("global");
    }
    let repo = config::read_json(&config::repo_config_path(root));
    if config::get_path(&repo, path).is_some() {
        layers.push("repo");
    }
    let local = config::read_json(&config::repo_local_config_path(root));
    if config::get_path(&local, path).is_some() {
        layers.push("repo-local");
    }
    source_label(&layers, config::is_tracked_ref_dotted(path))
}

fn source_label(layers: &[&str], union_list: bool) -> String {
    if layers.is_empty() {
        return "unknown".into();
    }
    if union_list {
        layers.join("+")
    } else {
        layers.last().unwrap().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{run, source_label};

    #[test]
    fn scalar_source_label_uses_last_overlay() {
        assert_eq!(source_label(&["defaults", "global", "repo"], false), "repo");
    }

    #[test]
    fn tracked_list_source_label_keeps_all_contributors() {
        assert_eq!(
            source_label(&["defaults", "global", "repo"], true),
            "defaults+global+repo"
        );
    }

    #[test]
    fn config_rejects_non_jina_embedding_model_identity() {
        assert_eq!(
            run(
                Some("set"),
                Some("embedding.model"),
                Some("other-model"),
                false
            )
            .unwrap(),
            2
        );
    }
}
