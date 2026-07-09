//! Granola connector (SPEC §6.14): reads Granola's on-device meeting-notes
//! cache — no network call, no credential. One document per note (AI-enhanced
//! notes plus the user's raw notes; raw transcript appended only when
//! `granola.transcripts=true`). Prunes notes that vanish from the cache or
//! whose folder was untracked.

use super::{ingest_remote_doc, prune_source_except, tracked_list, RemoteDoc, SyncStats};
use anyhow::Result;
use duckdb::Connection;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub fn sync(conn: &Connection, cfg: &Value, _rebuild: bool) -> Result<SyncStats> {
    // Connected = cache present (§6.14). An absent cache is a no-op, not an
    // error — most repos will never have Granola installed.
    let Some(cache) = load_cache(cfg) else {
        return Ok(SyncStats::default());
    };
    let folders = tracked_list(cfg, "granola.folders");
    let want_transcripts = cfg["granola"]["transcripts"].as_bool().unwrap_or(false);

    let mut stats = SyncStats::default();
    let mut keep: BTreeSet<String> = BTreeSet::new();
    for note in parse_notes(&cache, want_transcripts) {
        // With nothing tracked, index every note; `folders` narrows.
        if !folders.is_empty()
            && !note
                .folders
                .iter()
                .any(|f| folders.iter().any(|t| folder_matches(t, f)))
        {
            continue;
        }
        keep.insert(note.id.clone());
        stats.seen += 1;
        let doc = note.to_doc();
        match ingest_remote_doc(conn, "granola", &doc) {
            Ok(Some(chunks)) => {
                stats.changed += 1;
                stats.chunks += chunks;
                eprintln!("  granola {}", doc.external_id);
            }
            Ok(None) => {}
            Err(e) => eprintln!("note: granola {} skipped: {e}", doc.external_id),
        }
    }
    // Prunes notes removed from the cache (and, when a folder filter is set,
    // notes that fell out of the tracked folders).
    stats.deleted += prune_source_except(conn, "granola", &keep)?;
    Ok(stats)
}

/// Resolve the cache location: explicit `granola.cache_path`, else the macOS
/// default `~/Library/Application Support/Granola/cache-v3.json`.
pub fn cache_path(cfg: &Value) -> PathBuf {
    if let Some(p) = cfg["granola"]["cache_path"]
        .as_str()
        .filter(|s| !s.is_empty())
    {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join("Library/Application Support/Granola/cache-v3.json")
}

/// Whether a Granola cache is present (drives always-when-connected activeness).
pub fn cache_present(cfg: &Value) -> bool {
    cache_path(cfg).exists()
}

fn load_cache(cfg: &Value) -> Option<Value> {
    let raw = std::fs::read_to_string(cache_path(cfg)).ok()?;
    unwrap_cache(&raw)
}

/// Granola double-encodes: the file is `{"cache": "<stringified state JSON>"}`.
/// Older/other layouts store the state object directly; accept both.
fn unwrap_cache(raw: &str) -> Option<Value> {
    let v: Value = serde_json::from_str(raw).ok()?;
    match v.get("cache").and_then(|c| c.as_str()) {
        Some(inner) => serde_json::from_str(inner).ok(),
        None => Some(v),
    }
}

struct RawNote {
    id: String,
    title: String,
    created_at: Option<String>,
    updated_at: Option<String>,
    enhanced: String,
    raw: String,
    transcript: String,
    folders: Vec<String>,
}

impl RawNote {
    fn to_doc(&self) -> RemoteDoc {
        let mut body = format!("# {}\n", self.title);
        if !self.enhanced.trim().is_empty() {
            body.push('\n');
            body.push_str(self.enhanced.trim());
            body.push('\n');
        }
        if !self.raw.trim().is_empty() {
            body.push_str("\n## Notes\n\n");
            body.push_str(self.raw.trim());
            body.push('\n');
        }
        if !self.transcript.trim().is_empty() {
            body.push_str("\n## Transcript\n\n");
            body.push_str(self.transcript.trim());
            body.push('\n');
        }
        RemoteDoc {
            external_id: self.id.clone(),
            canonical_ref: format!("granola:{}", self.id),
            title: self.title.clone(),
            url: None,
            author: None,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
            mime: "text/markdown",
            kind: "note",
            container: None,
            body,
            // Content hash is the re-embed authority (§6.0); the note's own
            // updated_at is the revision signal.
            revision: self
                .updated_at
                .clone()
                .or_else(|| self.created_at.clone())
                .unwrap_or_default(),
        }
    }
}

fn parse_notes(cache: &Value, want_transcripts: bool) -> Vec<RawNote> {
    // The document map lives under `state.documents`; some dumps hoist it to
    // the top level.
    let state = cache.get("state").unwrap_or(cache);
    let Some(documents) = state.get("documents").and_then(|d| d.as_object()) else {
        return Vec::new();
    };
    let folder_map = folder_index(state);
    let transcripts = state.get("transcripts");

    let mut notes = Vec::new();
    for (key, doc) in documents {
        let id = doc
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or(key)
            .to_string();
        let title = doc
            .get("title")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("Untitled note")
            .to_string();
        // AI-enhanced notes live in the last viewed panel; fall back to any
        // ai-notes field if the panel is absent.
        let mut enhanced = prose_text(&doc["last_viewed_panel"]["content"]);
        if enhanced.trim().is_empty() {
            enhanced = prose_text(&doc["ai_notes"]);
        }
        // The user's raw notes: ProseMirror doc, or a plain/markdown mirror.
        let mut raw = prose_text(&doc["notes"]);
        if raw.trim().is_empty() {
            raw = doc
                .get("notes_markdown")
                .or_else(|| doc.get("notes_plain"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
        }
        let transcript = if want_transcripts {
            transcript_text(transcripts, &id)
        } else {
            String::new()
        };
        notes.push(RawNote {
            created_at: doc
                .get("created_at")
                .and_then(|v| v.as_str())
                .map(String::from),
            updated_at: doc
                .get("updated_at")
                .and_then(|v| v.as_str())
                .map(String::from),
            enhanced,
            raw,
            transcript,
            folders: folder_map.get(&id).cloned().unwrap_or_default(),
            id,
            title,
        });
    }
    notes
}

/// Build a doc-id → folder-names map from Granola's document lists.
fn folder_index(state: &Value) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let lists = state
        .get("documentLists")
        .or_else(|| state.get("lists"))
        .and_then(|v| v.as_object());
    let Some(lists) = lists else {
        return map;
    };
    for (list_id, list) in lists {
        let name = list
            .get("title")
            .or_else(|| list.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or(list_id)
            .to_string();
        let ids = list
            .get("document_ids")
            .or_else(|| list.get("documentIds"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for id in ids.iter().filter_map(|v| v.as_str()) {
            map.entry(id.to_string()).or_default().push(name.clone());
        }
    }
    map
}

fn transcript_text(transcripts: Option<&Value>, doc_id: &str) -> String {
    let Some(segments) = transcripts
        .and_then(|t| t.get(doc_id))
        .and_then(|v| v.as_array())
    else {
        return String::new();
    };
    let mut out = String::new();
    for seg in segments {
        let text = seg
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if text.is_empty() {
            continue;
        }
        match seg.get("source").and_then(|v| v.as_str()) {
            Some(who) => out.push_str(&format!("{who}: {text}\n")),
            None => out.push_str(&format!("{text}\n")),
        }
    }
    out
}

/// Collect readable text from a ProseMirror-shaped node (or a plain string),
/// inserting line breaks at block boundaries. Tolerant of unknown shapes.
fn prose_text(node: &Value) -> String {
    let mut out = String::new();
    walk_prose(node, &mut out);
    // Collapse the runs of blank lines walking can produce.
    let mut cleaned = String::new();
    let mut blanks = 0;
    for line in out.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            blanks += 1;
            if blanks <= 1 {
                cleaned.push('\n');
            }
        } else {
            blanks = 0;
            cleaned.push_str(line);
            cleaned.push('\n');
        }
    }
    cleaned.trim().to_string()
}

fn walk_prose(node: &Value, out: &mut String) {
    match node {
        Value::String(s) => out.push_str(s),
        Value::Array(items) => {
            for item in items {
                walk_prose(item, out);
            }
        }
        Value::Object(map) => {
            let block = matches!(
                map.get("type").and_then(|v| v.as_str()),
                Some(
                    "paragraph"
                        | "heading"
                        | "listItem"
                        | "list_item"
                        | "bulletList"
                        | "bullet_list"
                        | "orderedList"
                        | "ordered_list"
                        | "blockquote"
                        | "codeBlock"
                        | "code_block"
                )
            );
            if let Some(text) = map.get("text").and_then(|v| v.as_str()) {
                out.push_str(text);
            }
            if let Some(content) = map.get("content") {
                walk_prose(content, out);
            }
            if block {
                out.push('\n');
            }
        }
        _ => {}
    }
}

/// A tracked folder ref (`granola:Eng` or bare `Eng`) matches a folder name
/// case-insensitively.
fn folder_matches(tracked: &str, folder: &str) -> bool {
    let want = tracked.strip_prefix("granola:").unwrap_or(tracked);
    want.eq_ignore_ascii_case(folder)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_cache() -> Value {
        json!({
            "state": {
                "documents": {
                    "doc-1": {
                        "id": "doc-1",
                        "title": "Kafka connector kickoff",
                        "created_at": "2026-06-01T10:00:00Z",
                        "updated_at": "2026-06-01T11:00:00Z",
                        "notes": {
                            "type": "doc",
                            "content": [
                                {"type": "paragraph", "content": [
                                    {"type": "text", "text": "raw agenda point"}
                                ]}
                            ]
                        },
                        "last_viewed_panel": {
                            "content": {
                                "type": "doc",
                                "content": [
                                    {"type": "heading", "content": [
                                        {"type": "text", "text": "Summary"}
                                    ]},
                                    {"type": "paragraph", "content": [
                                        {"type": "text", "text": "decided to ship v1"}
                                    ]}
                                ]
                            }
                        }
                    },
                    "doc-2": {
                        "id": "doc-2",
                        "title": "Unrelated 1:1",
                        "updated_at": "2026-06-02T09:00:00Z",
                        "notes_markdown": "personal note"
                    }
                },
                "documentLists": {
                    "list-a": {"title": "OSS", "document_ids": ["doc-1"]}
                },
                "transcripts": {
                    "doc-1": [
                        {"source": "me", "text": "let's begin"},
                        {"source": "them", "text": "sounds good"}
                    ]
                }
            }
        })
    }

    #[test]
    fn prose_text_flattens_prosemirror() {
        let node = json!({
            "type": "doc",
            "content": [
                {"type": "heading", "content": [{"type": "text", "text": "Summary"}]},
                {"type": "paragraph", "content": [{"type": "text", "text": "one two"}]}
            ]
        });
        let text = prose_text(&node);
        assert!(text.contains("Summary"));
        assert!(text.contains("one two"));
    }

    #[test]
    fn parse_notes_maps_enhanced_raw_and_folders() {
        let notes = parse_notes(&sample_cache(), false);
        let n1 = notes.iter().find(|n| n.id == "doc-1").unwrap();
        assert_eq!(n1.title, "Kafka connector kickoff");
        assert!(n1.enhanced.contains("decided to ship v1"));
        assert!(n1.raw.contains("raw agenda point"));
        assert_eq!(n1.folders, vec!["OSS".to_string()]);
        // notes_markdown fallback when there is no ProseMirror `notes`.
        let n2 = notes.iter().find(|n| n.id == "doc-2").unwrap();
        assert_eq!(n2.raw, "personal note");
        assert!(n2.folders.is_empty());
    }

    #[test]
    fn transcript_included_only_when_requested() {
        let off = parse_notes(&sample_cache(), false);
        assert!(off
            .iter()
            .find(|n| n.id == "doc-1")
            .unwrap()
            .transcript
            .is_empty());
        let on = parse_notes(&sample_cache(), true);
        let t = &on.iter().find(|n| n.id == "doc-1").unwrap().transcript;
        assert!(t.contains("me: let's begin"));
        assert!(t.contains("them: sounds good"));
    }

    #[test]
    fn doc_body_prepends_title_and_sections() {
        let notes = parse_notes(&sample_cache(), true);
        let doc = notes.iter().find(|n| n.id == "doc-1").unwrap().to_doc();
        assert_eq!(doc.external_id, "doc-1");
        assert_eq!(doc.canonical_ref, "granola:doc-1");
        assert!(doc.body.starts_with("# Kafka connector kickoff"));
        assert!(doc.body.contains("## Notes"));
        assert!(doc.body.contains("## Transcript"));
        assert_eq!(doc.revision, "2026-06-01T11:00:00Z");
    }

    #[test]
    fn folder_matching_is_prefix_and_case_insensitive() {
        assert!(folder_matches("granola:OSS", "oss"));
        assert!(folder_matches("OSS", "OSS"));
        assert!(!folder_matches("granola:Internal", "OSS"));
    }

    #[test]
    fn unwrap_handles_double_encoded_and_direct() {
        let inner = json!({"state": {"documents": {}}});
        let wrapped = json!({"cache": inner.to_string()}).to_string();
        assert!(unwrap_cache(&wrapped).is_some());
        assert!(unwrap_cache(&inner.to_string()).is_some());
        assert!(unwrap_cache("not json").is_none());
    }
}
