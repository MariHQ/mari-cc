//! Assistant-guided editorial setup.

use crate::workspace;
use anyhow::Result;

pub fn run(which: Option<&str>) -> Result<i32> {
    if !matches!(which, None | Some("style") | Some("all")) {
        eprintln!(
            "unknown init target: {} (style | all)",
            which.unwrap_or_default()
        );
        return Ok(2);
    }
    let root = workspace::work_root();
    println!("Mari editorial setup\n");
    if root.join("PRODUCT.md").exists() {
        println!("[x] PRODUCT.md exists — editorial context is configured.");
    } else {
        println!("[ ] PRODUCT.md missing. Ask for the audience, register, voice, and banned words, then create it.");
    }
    if root.join("STYLE.md").exists() {
        println!("[x] STYLE.md exists.");
    } else {
        println!("[ ] STYLE.md missing — offer to create terminology and formatting guidance.");
    }
    let hook_installed = std::fs::read_to_string(root.join(".claude/settings.json"))
        .map(|text| text.contains("mari hook"))
        .unwrap_or(false);
    println!(
        "[{}] post-edit hook {}.",
        if hook_installed { "x" } else { " " },
        if hook_installed {
            "installed"
        } else {
            "not installed — offer `mari hooks on`"
        }
    );
    println!("[ ] offer `mari rules discover` to propose edit-notify rules.");
    Ok(0)
}
