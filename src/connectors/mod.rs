//! Connector registry and tracked-ref config (SPEC §6 / §22).

pub mod cloud;
pub mod gitlog;

use crate::{config, workspace};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

fn list_keys(source: &str) -> Option<&'static [&'static str]> {
    match source {
        "slack" => Some(&["slack.channels"]),
        "gdocs" | "google" => Some(&["google.docs", "google.folders"]),
        "github" => Some(&["github.repos"]),
        "git" => Some(&["git.repos"]),
        "confluence" => Some(&["confluence.spaces", "confluence.pages"]),
        "jira" => Some(&["jira.projects"]),
        "zendesk" => Some(&["zendesk.include"]),
        "salesforce" => Some(&["salesforce.objects"]),
        "hubspot" => Some(&["hubspot.include"]),
        "microsoft" => Some(&["microsoft.drives", "microsoft.mail", "microsoft.teams"]),
        "discord" => Some(&["discord.channels", "discord.guilds"]),
        "linear" => Some(&["linear.teams", "linear.projects"]),
        "localfiles" => Some(&["localfiles.paths"]),
        _ => None,
    }
}

pub fn track(args: &[String], list_key: Option<&str>) -> Result<i32> {
    track_at(&workspace::work_root(), args, list_key)
}

fn track_at(root: &Path, args: &[String], list_key: Option<&str>) -> Result<i32> {
    let Some(source) = args.first().map(|s| s.as_str()) else {
        eprintln!("usage: mari track <source> <add|remove|list> [ref] [--list-key <key>]");
        return Ok(2);
    };
    let Some(keys) = list_keys(source) else {
        eprintln!("unknown source: {source}");
        return Ok(2);
    };
    let action = args.get(1).map(|s| s.as_str()).unwrap_or("list");
    let Some(key) = select_list_key(keys, list_key) else {
        eprintln!(
            "unknown list key for {source}: {}; valid keys: {}",
            list_key.unwrap_or(""),
            keys.join(", ")
        );
        return Ok(2);
    };
    match action {
        "list" => {
            let cfg = config::resolve(Some(root));
            for k in keys {
                let vals = config::get_path(&cfg, k)
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                if vals.is_empty() {
                    println!("{k}: (none)");
                } else {
                    for v in vals {
                        println!("{k}: {}", v.as_str().unwrap_or(&v.to_string()));
                    }
                }
            }
            Ok(0)
        }
        "add" | "remove" => {
            let Some(r) = args.get(2) else {
                eprintln!("usage: mari track {source} {action} <ref>");
                return Ok(2);
            };
            mutate_array_at(root, key, r, action == "add")?;
            if action == "add" {
                println!("✓ tracked {source}: {r}");
            } else {
                println!("✓ untracked {source}: {r}");
            }
            Ok(0)
        }
        _ => {
            eprintln!("usage: mari track <source> <add|remove|list> [ref]");
            Ok(2)
        }
    }
}

fn select_list_key<'a>(keys: &'a [&'a str], requested: Option<&str>) -> Option<&'a str> {
    let Some(requested) = requested else {
        return keys.first().copied();
    };
    if let Some(key) = keys.iter().copied().find(|key| *key == requested) {
        return Some(key);
    }
    let suffix_matches: Vec<&str> = keys
        .iter()
        .copied()
        .filter(|key| key.rsplit('.').next() == Some(requested))
        .collect();
    if suffix_matches.len() == 1 {
        Some(suffix_matches[0])
    } else {
        None
    }
}

fn mutate_array_at(root: &Path, dotted: &str, item: &str, add: bool) -> Result<()> {
    let path = config::repo_config_path(root);
    std::fs::create_dir_all(path.parent().unwrap())?;
    let mut cfg = config::read_json(&path);
    let mut arr = config::get_path(&cfg, dotted)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if add {
        let val = json!(item);
        if !arr.contains(&val) {
            arr.push(val);
        }
    } else {
        arr.retain(|v| v.as_str() != Some(item));
    }
    config::set_path(&mut cfg, dotted, Value::Array(arr));
    std::fs::write(path, serde_json::to_string_pretty(&cfg)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{select_list_key, track_at};
    use crate::config;
    use tempfile::tempdir;

    #[test]
    fn list_key_defaults_to_first_key() {
        let keys = ["google.docs", "google.folders"];
        assert_eq!(select_list_key(&keys, None), Some("google.docs"));
    }

    #[test]
    fn list_key_accepts_exact_or_unique_suffix() {
        let keys = ["microsoft.drives", "microsoft.mail", "microsoft.teams"];
        assert_eq!(
            select_list_key(&keys, Some("microsoft.teams")),
            Some("microsoft.teams")
        );
        assert_eq!(select_list_key(&keys, Some("mail")), Some("microsoft.mail"));
    }

    #[test]
    fn list_key_rejects_unknown_key() {
        let keys = ["google.docs", "google.folders"];
        assert_eq!(select_list_key(&keys, Some("channels")), None);
    }

    #[test]
    fn track_rejects_unknown_source_and_list_key() {
        let dir = tempdir().unwrap();

        assert_eq!(
            track_at(
                dir.path(),
                &[String::from("bogus"), String::from("list")],
                None
            )
            .unwrap(),
            2
        );
        assert_eq!(
            track_at(
                dir.path(),
                &[String::from("gdocs"), String::from("list")],
                Some("channels")
            )
            .unwrap(),
            2
        );
    }

    #[test]
    fn track_list_and_mutation_follow_spec_contract() {
        let dir = tempdir().unwrap();

        assert_eq!(
            track_at(
                dir.path(),
                &[String::from("gdocs"), String::from("list")],
                None
            )
            .unwrap(),
            0
        );
        assert_eq!(
            track_at(
                dir.path(),
                &[
                    String::from("gdocs"),
                    String::from("add"),
                    String::from("folder-123"),
                ],
                Some("folders")
            )
            .unwrap(),
            0
        );

        let cfg = config::read_json(&config::repo_config_path(dir.path()));
        assert_eq!(
            config::get_path(&cfg, "google.folders")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str()),
            Some("folder-123")
        );

        assert_eq!(
            track_at(
                dir.path(),
                &[
                    String::from("gdocs"),
                    String::from("remove"),
                    String::from("folder-123"),
                ],
                Some("google.folders")
            )
            .unwrap(),
            0
        );
        let cfg = config::read_json(&config::repo_config_path(dir.path()));
        assert_eq!(
            config::get_path(&cfg, "google.folders")
                .and_then(|v| v.as_array())
                .map(Vec::is_empty),
            Some(true)
        );
    }
}
