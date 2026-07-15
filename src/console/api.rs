//! JSON API for the repository-local console.

use crate::{assets, config, curation, detector, i18n, rulescmd, workspace};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tiny_http::Method;

pub struct Ctx {
    pub method: Method,
    pub path: String,
    pub query: String,
    pub body: String,
}

pub fn runtime_config() -> Value {
    json!({ "apiBase": "", "marketingUrl": "", "local": true })
}

pub fn auth(path: &str) -> Value {
    if path == "/auth/logout" {
        return json!({ "ok": true });
    }
    json!({
        "authenticated": true,
        "user": { "login": "local", "name": "Local workspace" },
        "orgSlug": "local",
        "orgName": "Local workspace",
        "installations": [],
    })
}

pub fn route(ctx: &Ctx) -> Result<(u16, Value)> {
    let segments: Vec<&str> = ctx.path.trim_matches('/').split('/').collect();
    let rest = &segments[1..];
    let value = match (&ctx.method, rest) {
        (Method::Get, ["overview"]) => json!({
            "rules": detector::registry().len(),
            "wordLists": detector::lists::registry().len(),
        }),
        (Method::Get, ["glossary"]) => glossary(),
        (Method::Get, ["config"]) => config_get(),
        (Method::Put, ["config"]) => config_set(ctx)?,
        (Method::Get, ["nudges"]) => nudges_list(),
        (Method::Post, ["nudges"]) => nudges_add(ctx)?,
        (Method::Delete, ["nudges"]) => nudges_remove(ctx)?,
        (Method::Get, ["rules"]) => rules_list(),
        (Method::Post, ["rules"]) => rules_add(ctx)?,
        (Method::Post, ["rules", "discover"]) => rules_discover()?,
        (Method::Delete, ["rules"]) => rules_remove(ctx)?,
        (Method::Get, ["detector"]) => detector_get(),
        (Method::Post, ["detector", "zero"]) => detector_zero(ctx)?,
        (Method::Post, ["detector", "ignore"]) => detector_ignore(ctx)?,
        (Method::Get, ["detector", "lists"]) => detector_lists_get(),
        (Method::Put, ["detector", "lists"]) => detector_lists_set(ctx)?,
        (Method::Post, ["detect"]) => detect(ctx)?,
        (Method::Get, ["templates"]) => json!({ "templates": assets::archetypes() }),
        (Method::Post, ["templates", "scaffold"]) => templates_scaffold(ctx)?,
        (Method::Get, ["localization"]) => i18n::overview_json(),
        (Method::Get, ["localization", "file"]) => repo_file(ctx)?,
        _ => {
            return Ok((
                404,
                json!({ "error": format!("no route for {} {}", ctx.method, ctx.path) }),
            ))
        }
    };
    Ok((200, value))
}

pub fn register_current() {}

fn root() -> PathBuf {
    workspace::work_root()
}

fn body_json(ctx: &Ctx) -> Result<Value> {
    if ctx.body.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&ctx.body).map_err(Into::into)
}

fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        (name == key).then(|| {
            percent_encoding::percent_decode_str(value)
                .decode_utf8_lossy()
                .to_string()
        })
    })
}

fn glossary() -> Value {
    let root = root();
    let cfg = config::resolve(Some(&root));
    let terms = curation::glossary_groups(&root, &cfg)
        .into_iter()
        .filter_map(|group| {
            let mut values = group.into_iter();
            Some(json!({ "use": values.next()?, "variants": values.collect::<Vec<_>>() }))
        })
        .collect::<Vec<_>>();
    json!({ "file": cfg["glossary"]["file"].as_str().unwrap_or("STYLE.md"), "terms": terms })
}

fn config_get() -> Value {
    let root = root();
    let defaults = config::defaults();
    let paths = config::known_paths()
        .into_iter()
        .map(|path| {
            let kind = config::get_path(&defaults, &path)
                .map(type_name)
                .unwrap_or("string");
            json!({ "path": path, "type": kind })
        })
        .collect::<Vec<_>>();
    json!({
        "effective": config::resolve(Some(&root)),
        "paths": paths,
        "repo": config::read_json(&config::repo_config_path(&root)),
    })
}

fn config_set(ctx: &Ctx) -> Result<Value> {
    let body = body_json(ctx)?;
    let path = body["path"]
        .as_str()
        .ok_or_else(|| anyhow!("path required"))?;
    let value = match &body["value"] {
        Value::String(raw) => config::coerce(path, raw)?,
        value if !value.is_null() => value.clone(),
        _ => return Err(anyhow!("value required")),
    };
    config::set_repo(&root(), path, value)?;
    Ok(json!({ "ok": true, "rebuildReminder": false }))
}

fn nudges_list() -> Value {
    let cfg = config::resolve(Some(&root()));
    json!({ "nudges": cfg.get("nudges").cloned().unwrap_or(json!([])) })
}

fn nudges_add(ctx: &Ctx) -> Result<Value> {
    let body = body_json(ctx)?;
    let name = body["name"]
        .as_str()
        .ok_or_else(|| anyhow!("name required"))?;
    let when = body["when"]
        .as_str()
        .ok_or_else(|| anyhow!("when required"))?;
    let edit = body["edit"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let code = rulescmd::nudge(
        &["add".into(), name.into()],
        false,
        Some(when),
        &edit,
        body["message"].as_str(),
        body["exclude"].as_str(),
    )?;
    if code != 0 {
        return Err(anyhow!("nudge add failed"));
    }
    Ok(json!({ "ok": true }))
}

fn nudges_remove(ctx: &Ctx) -> Result<Value> {
    let name = query_param(&ctx.query, "name").ok_or_else(|| anyhow!("name required"))?;
    let code = rulescmd::nudge(&["remove".into(), name], false, None, &[], None, None)?;
    if code != 0 {
        return Err(anyhow!("no such nudge"));
    }
    Ok(json!({ "ok": true }))
}

fn rules_list() -> Value {
    let cfg = config::resolve(Some(&root()));
    json!({ "rules": cfg.get("rules").cloned().unwrap_or(json!([])) })
}

fn rules_add(ctx: &Ctx) -> Result<Value> {
    let body = body_json(ctx)?;
    let name = body["name"]
        .as_str()
        .ok_or_else(|| anyhow!("name required"))?;
    let paths = body["paths"]
        .as_str()
        .ok_or_else(|| anyhow!("paths required"))?;
    let notify = body["notify"]
        .as_str()
        .ok_or_else(|| anyhow!("notify required"))?;
    let code = rulescmd::rules(
        &["add".into(), name.into()],
        false,
        false,
        Some(paths),
        Some(notify),
        body["exclude"].as_str(),
    )?;
    if code != 0 {
        return Err(anyhow!("rule add failed"));
    }
    Ok(json!({ "ok": true }))
}

fn rules_remove(ctx: &Ctx) -> Result<Value> {
    let name = query_param(&ctx.query, "name").ok_or_else(|| anyhow!("name required"))?;
    let code = rulescmd::rules(&["remove".into(), name], false, false, None, None, None)?;
    if code != 0 {
        return Err(anyhow!("no such rule"));
    }
    Ok(json!({ "ok": true }))
}

fn rules_discover() -> Result<Value> {
    rulescmd::rules(&["discover".into()], false, true, None, None, None)?;
    Ok(rules_list())
}

fn detector_get() -> Value {
    let cfg = config::resolve(Some(&root()));
    let detector_cfg = &cfg["detector"];
    let catalog = detector::registry()
        .iter()
        .map(|rule| {
            json!({
                "id": rule.id,
                "family": serde_json::to_value(rule.family).unwrap_or(json!("")),
                "pack": rule.pack,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "styleGuide": detector_cfg["styleGuide"],
        "zeroTolerance": detector_cfg["zeroTolerance"],
        "ignoreRules": detector_cfg["ignoreRules"],
        "ignoreFiles": detector_cfg["ignoreFiles"],
        "grammar": detector_cfg["grammar"],
        "catalog": catalog,
    })
}

fn detector_zero(ctx: &Ctx) -> Result<Value> {
    let body = body_json(ctx)?;
    let rule = body["rule"]
        .as_str()
        .ok_or_else(|| anyhow!("rule required"))?;
    let action = body["action"].as_str().unwrap_or("add");
    let code = rulescmd::zero(&[action.into(), rule.into()])?;
    if code != 0 {
        return Err(anyhow!("zero {action} failed"));
    }
    Ok(json!({ "ok": true }))
}

fn detector_ignore(ctx: &Ctx) -> Result<Value> {
    let body = body_json(ctx)?;
    let rule = body["rule"]
        .as_str()
        .ok_or_else(|| anyhow!("rule required"))?;
    match body["action"].as_str().unwrap_or("add") {
        "add" => {
            rulescmd::ignores(&["add-rule".into(), rule.into()], body["reason"].as_str())?;
        }
        "remove" => remove_from_repo_array("detector.ignoreRules", rule)?,
        _ => return Err(anyhow!("action must be add or remove")),
    }
    Ok(json!({ "ok": true }))
}

fn detector_lists_get() -> Value {
    let root = root();
    let effective = config::resolve(Some(&root));
    let overrides = effective["detector"]["lists"].clone();
    let repo = config::read_json(&config::repo_config_path(&root));
    let repo_overrides = &repo["detector"]["lists"];
    let lists = detector::lists::registry().iter().map(|definition| {
        let value = overrides.get(definition.id);
        json!({
            "id": definition.id,
            "label": definition.label,
            "family": serde_json::to_value(definition.family).unwrap_or(json!("")),
            "pack": definition.pack,
            "kind": serde_json::to_value(definition.kind).unwrap_or(json!("")),
            "default": definition.default_json(),
            "override": value,
            "overridden": value.is_some(),
            "source": if repo_overrides.get(definition.id).is_some() { "repo" } else { "default" },
        })
    }).collect::<Vec<_>>();
    json!({ "lists": lists })
}

fn detector_lists_set(ctx: &Ctx) -> Result<Value> {
    let body = body_json(ctx)?;
    let id = body["id"].as_str().ok_or_else(|| anyhow!("id required"))?;
    if !detector::lists::registry()
        .iter()
        .any(|definition| definition.id == id)
    {
        return Err(anyhow!("unknown list id: {id}"));
    }
    if body["reset"].as_bool().unwrap_or(false) {
        remove_list_override(id)?;
    } else {
        let value = body
            .get("value")
            .filter(|value| value.is_array())
            .cloned()
            .ok_or_else(|| anyhow!("value must be an array"))?;
        config::set_repo(&root(), &format!("detector.lists.{id}"), value)?;
    }
    Ok(json!({ "ok": true }))
}

fn detect(ctx: &Ctx) -> Result<Value> {
    let body = body_json(ctx)?;
    let settings = detector::runner::settings(false, body["style"].as_str());
    let (path, text) = if let Some(path) = body["path"].as_str().filter(|path| !path.is_empty()) {
        (
            path.to_string(),
            std::fs::read_to_string(safe_join(&root(), path)?)?,
        )
    } else {
        (
            "input.md".into(),
            body["text"]
                .as_str()
                .ok_or_else(|| anyhow!("text or path required"))?
                .into(),
        )
    };
    let result = detector::runner::detect_text(&path, &text, &settings);
    let findings = result
        .findings
        .iter()
        .map(|finding| {
            json!({
                "ruleId": finding.rule_id,
                "family": serde_json::to_value(finding.family).unwrap_or(json!("")),
                "severity": serde_json::to_value(finding.severity).unwrap_or(json!("")),
                "message": finding.message,
                "span": finding.span,
                "offset": finding.offset,
                "length": finding.length,
                "line": finding.line,
                "col": finding.col,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "path": path,
        "styleGuide": settings.style_guide,
        "wordCount": result.word_count,
        "score": detector::score::compute(&text, &result.findings, None),
        "findings": findings,
    }))
}

fn templates_scaffold(ctx: &Ctx) -> Result<Value> {
    let body = body_json(ctx)?;
    let kind = body["type"]
        .as_str()
        .ok_or_else(|| anyhow!("type required"))?;
    let mut args = vec!["scaffold".into(), kind.into()];
    if let Some(title) = body["title"]
        .as_str()
        .filter(|title| !title.trim().is_empty())
    {
        args.push(title.into());
    }
    let code = assets::run(&args, false, body["force"].as_bool().unwrap_or(false))?;
    if code != 0 {
        return Err(anyhow!("scaffold failed"));
    }
    Ok(json!({ "ok": true }))
}

fn repo_file(ctx: &Ctx) -> Result<Value> {
    let relative = query_param(&ctx.query, "path").ok_or_else(|| anyhow!("path required"))?;
    let content = std::fs::read_to_string(safe_join(&root(), &relative)?)?;
    let truncated = content.chars().count() > 200_000;
    let content = if truncated {
        content.chars().take(200_000).collect()
    } else {
        content
    };
    Ok(json!({ "path": relative, "content": content, "truncated": truncated }))
}

fn safe_join(root: &Path, relative: &str) -> Result<PathBuf> {
    let path = std::fs::canonicalize(root.join(relative))
        .map_err(|_| anyhow!("no such path: {relative}"))?;
    let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    if !path.starts_with(root) {
        return Err(anyhow!("path is outside the repository"));
    }
    Ok(path)
}

fn remove_from_repo_array(path: &str, item: &str) -> Result<()> {
    let config_path = config::repo_config_path(&root());
    let mut value = config::read_json(&config_path);
    let retained = config::get_path(&value, path)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|value| value.as_str() != Some(item))
        .collect();
    config::set_path(&mut value, path, Value::Array(retained));
    std::fs::create_dir_all(config_path.parent().unwrap())?;
    std::fs::write(config_path, serde_json::to_string_pretty(&value)?)?;
    Ok(())
}

fn remove_list_override(id: &str) -> Result<()> {
    let path = config::repo_config_path(&root());
    let mut value = config::read_json(&path);
    if let Some(map) = value
        .get_mut("detector")
        .and_then(|value| value.get_mut("lists"))
        .and_then(Value::as_object_mut)
    {
        map.remove(id);
    }
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(path, serde_json::to_string_pretty(&value)?)?;
    Ok(())
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Bool(_) => "boolean",
        Value::Number(number) if number.is_f64() => "number",
        Value::Number(_) => "integer",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
        _ => "string",
    }
}
