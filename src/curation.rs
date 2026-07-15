//! File-backed glossary helpers.

use crate::{config, workspace};
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn configured_path(root: &Path, cfg: &Value, section: &str, fallback: &str) -> PathBuf {
    root.join(cfg[section]["file"].as_str().unwrap_or(fallback))
}

fn glossary_path(root: &Path, cfg: &Value) -> PathBuf {
    configured_path(root, cfg, "glossary", "STYLE.md")
}

pub fn glossary(args: &[String], use_: Option<&str>, not: Option<&str>) -> Result<i32> {
    let root = workspace::work_root();
    let cfg = config::resolve(Some(&root));
    let path = glossary_path(&root, &cfg);
    match args.first().map(String::as_str) {
        None | Some("list") => {
            for group in glossary_groups(&root, &cfg) {
                println!("{}", group.join(" | "));
            }
        }
        Some("add") => {
            let preferred = use_
                .or_else(|| args.get(1).map(String::as_str))
                .ok_or_else(|| anyhow!("usage: mari glossary add <term> [--not <variants>]"))?;
            let variants = not.unwrap_or("");
            append_terminology_row(&path, preferred, variants)?;
            println!("✓ added `{preferred}` to {}", path.display());
        }
        Some("harvest") => println!(
            "glossary harvest is agent-guided; add approved terms with `mari glossary add`"
        ),
        Some(other) => return Err(anyhow!("unknown glossary action: {other}")),
    }
    Ok(0)
}

pub fn glossary_groups(root: &Path, cfg: &Value) -> Vec<Vec<String>> {
    let Ok(text) = std::fs::read_to_string(glossary_path(root, cfg)) else {
        return Vec::new();
    };
    let Some((start, end)) = terminology_section(&text) else {
        return Vec::new();
    };
    text[start..end]
        .lines()
        .filter_map(|line| {
            let cells: Vec<_> = line
                .trim()
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect();
            let preferred = cells.first().copied().unwrap_or("");
            if !line.trim().starts_with('|')
                || preferred.is_empty()
                || preferred.eq_ignore_ascii_case("use")
                || preferred.chars().all(|c| matches!(c, '-' | ':' | ' '))
            {
                return None;
            }
            let mut group = vec![preferred.to_string()];
            if let Some(variants) = cells.get(1) {
                group.extend(
                    variants
                        .split(',')
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                        .map(str::to_string),
                );
            }
            Some(group)
        })
        .collect()
}

fn terminology_section(text: &str) -> Option<(usize, usize)> {
    let mut offset = 0;
    let mut start = None;
    for line in text.lines() {
        let next = (offset + line.len() + 1).min(text.len());
        let trimmed = line.trim();
        if start.is_none()
            && trimmed.starts_with('#')
            && trimmed
                .trim_start_matches('#')
                .trim()
                .eq_ignore_ascii_case("terminology")
        {
            start = Some(next);
        } else if let Some(section_start) = start {
            if trimmed.starts_with('#') {
                return Some((section_start, offset));
            }
        }
        offset = next;
    }
    start.map(|section_start| (section_start, text.len()))
}

fn append_terminology_row(path: &Path, preferred: &str, variants: &str) -> Result<()> {
    let mut text = std::fs::read_to_string(path).unwrap_or_default();
    let row = format!("| {preferred} | {variants} |\n");
    if let Some((_, end)) = terminology_section(&text) {
        text.insert_str(end, &row);
    } else {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str("\n## Terminology\n\n| Use | Not |\n|---|---|\n");
        text.push_str(&row);
    }
    std::fs::write(path, text)?;
    Ok(())
}
