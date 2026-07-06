//! STUB — tags, glossary, facts, extract, audit kb, humanize (SPEC §5.3/§10).
use anyhow::Result;
use std::path::Path;
pub fn tag(_args: &[String], _note: Option<&str>, _status: Option<&str>, _json: bool) -> Result<i32> { eprintln!("not implemented yet"); Ok(1) }
pub fn glossary(_args: &[String], _use: Option<&str>, _not: Option<&str>) -> Result<i32> { eprintln!("not implemented yet"); Ok(1) }
pub fn facts(_args: &[String], _source: Option<&str>) -> Result<i32> { eprintln!("not implemented yet"); Ok(1) }
pub fn extract(_args: &[String], _source: Option<&str>, _doc: Option<&str>, _since: Option<i64>, _json: bool) -> Result<i32> { eprintln!("not implemented yet"); Ok(1) }
pub fn audit_kb(_paths: &[String], _json: bool, _strict: bool) -> Result<i32> { eprintln!("not implemented yet"); Ok(1) }
pub fn humanize(_action: Option<&str>, _json: bool) -> Result<i32> { eprintln!("not implemented yet"); Ok(1) }
/// STYLE.md Terminology table rows as variant groups [use, not...] for the
/// terminology-consistency rule (SPEC §10.2).
pub fn glossary_groups(_root: &Path, _cfg: &serde_json::Value) -> Vec<Vec<String>> {
    Vec::new()
}
/// Curation tag for a repo-relative path or doc ref, if any (SPEC §10.1).
pub fn tag_of(_root: &Path, _cfg: &serde_json::Value, _ref: &str) -> Option<String> {
    None
}
