//! `mari config [get PATH | set PATH VALUE | list] [--json]` per SPEC §5.1.

use crate::{config, workspace};
use anyhow::Result;

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
                        println!("{p} = {v}");
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
