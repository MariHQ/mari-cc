//! Repository-local configuration: DEFAULTS → <repo>/.mari/config.json →
//! <repo>/.mari/config.local.json. Scalars from the local override win.

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

pub fn defaults() -> Value {
    json!({
        "detector": {
            "styleGuide": "microsoft",
            "ignoreRules": [], "ignoreFiles": [], "ignoreValues": {},
            "ignoreReasons": {}, "ignoreSpans": {}, "zeroTolerance": [], "grammar": false,
            "lists": {}
        },
        "hook": { "maxFindings": 20, "grammar": false, "quiet": false },
        "rules": [],
        "nudges": [],
        "glossary": { "file": "STYLE.md" }
    })
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

/// Deep merge `overlay` onto `base`; `null` in the overlay deletes a key.
pub fn deep_merge(base: &mut Value, overlay: &Value) {
    match (base, overlay) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                if v.is_null() {
                    b.remove(k);
                } else if let Some(bv) = b.get_mut(k) {
                    if bv.is_object() && v.is_object() {
                        deep_merge(bv, v);
                    } else {
                        *bv = v.clone();
                    }
                } else {
                    b.insert(k.clone(), v.clone());
                }
            }
        }
        (b, o) => *b = o.clone(),
    }
}

/// Effective config for a repo (or global-only when repo is None).
pub fn resolve(repo: Option<&Path>) -> Value {
    let mut cfg = defaults();
    if let Some(r) = repo {
        deep_merge(&mut cfg, &read_json(&repo_config_path(r)));
        deep_merge(&mut cfg, &read_json(&repo_local_config_path(r)));
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

/// Write a dotted path into the repository's `.mari/config.json`.
pub fn set_repo(repo: &Path, dotted: &str, value: Value) -> Result<()> {
    let path = repo_config_path(repo);
    std::fs::create_dir_all(path.parent().unwrap())?;
    let mut cfg = read_json(&path);
    set_path(&mut cfg, dotted, value);
    std::fs::write(&path, serde_json::to_string_pretty(&cfg)?)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_merge_replaces_lists() {
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

        deep_merge(&mut base, &overlay);

        assert_eq!(base["github"]["repos"], json!(["acme/two"]));
        assert_eq!(base["github"]["include"], json!(["issues"]));
        assert_eq!(base["detector"]["ignoreRules"], json!(["new-rule"]));
        assert_eq!(base["tags"]["statuses"], json!(["canonical", "draft"]));
    }

    #[test]
    fn deep_merge_null_deletes_key() {
        let mut base = json!({ "detector": { "grammar": false, "lists": {} } });
        let overlay = json!({ "detector": { "lists": null } });

        deep_merge(&mut base, &overlay);

        assert_eq!(base["detector"]["grammar"], json!(false));
        assert!(base["detector"].get("lists").is_none());
    }

    #[test]
    fn coerce_bool_accepts_explicit_forms_only() {
        assert_eq!(coerce("detector.grammar", "yes").unwrap(), json!(true));
        assert_eq!(coerce("detector.grammar", "off").unwrap(), json!(false));
        assert!(coerce("detector.grammar", "tru").is_err());
    }
}
