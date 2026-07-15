//! Repository-local detector and hook status.

use crate::{config, workspace};
use anyhow::Result;

pub fn run() -> Result<i32> {
    let root = workspace::work_root();
    let cfg = config::resolve(Some(&root));
    let style = cfg["detector"]["styleGuide"]
        .as_str()
        .unwrap_or("microsoft");
    let hook_installed = std::fs::read_to_string(root.join(".claude/settings.json"))
        .map(|text| text.contains("mari hook"))
        .unwrap_or(false);
    println!("repository: {}", root.display());
    println!("config: {}", config::repo_config_path(&root).display());
    println!(
        "detector: style={style}  rules={}  {} word lists  hook={}",
        crate::detector::registry().len(),
        crate::detector::lists::registry().len(),
        if hook_installed {
            "installed"
        } else {
            "not installed"
        }
    );
    Ok(0)
}

pub fn doctor() -> Result<i32> {
    println!("mari doctor\n");
    println!("  detector      ready");
    println!("  grammar       ready (Harper, compiled in)");
    println!(
        "  word lists    {}",
        crate::detector::lists::registry().len()
    );
    Ok(0)
}
