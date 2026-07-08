//! Configuration per SPEC §3.3/§4: DEFAULTS → ~/.mari/config.json →
//! <repo>/.mari/config.json (→ <repo>/.mari/config.local.json, personal
//! overrides, `null` deletes). List-valued tracked refs union across layers;
//! scalars from more-personal layers win.

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

pub fn defaults() -> Value {
    json!({
        "embedding": {
            "model": "",
            "batch_size": 16,
            "auto_download": true,
            "gpu_layers": 999
        },
        "chunking": {
            "lines": 40, "overlap": 8, "min_chars": 40,
            "title_prefix": true, "large_chunks": false, "large_chunk_ratio": 4
        },
        "search": {
            "hybrid": true, "k": 8, "rrf_k": 60, "keyword_pool": 200,
            "expand": 1, "vector_weight": 1.0, "keyword_weight": 1.0,
            "auto_weight": true, "recency_decay": 0.0, "recency_floor": 0.75,
            "merge_sections": true,
            "rerank": { "enabled": false, "model": "", "pool": 40 },
            "tag_boosts": { "canonical": 1.15, "draft": 0.9, "stale": 0.7, "deprecated": 0.5 }
        },
        "sync": { "stale_days": 7 },
        "audit": { "stale_pages": false, "stale_days": 90 },
        "slack": {
            "lookback_days": 14, "channels": [],
            "chunking": { "lines": 5, "overlap": 3, "min_chars": 20 }
        },
        "discord": { "lookback_days": 14, "channels": [], "guilds": [] },
        "gdocs": { "lookback_days": 30, "comments": true },
        "google": { "docs": [], "folders": [] },
        "github": { "include": ["issues", "pulls"], "repos": [] },
        "git": {
            "repos": [],
            "chunking": { "lines": 15, "overlap": 3, "min_chars": 10 }
        },
        "confluence": { "spaces": [], "pages": [] },
        "jira": { "projects": [] },
        "zendesk": { "brands": [], "include": [] },
        "salesforce": { "objects": [] },
        "hubspot": { "include": [] },
        "microsoft": { "drives": [], "mail": [], "teams": [] },
        "linear": { "teams": [], "projects": [] },
        "localfiles": { "paths": [] },
        "ocr": { "backend": "text", "model": "baidu/Unlimited-OCR", "dpi": 200, "auto_install": true, "accept_remote_code": false },
        "cloud": { "enabled": false, "backend": "s3", "bucket": "", "prefix": "", "region": "" },
        // Storage / read layer (§4.4/§8.8): the catalog IS a set of Iceberg
        // tables that every read and write goes through — there is no
        // catalog.duckdb master. `backend` is "local" (default: an `iceberg/`
        // dir in the workspace) or "s3" (remote reads/writes for the cloud
        // service). `path` is the s3://bucket/prefix base when backend=s3;
        // `region` feeds the duckdb s3 secret (else the AWS credential chain).
        // `cache_dir`/`cache_size` locate and bound the cache_httpfs + Lance
        // on-disk read cache; `retain_snapshots` is how many snapshots
        // compaction keeps (1 = current only); `bucket_chunks` is the Iceberg
        // bucket() partition count for chunks/spans/symbols (§8.7).
        "storage": {
            "backend": "local", "path": "", "region": "",
            "cache_dir": "", "cache_size": "",
            "retain_snapshots": 1, "bucket_chunks": 16
        },
        // Shared knowledge base (§4.4/§8.8) — HTTP-federated read-only scope. v2:
        // v1 ships storage.backend = local | s3 only, but the config surface is
        // reserved now so the v1 path doesn't foreclose it.
        "knowledge_base": { "enabled": false, "uri": "", "projects": [] },
        "detector": {
            "styleGuide": "microsoft",
            "ignoreRules": [], "ignoreFiles": [], "ignoreValues": {},
            "ignoreReasons": {}, "ignoreSpans": {}, "zeroTolerance": [], "grammar": false
        },
        "hook": { "maxFindings": 20, "grammar": false, "quiet": false },
        "rules": [],
        "nudges": [],
        "tags": {
            "statuses": ["canonical","stale","deprecated","draft","internal","customer-facing","needs-review"],
            "entries": {}
        },
        "facts": { "file": "FACTS.md" },
        "glossary": { "file": "STYLE.md" },
        "scan": {
            "google": { "docs": [], "folders": [] },
            "slack": { "channels": [], "lookbackDays": 14 }
        },
        "assoc": { "attn": 0.5 },
        "attention": { "model": "", "auto_download": true, "threshold": 0.3, "gpu_layers": 999 },
        "humanizer": { "repo": "" }
    })
}

pub fn mari_home() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".mari")
}

pub fn global_config_path() -> PathBuf {
    mari_home().join("config.json")
}

pub fn repo_config_path(repo: &Path) -> PathBuf {
    repo.join(".mari").join("config.json")
}

pub fn repo_local_config_path(repo: &Path) -> PathBuf {
    repo.join(".mari").join("config.local.json")
}

pub fn read_json(path: &Path) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Object(Map::new()))
}

/// Deep merge `overlay` onto `base`; `null` in overlay deletes the key;
/// tracked-ref list keys UNION across layers (SPEC §3.3).
pub fn deep_merge(base: &mut Value, overlay: &Value, union_lists: bool) {
    deep_merge_at(base, overlay, union_lists, &mut Vec::new());
}

fn deep_merge_at(base: &mut Value, overlay: &Value, union_lists: bool, path: &mut Vec<String>) {
    match (base, overlay) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                path.push(k.clone());
                if v.is_null() {
                    b.remove(k);
                } else if let Some(bv) = b.get_mut(k) {
                    if bv.is_object() && v.is_object() {
                        deep_merge_at(bv, v, union_lists, path);
                    } else if union_lists
                        && bv.is_array()
                        && v.is_array()
                        && is_tracked_ref_path(path)
                    {
                        let arr = bv.as_array_mut().unwrap();
                        for item in v.as_array().unwrap() {
                            if !arr.contains(item) {
                                arr.push(item.clone());
                            }
                        }
                    } else {
                        *bv = v.clone();
                    }
                } else {
                    b.insert(k.clone(), v.clone());
                }
                path.pop();
            }
        }
        (b, o) => *b = o.clone(),
    }
}

fn is_tracked_ref_path(path: &[String]) -> bool {
    is_tracked_ref_dotted(&dotted_path(path))
}

pub fn is_tracked_ref_dotted(dotted: &str) -> bool {
    matches!(
        dotted,
        "slack.channels"
            | "google.docs"
            | "google.folders"
            | "github.repos"
            | "git.repos"
            | "confluence.spaces"
            | "confluence.pages"
            | "jira.projects"
            | "zendesk.include"
            | "salesforce.objects"
            | "hubspot.include"
            | "microsoft.drives"
            | "microsoft.mail"
            | "microsoft.teams"
            | "discord.channels"
            | "discord.guilds"
            | "linear.teams"
            | "linear.projects"
            | "localfiles.paths"
    )
}

fn dotted_path(path: &[String]) -> String {
    path.join(".")
}

/// Effective config for a repo (or global-only when repo is None).
pub fn resolve(repo: Option<&Path>) -> Value {
    let mut cfg = defaults();
    deep_merge(&mut cfg, &read_json(&global_config_path()), true);
    if let Some(r) = repo {
        deep_merge(&mut cfg, &read_json(&repo_config_path(r)), true);
        deep_merge(&mut cfg, &read_json(&repo_local_config_path(r)), true);
    }
    cfg
}

pub fn get_path<'a>(v: &'a Value, dotted: &str) -> Option<&'a Value> {
    let mut cur = v;
    for part in dotted.split('.') {
        cur = cur.get(part)?;
    }
    Some(cur)
}

/// All known dotted paths in the defaults tree (leaf paths).
pub fn known_paths() -> Vec<String> {
    fn walk(prefix: &str, v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(m) => {
                for (k, val) in m {
                    let p = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{prefix}.{k}")
                    };
                    walk(&p, val, out);
                }
            }
            _ => out.push(prefix.to_string()),
        }
    }
    let mut out = Vec::new();
    walk("", &defaults(), &mut out);
    out.sort();
    out
}

/// Coerce a CLI string to the type of the default at that path
/// (booleans accept 1/true/yes/on).
pub fn coerce(dotted: &str, raw: &str) -> Result<Value> {
    let d = defaults();
    let target = get_path(&d, dotted).ok_or_else(|| anyhow!("unknown config path: {dotted}"))?;
    Ok(match target {
        Value::Bool(_) => {
            let t = match raw.to_lowercase().as_str() {
                "1" | "true" | "yes" | "on" => true,
                "0" | "false" | "no" | "off" => false,
                _ => {
                    return Err(anyhow!(
                        "expected a boolean (true/false, yes/no, on/off, 1/0)"
                    ))
                }
            };
            Value::Bool(t)
        }
        Value::Number(n) if n.is_i64() || n.is_u64() => {
            Value::Number(raw.parse::<i64>().context("expected an integer")?.into())
        }
        Value::Number(_) => json!(raw.parse::<f64>().context("expected a number")?),
        Value::Array(_) | Value::Object(_) => {
            serde_json::from_str(raw).context("expected JSON for this path")?
        }
        _ => Value::String(raw.to_string()),
    })
}

/// Write a dotted path into the GLOBAL config file (SPEC: `mari config set`
/// writes to ~/.mari/config.json).
pub fn set_global(dotted: &str, value: Value) -> Result<()> {
    let path = global_config_path();
    std::fs::create_dir_all(path.parent().unwrap())?;
    let mut cfg = read_json(&path);
    set_path(&mut cfg, dotted, value);
    std::fs::write(&path, serde_json::to_string_pretty(&cfg)?)?;
    Ok(())
}

/// Write a dotted path into a specific JSON file (e.g. committed repo config).
pub fn set_in_file(path: &Path, dotted: &str, value: Value) -> Result<()> {
    std::fs::create_dir_all(path.parent().unwrap())?;
    let mut cfg = read_json(path);
    set_path(&mut cfg, dotted, value);
    std::fs::write(path, serde_json::to_string_pretty(&cfg)?)?;
    Ok(())
}

pub fn set_path(root: &mut Value, dotted: &str, value: Value) {
    let parts: Vec<&str> = dotted.split('.').collect();
    let mut cur = root;
    for part in &parts[..parts.len() - 1] {
        if !cur.is_object() {
            *cur = Value::Object(Map::new());
        }
        let obj = cur.as_object_mut().unwrap();
        cur = obj
            .entry(part.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    if !cur.is_object() {
        *cur = Value::Object(Map::new());
    }
    cur.as_object_mut()
        .unwrap()
        .insert(parts.last().unwrap().to_string(), value);
}

/// True when a changed path requires a `mari sync --rebuild` reminder.
pub fn needs_rebuild_reminder(dotted: &str) -> bool {
    dotted.starts_with("embedding.")
        || dotted == "chunking"
        || dotted.starts_with("chunking.")
        || dotted.contains(".chunking")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_merge_unions_only_tracked_ref_lists() {
        let mut base = json!({
            "github": {
                "repos": ["acme/one"],
                "include": ["issues", "pulls"]
            },
            "detector": {
                "ignoreRules": ["old-rule"]
            },
            "tags": {
                "statuses": ["canonical", "stale"]
            }
        });
        let overlay = json!({
            "github": {
                "repos": ["acme/two"],
                "include": ["issues"]
            },
            "detector": {
                "ignoreRules": ["new-rule"]
            },
            "tags": {
                "statuses": ["canonical", "draft"]
            }
        });

        deep_merge(&mut base, &overlay, true);

        assert_eq!(base["github"]["repos"], json!(["acme/one", "acme/two"]));
        assert_eq!(base["github"]["include"], json!(["issues"]));
        assert_eq!(base["detector"]["ignoreRules"], json!(["new-rule"]));
        assert_eq!(base["tags"]["statuses"], json!(["canonical", "draft"]));
    }

    #[test]
    fn deep_merge_null_deletes_key() {
        let mut base = json!({
            "search": {
                "k": 8,
                "tag_boosts": { "draft": 0.9 }
            }
        });
        let overlay = json!({
            "search": {
                "tag_boosts": null
            }
        });

        deep_merge(&mut base, &overlay, true);

        assert_eq!(base["search"]["k"], json!(8));
        assert!(base["search"].get("tag_boosts").is_none());
    }

    #[test]
    fn coerce_bool_accepts_explicit_forms_only() {
        assert_eq!(coerce("search.hybrid", "yes").unwrap(), json!(true));
        assert_eq!(coerce("search.hybrid", "off").unwrap(), json!(false));
        assert!(coerce("search.hybrid", "tru").is_err());
    }

    #[test]
    fn embedding_model_override_is_registered_and_empty_by_default() {
        // Default is empty so the built-in Qwen3-Embedding-0.6B identity is
        // used; a non-empty value is a path override (air-gapped installs).
        assert_eq!(
            get_path(&defaults(), "embedding.model").and_then(|v| v.as_str()),
            Some("")
        );
        assert!(known_paths().contains(&"embedding.model".to_string()));
        assert_eq!(
            coerce("embedding.model", "/models/custom.gguf").unwrap(),
            json!("/models/custom.gguf")
        );
    }
}
