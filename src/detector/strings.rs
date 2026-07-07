//! §5.4 `detect --strings`: extract user-facing copy from code so the detector
//! runs on a product's real copy, not only its markdown. Deterministic,
//! conservative extraction — JSX text nodes plus string/template literals that
//! read like prose (a space, ≥2 alphabetic words, a lowercase letter), with
//! `className`/`import`/attribute noise excluded. It favours precision: a
//! single-word label is left out rather than risk flagging an identifier.

use ignore::WalkBuilder;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// One extracted user-facing string with the source line it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Copy {
    pub line: usize,
    pub text: String,
}

/// Extract copy from every JS/TS/JSX/TSX file under `dir`, returning
/// `(relative_path, raw_source, copies)` per file that yielded any copy.
pub fn extract_dir(root: &Path, dir: &Path) -> Vec<(String, String, Vec<Copy>)> {
    let mut out = Vec::new();
    for file in code_files(dir) {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let copies = extract_source(&text);
        if copies.is_empty() {
            continue;
        }
        let rel = file
            .strip_prefix(root)
            .unwrap_or(&file)
            .display()
            .to_string();
        out.push((rel, text, copies));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn code_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in WalkBuilder::new(dir)
        .hidden(false)
        .git_ignore(true)
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !(e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                && matches!(
                    name.as_ref(),
                    ".git"
                        | ".mari"
                        | "target"
                        | "node_modules"
                        | "dist"
                        | "build"
                        | ".next"
                        | "coverage"
                        | "vendor"
                ))
        })
        .build()
        .flatten()
    {
        let path = entry.path();
        let is_code = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| matches!(e.to_ascii_lowercase().as_str(), "js" | "jsx" | "ts" | "tsx"))
            .unwrap_or(false);
        // Skip test files — their strings are assertions, not product copy.
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let is_test = name.contains(".test.") || name.contains(".spec.");
        if path.is_file() && is_code && !is_test {
            out.push(path.to_path_buf());
        }
    }
    out.sort();
    out
}

/// Extract copy strings from one source file's text.
pub fn extract_source(text: &str) -> Vec<Copy> {
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let lno = idx + 1;
        let trimmed = line.trim_start();
        if is_ignorable_line(trimmed) {
            continue;
        }
        for s in jsx_text_nodes(line) {
            if is_copy(&s) {
                out.push(Copy { line: lno, text: s });
            }
        }
        for (before, s) in string_literals(line) {
            if attribute_excluded(&before) {
                continue;
            }
            if is_copy(&s) {
                out.push(Copy { line: lno, text: s });
            }
        }
    }
    out
}

fn is_ignorable_line(trimmed: &str) -> bool {
    trimmed.starts_with("import ")
        || trimmed.starts_with("export ") && trimmed.contains(" from ")
        || trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.contains("require(")
}

/// JSX text between a closing `>` and the next `<`, excluding `{expr}` braces.
fn jsx_text_nodes(line: &str) -> Vec<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r">([^<>{}]+)<").unwrap());
    re.captures_iter(line)
        .filter_map(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Scan a line for `"…"`, `'…'`, and `` `…` `` literals, returning the text
/// immediately before each opening quote (for attribute detection) and the
/// literal's inner content. Template literals with `${}` are skipped.
fn string_literals(line: &str) -> Vec<(String, String)> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '"' || c == '\'' || c == '`' {
            if let Some(end) = find_close(bytes, i + 1, c) {
                let inner = &line[i + 1..end];
                if c != '`' || !inner.contains("${") {
                    let before = line[..i].to_string();
                    out.push((before, inner.to_string()));
                }
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn find_close(bytes: &[u8], from: usize, quote: char) -> Option<usize> {
    let q = quote as u8;
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == q {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// True when a string literal is the value of a non-copy attribute
/// (`className="…"`, `href="…"`, …) or an object key/`.method("…")` argument
/// that is plumbing rather than product copy.
fn attribute_excluded(before: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"([A-Za-z_][\w-]*)\s*[=:]\s*$").unwrap());
    if let Some(caps) = re.captures(before.trim_end()) {
        let name = caps.get(1).unwrap().as_str().to_ascii_lowercase();
        return matches!(
            name.as_str(),
            "classname"
                | "class"
                | "href"
                | "src"
                | "id"
                | "key"
                | "type"
                | "rel"
                | "htmlfor"
                | "name"
                | "to"
                | "path"
                | "role"
                | "style"
                | "target"
                | "method"
                | "action"
                | "xmlns"
                | "viewbox"
                | "d"
                | "points"
                | "as"
                | "variant"
                | "size"
                | "color"
                | "testid"
        ) || name.starts_with("data")
            || name.starts_with("aria")
            || name.starts_with("on");
    }
    false
}

/// Prose test: at least one space, ≥2 purely-alphabetic words, a lowercase
/// letter, and not a URL or path. Deliberately conservative — requiring clean
/// words (no digits or hyphens) rejects CSS class lists like
/// `"grid grid-cols-2"` while keeping copy like `"Save changes"`.
fn is_copy(s: &str) -> bool {
    let s = s.trim();
    if s.len() < 4 || !s.contains(' ') {
        return false;
    }
    if s.contains("://") || s.starts_with('/') || s.starts_with("./") || s.starts_with("../") {
        return false;
    }
    if !s.chars().any(|c| c.is_ascii_lowercase()) {
        return false;
    }
    let clean_words = s
        .split_whitespace()
        .filter(|w| {
            let w = w.trim_matches(|c: char| {
                matches!(c, '.' | ',' | '!' | '?' | ':' | ';' | '"' | '\'')
            });
            w.chars().count() >= 2 && w.chars().all(|c| c.is_alphabetic())
        })
        .count();
    clean_words >= 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_jsx_text_and_prose_literals() {
        let src = r#"
import { Button } from "./ui";
export function Hero() {
  return (
    <section className="flex items-center gap-2">
      <h1>Ship your docs in minutes</h1>
      <Button aria-label="Get started now">Get started</Button>
      <p title="Read the full guide">{tagline}</p>
    </section>
  );
}
const url = "https://example.com/docs";
const label = "Save changes";
const cls = "grid grid-cols-2";
"#;
        let copies = extract_source(src);
        let texts: Vec<&str> = copies.iter().map(|c| c.text.as_str()).collect();

        // JSX text node and a prose title attribute are copy.
        assert!(texts.contains(&"Ship your docs in minutes"));
        assert!(texts.contains(&"Read the full guide"));
        assert!(texts.contains(&"Save changes"));

        // className / import path / URL are not copy.
        assert!(!texts.iter().any(|t| t.contains("items-center")));
        assert!(!texts.iter().any(|t| t.contains("grid-cols")));
        assert!(!texts.iter().any(|t| t.contains("example.com")));
        assert!(!texts.contains(&"./ui"));
    }

    #[test]
    fn skips_test_and_template_interpolation() {
        // `${}` template literals are skipped (they're not stable copy).
        let copies = extract_source("const msg = `Hello ${name} welcome`;\n");
        assert!(copies.is_empty(), "got {copies:?}");
    }
}
