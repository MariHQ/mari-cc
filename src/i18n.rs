//! Localization layout detection and structural conformance (SPEC §5.7).

use anyhow::{anyhow, Result};
use ignore::WalkBuilder;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const SOURCE_LANGS: &[&str] = &["en", "en-us", "en-gb"];
const LANGS: &[&str] = &[
    "ar", "bg", "cs", "da", "de", "el", "en", "en-gb", "en-us", "es", "et", "fi", "fr", "he", "hi",
    "hr", "hu", "id", "it", "ja", "ko", "lt", "lv", "ms", "nl", "no", "pl", "pt", "pt-br", "ro",
    "ru", "sk", "sl", "sv", "th", "tr", "uk", "vi", "zh", "zh-cn", "zh-tw",
];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct Translation {
    path: PathBuf,
    lang: String,
    layout: String,
}

#[derive(Debug, Clone, Serialize)]
struct FileReport {
    source: String,
    translations: Vec<TranslationReport>,
}

#[derive(Debug, Clone, Serialize)]
struct TranslationReport {
    path: String,
    lang: String,
    layout: String,
    issues: Vec<String>,
}

#[derive(Default, Debug, PartialEq, Eq)]
struct Structure {
    headings: Vec<String>,
    heading_levels: Vec<u8>,
    code_blocks: usize,
    links: Vec<String>,
}

pub fn run(args: &[String], deep: bool, limit: Option<usize>, strict: bool) -> Result<i32> {
    if deep {
        eprintln!("note: i18n attention coverage is not available in this build; running structural checks only");
    }

    match args.first().map(|s| s.as_str()) {
        None => {
            eprintln!("usage: mari i18n <file> | mari i18n conform <file|dir> | mari i18n coverage <source> [translation]");
            Ok(2)
        }
        Some("conform") => {
            let target = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: mari i18n conform <file|dir>"))?;
            conform(Path::new(target), limit, strict)
        }
        Some("coverage") => {
            let source = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: mari i18n coverage <source> [translation]"))?;
            coverage(Path::new(source), args.get(2).map(Path::new), strict)
        }
        Some(file) => list(Path::new(file)),
    }
}

/// Localized translation files are skipped by the detector (SPEC §11.0.6).
pub fn is_translation_file(path: &Path) -> bool {
    detected_language(path)
        .map(|lang| !is_source_lang(&lang))
        .unwrap_or(false)
}

/// Translation siblings of a source file, for the hook staleness note.
pub fn siblings(path: &Path) -> Vec<PathBuf> {
    find_translations(path)
        .into_iter()
        .map(|t| t.path)
        .collect()
}

fn list(path: &Path) -> Result<i32> {
    let source = source_for(path);
    println!("source: {}", source.display());
    let translations = find_translations(&source);
    if translations.is_empty() {
        println!("translations: none");
    } else {
        println!("translations:");
        for t in translations {
            println!("  {:<8} {:<12} {}", t.lang, t.layout, t.path.display());
        }
    }
    Ok(0)
}

fn conform(target: &Path, limit: Option<usize>, strict: bool) -> Result<i32> {
    let mut sources = source_files(target);
    sources.sort();
    sources.dedup();
    if let Some(limit) = limit {
        sources.truncate(limit);
    }

    let mut reports = Vec::new();
    let mut issue_count = 0usize;
    for source in sources {
        let translations = find_translations(&source);
        if translations.is_empty() {
            continue;
        }
        let source_structure = read_structure(&source)?;
        let mut translation_reports = Vec::new();
        for t in translations {
            let other = read_structure(&t.path)?;
            let issues = compare_structure(&source_structure, &other);
            issue_count += issues.len();
            translation_reports.push(TranslationReport {
                path: t.path.display().to_string(),
                lang: t.lang,
                layout: t.layout,
                issues,
            });
        }
        reports.push(FileReport {
            source: source.display().to_string(),
            translations: translation_reports,
        });
    }

    if reports.is_empty() {
        println!("i18n: no translation siblings found");
        return Ok(0);
    }

    for report in &reports {
        println!("{}", report.source);
        for t in &report.translations {
            if t.issues.is_empty() {
                println!("  ✓ {:<8} {}", t.lang, t.path);
            } else {
                println!("  ✗ {:<8} {}", t.lang, t.path);
                for issue in &t.issues {
                    println!("    - {issue}");
                }
            }
        }
    }

    if strict && issue_count > 0 {
        Ok(1)
    } else {
        Ok(0)
    }
}

fn coverage(source: &Path, translation: Option<&Path>, strict: bool) -> Result<i32> {
    let source = source_for(source);
    let translations = match translation {
        Some(t) => vec![Translation {
            path: t.to_path_buf(),
            lang: detected_language(t).unwrap_or_else(|| "unknown".into()),
            layout: "explicit".into(),
        }],
        None => find_translations(&source),
    };
    if translations.is_empty() {
        println!("i18n coverage: no translation siblings found");
        return Ok(0);
    }

    let source_structure = read_structure(&source)?;
    let mut issue_count = 0usize;
    for t in translations {
        let other = read_structure(&t.path)?;
        let issues = compare_structure(&source_structure, &other);
        issue_count += issues.len();
        if issues.is_empty() {
            println!(
                "✓ {} structurally covers {}",
                t.path.display(),
                source.display()
            );
        } else {
            println!("✗ {} differs from {}", t.path.display(), source.display());
            for issue in issues {
                println!("  - {issue}");
            }
        }
    }

    if strict && issue_count > 0 {
        Ok(1)
    } else {
        Ok(0)
    }
}

fn source_files(target: &Path) -> Vec<PathBuf> {
    if target.is_file() {
        let source = source_for(target);
        return if is_translation_file(target) {
            vec![source]
        } else {
            vec![target.to_path_buf()]
        };
    }

    let mut out = Vec::new();
    for entry in WalkBuilder::new(target)
        .hidden(false)
        .git_ignore(true)
        .build()
        .flatten()
    {
        let path = entry.path();
        if path.is_file() && is_prose_path(path) && !is_translation_file(path) {
            out.push(path.to_path_buf());
        }
    }
    out
}

fn find_translations(source: &Path) -> Vec<Translation> {
    let source = source_for(source);
    let mut out = Vec::new();
    out.extend(suffix_siblings(&source));
    out.extend(lang_dir_siblings(&source));
    out.extend(hugo_siblings(&source));
    out.extend(docusaurus_siblings(&source));
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out.dedup_by(|a, b| a.path == b.path);
    out
}

fn source_for(path: &Path) -> PathBuf {
    if let Some(p) = suffix_source(path) {
        return p;
    }
    if let Some(p) = lang_dir_source(path) {
        return p;
    }
    if let Some(p) = hugo_source(path) {
        return p;
    }
    if let Some(p) = docusaurus_source(path) {
        return p;
    }
    path.to_path_buf()
}

fn suffix_source(path: &Path) -> Option<PathBuf> {
    let file_name = path.file_name()?.to_str()?;
    let (base, ext) = file_name.rsplit_once('.')?;
    let (stem, lang) = base.rsplit_once('.')?;
    if !is_lang(lang) {
        return None;
    }
    Some(path.with_file_name(format!("{stem}.{ext}")))
}

fn suffix_siblings(source: &Path) -> Vec<Translation> {
    let Some(file_name) = source.file_name().and_then(|s| s.to_str()) else {
        return Vec::new();
    };
    let Some((stem, ext)) = file_name.rsplit_once('.') else {
        return Vec::new();
    };
    let Some(dir) = source.parent() else {
        return Vec::new();
    };
    LANGS
        .iter()
        .filter(|lang| !is_source_lang(lang))
        .filter_map(|lang| {
            let path = dir.join(format!("{stem}.{lang}.{ext}"));
            path.exists().then(|| Translation {
                path,
                lang: (*lang).into(),
                layout: "suffix".into(),
            })
        })
        .collect()
}

fn lang_dir_source(path: &Path) -> Option<PathBuf> {
    let parts: Vec<_> = path.components().collect();
    for (idx, part) in parts.iter().enumerate() {
        let s = part.as_os_str().to_str()?;
        if is_lang(s) && !is_source_lang(s) {
            let mut p = PathBuf::new();
            for part in &parts[..idx] {
                p.push(part.as_os_str());
            }
            p.push("en");
            for part in &parts[idx + 1..] {
                p.push(part.as_os_str());
            }
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

fn lang_dir_siblings(source: &Path) -> Vec<Translation> {
    let parts: Vec<_> = source.components().collect();
    let mut out = Vec::new();
    for (idx, part) in parts.iter().enumerate() {
        let Some(s) = part.as_os_str().to_str() else {
            continue;
        };
        if !is_source_lang(s) {
            continue;
        }
        for lang in LANGS.iter().filter(|lang| !is_source_lang(lang)) {
            let mut p = PathBuf::new();
            for part in &parts[..idx] {
                p.push(part.as_os_str());
            }
            p.push(lang);
            for part in &parts[idx + 1..] {
                p.push(part.as_os_str());
            }
            if p.exists() {
                out.push(Translation {
                    path: p,
                    lang: (*lang).into(),
                    layout: "lang-dir".into(),
                });
            }
        }
    }
    out
}

fn hugo_source(path: &Path) -> Option<PathBuf> {
    let parts: Vec<_> = path.components().collect();
    for (idx, part) in parts.iter().enumerate() {
        let s = part.as_os_str().to_str()?;
        let Some(lang) = s.strip_prefix("content.") else {
            continue;
        };
        if !is_lang(lang) || is_source_lang(lang) {
            continue;
        }
        let mut p = PathBuf::new();
        for part in &parts[..idx] {
            p.push(part.as_os_str());
        }
        p.push("content");
        for part in &parts[idx + 1..] {
            p.push(part.as_os_str());
        }
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn hugo_siblings(source: &Path) -> Vec<Translation> {
    let parts: Vec<_> = source.components().collect();
    let mut out = Vec::new();
    for (idx, part) in parts.iter().enumerate() {
        if part.as_os_str() != "content" {
            continue;
        }
        for lang in LANGS.iter().filter(|lang| !is_source_lang(lang)) {
            let mut p = PathBuf::new();
            for part in &parts[..idx] {
                p.push(part.as_os_str());
            }
            p.push(format!("content.{lang}"));
            for part in &parts[idx + 1..] {
                p.push(part.as_os_str());
            }
            if p.exists() {
                out.push(Translation {
                    path: p,
                    lang: (*lang).into(),
                    layout: "hugo-content".into(),
                });
            }
        }
    }
    out
}

fn docusaurus_source(path: &Path) -> Option<PathBuf> {
    let parts: Vec<_> = path.components().collect();
    for (idx, part) in parts.iter().enumerate() {
        if part.as_os_str() != "i18n" {
            continue;
        }
        let Some(lang) = parts.get(idx + 1).and_then(|p| p.as_os_str().to_str()) else {
            continue;
        };
        if !is_lang(lang) || is_source_lang(lang) {
            continue;
        }
        let mut p = PathBuf::new();
        for part in &parts[..idx] {
            p.push(part.as_os_str());
        }
        for part in &parts[idx + 2..] {
            p.push(part.as_os_str());
        }
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn docusaurus_siblings(source: &Path) -> Vec<Translation> {
    let Some(root) = nearest_existing_parent(source) else {
        return Vec::new();
    };
    let rel = source.strip_prefix(&root).unwrap_or(source);
    LANGS
        .iter()
        .filter(|lang| !is_source_lang(lang))
        .filter_map(|lang| {
            let path = root.join("i18n").join(lang).join(rel);
            path.exists().then(|| Translation {
                path,
                lang: (*lang).into(),
                layout: "docusaurus".into(),
            })
        })
        .collect()
}

fn nearest_existing_parent(path: &Path) -> Option<PathBuf> {
    let mut cur = path.parent()?;
    loop {
        if cur.join("i18n").is_dir() {
            return Some(cur.to_path_buf());
        }
        cur = cur.parent()?;
    }
}

fn detected_language(path: &Path) -> Option<String> {
    if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
        if let Some((base, _ext)) = file_name.rsplit_once('.') {
            if let Some((_stem, lang)) = base.rsplit_once('.') {
                if is_lang(lang) {
                    return Some(lang.to_ascii_lowercase());
                }
            }
        }
    }
    for component in path.components() {
        let Some(s) = component.as_os_str().to_str() else {
            continue;
        };
        let low = s.to_ascii_lowercase();
        if is_lang(&low) {
            return Some(low);
        }
        if let Some(lang) = low.strip_prefix("content.") {
            if is_lang(lang) {
                return Some(lang.into());
            }
        }
    }
    None
}

fn read_structure(path: &Path) -> Result<Structure> {
    let text = std::fs::read_to_string(path)?;
    Ok(parse_structure(&text))
}

fn parse_structure(text: &str) -> Structure {
    let mut structure = Structure::default();
    let mut current_heading: Option<String> = None;
    let parser = Parser::new_ext(text, Options::ENABLE_TABLES | Options::ENABLE_FOOTNOTES);
    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                structure.heading_levels.push(heading_level_number(level));
                current_heading = Some(String::new());
            }
            Event::End(TagEnd::Heading(..)) => {
                if let Some(heading) = current_heading.take() {
                    let heading = heading.split_whitespace().collect::<Vec<_>>().join(" ");
                    structure.headings.push(heading);
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some(heading) = &mut current_heading {
                    heading.push_str(&text);
                    heading.push(' ');
                }
            }
            Event::Start(Tag::CodeBlock(_)) => structure.code_blocks += 1,
            Event::Start(Tag::Link { dest_url, .. }) => {
                let dest = dest_url.trim();
                if !dest.is_empty() {
                    structure.links.push(dest.to_string());
                }
            }
            _ => {}
        }
    }
    structure.links.sort();
    structure.links.dedup();
    structure
}

fn heading_level_number(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn compare_structure(source: &Structure, translation: &Structure) -> Vec<String> {
    let mut issues = Vec::new();
    if source.headings.len() != translation.headings.len() {
        issues.push(format!(
            "heading count differs: source={} translation={}",
            source.headings.len(),
            translation.headings.len()
        ));
    } else if source.heading_levels != translation.heading_levels {
        issues.push(format!(
            "heading level sequence differs: source={} translation={}",
            heading_level_sequence(&source.heading_levels),
            heading_level_sequence(&translation.heading_levels)
        ));
    }
    if source.code_blocks != translation.code_blocks {
        issues.push(format!(
            "code block count differs: source={} translation={}",
            source.code_blocks, translation.code_blocks
        ));
    }
    let source_links: BTreeSet<_> = source.links.iter().collect();
    let translation_links: BTreeSet<_> = translation.links.iter().collect();
    let missing: Vec<_> = source_links.difference(&translation_links).collect();
    if !missing.is_empty() {
        issues.push(format!(
            "missing link target(s): {}",
            missing
                .into_iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    issues
}

fn heading_level_sequence(levels: &[u8]) -> String {
    levels
        .iter()
        .map(|level| format!("h{level}"))
        .collect::<Vec<_>>()
        .join(" > ")
}

fn is_prose_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "md" | "markdown" | "mdx" | "mdc" | "txt" | "html" | "htm"
            )
        })
        .unwrap_or(false)
}

fn is_source_lang(lang: &str) -> bool {
    SOURCE_LANGS.contains(&lang.to_ascii_lowercase().as_str())
}

fn is_lang(lang: &str) -> bool {
    LANGS.contains(&lang.to_ascii_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn suffix_layout_finds_siblings_and_marks_translation() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("README.md");
        let es = dir.path().join("README.es.md");
        std::fs::write(&source, "# Hello\n[Link](./x.md)\n").unwrap();
        std::fs::write(&es, "# Hola\n[Link](./x.md)\n").unwrap();

        assert!(is_translation_file(&es));
        assert!(!is_translation_file(&source));
        assert_eq!(siblings(&source), vec![es]);
    }

    #[test]
    fn lang_dir_layout_finds_siblings() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("docs/en/guide.md");
        let fr = dir.path().join("docs/fr/guide.md");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::create_dir_all(fr.parent().unwrap()).unwrap();
        std::fs::write(&source, "# Hello\n").unwrap();
        std::fs::write(&fr, "# Bonjour\n").unwrap();

        assert!(is_translation_file(&fr));
        assert_eq!(siblings(&source), vec![fr]);
    }

    #[test]
    fn structure_detects_missing_links_and_code_blocks() {
        let source = parse_structure("# A\n```sh\nx\n```\n[API](./api.md)\n");
        let translation = parse_structure("# A\n");
        let issues = compare_structure(&source, &translation);
        assert!(issues.iter().any(|i| i.contains("code block count")));
        assert!(issues.iter().any(|i| i.contains("./api.md")));
    }

    #[test]
    fn structure_detects_heading_level_drift() {
        let source = parse_structure("# A\n\n## B\n\n### C\n");
        let translation = parse_structure("# Uno\n\n### Dos\n\n### Tres\n");
        let issues = compare_structure(&source, &translation);

        assert!(issues
            .iter()
            .any(|issue| issue.contains("heading level sequence")));
    }

    #[test]
    fn structure_detects_reference_style_links() {
        let structure =
            parse_structure("# A `code`\n\nSee [API][api-ref].\n\n[api-ref]: ./api.md\n");
        assert_eq!(structure.headings, vec!["A code"]);
        assert_eq!(structure.heading_levels, vec![1]);
        assert_eq!(structure.links, vec!["./api.md"]);
    }
}
