//! Workspace identity and paths per SPEC §3.1:
//! `~/.mari/<repo-slug>-<first-8-hex-of-hash(abs-path)>/`.

use crate::config;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Walk up from cwd for a `.git` directory; None when not in a repo.
pub fn repo_root() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir.join(".git").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Repo root, else cwd (Mari works outside git too; the workspace hash is
/// keyed on the absolute path either way).
pub fn work_root() -> PathBuf {
    repo_root().unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

pub fn workspace_id(root: &Path) -> String {
    let abs = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let slug = abs
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| "repo".into())
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>();
    let mut h = Sha256::new();
    h.update(abs.to_string_lossy().as_bytes());
    let hex = format!("{:x}", h.finalize());
    format!("{}-{}", slug, &hex[..8])
}

/// Personal workspace dir for this repo (created on demand).
pub fn workspace_dir(root: &Path) -> PathBuf {
    config::mari_home().join(workspace_id(root))
}

/// Workspace for globally-scoped connectors.
pub fn global_workspace_dir() -> PathBuf {
    config::mari_home().join("_global")
}

pub fn ensure_dir(p: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(p)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

/// Credentials dir for a scope; files must be 0600 (SPEC §1.1).
pub fn credentials_dir(global: bool, root: &Path) -> PathBuf {
    if global {
        config::mari_home().join("credentials")
    } else {
        workspace_dir(root).join("credentials")
    }
}

pub fn write_credential(path: &Path, value: &serde_json::Value) -> anyhow::Result<()> {
    ensure_dir(path.parent().unwrap())?;
    std::fs::write(path, serde_json::to_string_pretty(value)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Per-source scope map at ~/.mari/scopes.json; default per SPEC §6.
pub fn source_scope(source: &str) -> String {
    let scopes = config::read_json(&config::mari_home().join("scopes.json"));
    if let Some(s) = scopes.get(source).and_then(|v| v.as_str()) {
        return s.to_string();
    }
    default_scope(source).to_string()
}

pub fn default_scope(source: &str) -> &'static str {
    match source {
        "slack" | "gdocs" | "zendesk" | "salesforce" | "hubspot" | "microsoft" | "discord" => {
            "global"
        }
        _ => "local",
    }
}

pub fn set_source_scope(source: &str, scope: &str) -> anyhow::Result<()> {
    let path = config::mari_home().join("scopes.json");
    ensure_dir(path.parent().unwrap())?;
    let mut scopes = config::read_json(&path);
    config::set_path(&mut scopes, source, serde_json::json!(scope));
    std::fs::write(&path, serde_json::to_string_pretty(&scopes)?)?;
    Ok(())
}
