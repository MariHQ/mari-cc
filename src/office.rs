//! Office document extraction (SPEC §8.5): docx/docm, odt/fodt, rtf, pptx
//! (shapes + tables + speaker notes, per-slide headings), xlsx (computed
//! values, per-sheet) — via `zip` + `quick-xml`, all Rust. Legacy binary
//! `.doc`/`.ppt` are unsupported (§20).

use anyhow::{anyhow, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::io::Read;
use std::path::Path;

pub const OFFICE_EXTS: &[&str] = &["docx", "docm", "odt", "fodt", "rtf", "pptx", "xlsx"];

pub fn is_office_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| OFFICE_EXTS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn extract(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let bytes = std::fs::read(path)?;
    let mut text = match ext.as_str() {
        "docx" | "docm" => docx(&bytes)?,
        "odt" => odt(&bytes)?,
        "fodt" => flat_odt(&bytes)?,
        "rtf" => rtf(&bytes),
        "pptx" => pptx(&bytes)?,
        "xlsx" => xlsx(&bytes)?,
        other => return Err(anyhow!("unsupported office format .{other}")),
    };
    if text.trim().is_empty() {
        return Err(anyhow!("no extractable text in {}", path.display()));
    }
    // Defensive cap (§7.5): a hostile document must not exhaust memory. 32 MB
    // of extracted text is far beyond any legitimate office file.
    const MAX_TEXT: usize = 32 * 1024 * 1024;
    if text.len() > MAX_TEXT {
        text.truncate(MAX_TEXT);
        eprintln!(
            "note: {} extraction truncated at {MAX_TEXT} bytes",
            path.display()
        );
    }
    Ok(text)
}

fn zip_entry(bytes: &[u8], name: &str) -> Result<Option<String>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    if archive.by_name(name).is_err() {
        return Ok(None);
    }
    let mut f = archive.by_name(name)?;
    let mut s = String::new();
    f.read_to_string(&mut s)?;
    Ok(Some(s))
}

fn zip_names(bytes: &[u8], prefix: &str, suffix: &str) -> Result<Vec<String>> {
    let archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    let mut names: Vec<String> = archive
        .file_names()
        .filter(|n| n.starts_with(prefix) && n.ends_with(suffix))
        .map(String::from)
        .collect();
    // Numeric-aware sort so slide10 follows slide9.
    names.sort_by_key(|n| {
        let digits: String = n.chars().filter(|c| c.is_ascii_digit()).collect();
        (digits.parse::<u64>().unwrap_or(0), n.clone())
    });
    Ok(names)
}

/// Pull character data from an XML stream, emitting `\n` at the close of
/// any element in `para_tags` and `\t` at the close of any in `cell_tags`.
fn xml_text(xml: &str, para_tags: &[&str], cell_tags: &[&str]) -> String {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut out = String::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(t)) => {
                if let Ok(s) = t.decode() {
                    out.push_str(&s);
                }
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if para_tags.contains(&name.as_str()) {
                    out.push('\n');
                } else if cell_tags.contains(&name.as_str()) {
                    out.push('\t');
                }
            }
            Ok(Event::Empty(e)) => {
                // <w:br/> and <text:line-break/> are explicit newlines.
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if name == "br" || name == "line-break" || name == "tab" {
                    out.push('\n');
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    // Collapse runs of blank lines.
    let mut collapsed = String::new();
    let mut blanks = 0;
    for line in out.lines() {
        if line.trim().is_empty() {
            blanks += 1;
            if blanks > 1 {
                continue;
            }
        } else {
            blanks = 0;
        }
        collapsed.push_str(line.trim_end());
        collapsed.push('\n');
    }
    collapsed
}

fn docx(bytes: &[u8]) -> Result<String> {
    let xml = zip_entry(bytes, "word/document.xml")?
        .ok_or_else(|| anyhow!("docx missing word/document.xml"))?;
    Ok(xml_text(&xml, &["p"], &["tc"]))
}

fn odt(bytes: &[u8]) -> Result<String> {
    let xml = zip_entry(bytes, "content.xml")?.ok_or_else(|| anyhow!("odt missing content.xml"))?;
    Ok(xml_text(&xml, &["p", "h"], &["table-cell"]))
}

fn flat_odt(bytes: &[u8]) -> Result<String> {
    let xml = String::from_utf8_lossy(bytes);
    Ok(xml_text(&xml, &["p", "h"], &["table-cell"]))
}

/// Minimal RTF: skip control words and groups' binary payloads, unescape
/// the common entities, keep the text stream.
fn rtf(bytes: &[u8]) -> String {
    let src = String::from_utf8_lossy(bytes);
    let mut out = String::new();
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' | '}' => {}
            '\\' => {
                // Control word or escape.
                match chars.peek() {
                    Some('\\') | Some('{') | Some('}') => {
                        out.push(chars.next().unwrap());
                    }
                    Some('\'') => {
                        // \'hh hex escape
                        chars.next();
                        let hex: String = chars.by_ref().take(2).collect();
                        if let Ok(b) = u8::from_str_radix(&hex, 16) {
                            out.push(b as char);
                        }
                    }
                    _ => {
                        let mut word = String::new();
                        while let Some(&n) = chars.peek() {
                            if n.is_ascii_alphabetic() {
                                word.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        }
                        // Numeric parameter.
                        while let Some(&n) = chars.peek() {
                            if n.is_ascii_digit() || n == '-' {
                                chars.next();
                            } else {
                                break;
                            }
                        }
                        // A single space terminates the control word.
                        if chars.peek() == Some(&' ') {
                            chars.next();
                        }
                        if matches!(word.as_str(), "par" | "line" | "sect" | "page") {
                            out.push('\n');
                        } else if word == "tab" {
                            out.push('\t');
                        }
                    }
                }
            }
            '\r' | '\n' => {}
            other => out.push(other),
        }
    }
    out
}

/// pptx: one `## Slide N` per slide (speaker notes appended), text runs
/// from shapes and tables (§8.5).
fn pptx(bytes: &[u8]) -> Result<String> {
    let slides = zip_names(bytes, "ppt/slides/slide", ".xml")?;
    if slides.is_empty() {
        return Err(anyhow!("pptx has no slides"));
    }
    let mut out = String::new();
    for (i, name) in slides.iter().enumerate() {
        let Some(xml) = zip_entry(bytes, name)? else {
            continue;
        };
        out.push_str(&format!("## Slide {}\n\n", i + 1));
        out.push_str(&xml_text(&xml, &["p"], &["tc"]));
        // Speaker notes share the slide number.
        let notes_name = format!(
            "ppt/notesSlides/notesSlide{}.xml",
            name.trim_start_matches("ppt/slides/slide")
                .trim_end_matches(".xml")
        );
        if let Some(notes) = zip_entry(bytes, &notes_name)? {
            let text = xml_text(&notes, &["p"], &[]);
            if !text.trim().is_empty() {
                out.push_str("\nnotes:\n");
                out.push_str(&text);
            }
        }
        out.push('\n');
    }
    Ok(out)
}

/// xlsx: computed values (cached in the sheet XML), per-sheet headings,
/// shared strings resolved by index.
fn xlsx(bytes: &[u8]) -> Result<String> {
    // Shared strings table (t="s" cells index into it).
    let shared: Vec<String> = match zip_entry(bytes, "xl/sharedStrings.xml")? {
        Some(xml) => {
            let mut reader = Reader::from_str(&xml);
            let mut out = Vec::new();
            let mut buf = Vec::new();
            let mut current = String::new();
            let mut depth = 0;
            loop {
                match reader.read_event_into(&mut buf) {
                    Ok(Event::Start(e)) if e.local_name().as_ref() == b"si" => {
                        depth = 1;
                        current.clear();
                    }
                    Ok(Event::Text(t)) if depth == 1 => {
                        if let Ok(s) = t.decode() {
                            current.push_str(&s);
                        }
                    }
                    Ok(Event::End(e)) if e.local_name().as_ref() == b"si" => {
                        depth = 0;
                        out.push(current.clone());
                    }
                    Ok(Event::Eof) => break,
                    Err(_) => break,
                    _ => {}
                }
                buf.clear();
            }
            out
        }
        None => Vec::new(),
    };

    let sheets = zip_names(bytes, "xl/worksheets/sheet", ".xml")?;
    if sheets.is_empty() {
        return Err(anyhow!("xlsx has no worksheets"));
    }
    let mut out = String::new();
    for (i, name) in sheets.iter().enumerate() {
        let Some(xml) = zip_entry(bytes, name)? else {
            continue;
        };
        out.push_str(&format!("## Sheet {}\n\n", i + 1));
        let mut reader = Reader::from_str(&xml);
        let mut buf = Vec::new();
        let mut is_shared = false;
        let mut in_value = false;
        let mut row = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => match e.local_name().as_ref() {
                    b"c" => {
                        is_shared = e.attributes().flatten().any(|a| {
                            a.key.local_name().as_ref() == b"t" && a.value.as_ref() == b"s"
                        });
                    }
                    b"v" | b"t" => in_value = true,
                    _ => {}
                },
                Ok(Event::Text(t)) if in_value => {
                    if let Ok(s) = t.decode() {
                        let cell = if is_shared {
                            s.trim()
                                .parse::<usize>()
                                .ok()
                                .and_then(|idx| shared.get(idx).cloned())
                                .unwrap_or_else(|| s.to_string())
                        } else {
                            s.to_string()
                        };
                        row.push(cell);
                    }
                }
                Ok(Event::End(e)) => match e.local_name().as_ref() {
                    b"v" | b"t" => in_value = false,
                    b"row" => {
                        if !row.is_empty() {
                            out.push_str(&row.join("\t"));
                            out.push('\n');
                        }
                        row.clear();
                    }
                    _ => {}
                },
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default();
            for (name, content) in entries {
                w.start_file(*name, opts).unwrap();
                w.write_all(content.as_bytes()).unwrap();
            }
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn docx_paragraphs_extract() {
        let bytes = make_zip(&[(
            "word/document.xml",
            r#"<w:document xmlns:w="x"><w:body>
                <w:p><w:r><w:t>The enterprise plan</w:t></w:r><w:r><w:t> costs $49.</w:t></w:r></w:p>
                <w:p><w:r><w:t>Second paragraph.</w:t></w:r></w:p>
               </w:body></w:document>"#,
        )]);
        let tmp = tempfile::NamedTempFile::with_suffix(".docx").unwrap();
        std::fs::write(tmp.path(), &bytes).unwrap();
        let text = extract(tmp.path()).unwrap();
        assert!(text.contains("The enterprise plan costs $49."));
        assert!(text.contains("Second paragraph."));
    }

    #[test]
    fn pptx_slides_and_notes() {
        let bytes = make_zip(&[
            (
                "ppt/slides/slide1.xml",
                r#"<p:sld xmlns:a="x"><a:p><a:r><a:t>Roadmap Q3</a:t></a:r></a:p></p:sld>"#,
            ),
            (
                "ppt/notesSlides/notesSlide1.xml",
                r#"<p:notes xmlns:a="x"><a:p><a:r><a:t>mention pricing</a:t></a:r></a:p></p:notes>"#,
            ),
        ]);
        let tmp = tempfile::NamedTempFile::with_suffix(".pptx").unwrap();
        std::fs::write(tmp.path(), &bytes).unwrap();
        let text = extract(tmp.path()).unwrap();
        assert!(text.contains("## Slide 1"));
        assert!(text.contains("Roadmap Q3"));
        assert!(text.contains("mention pricing"));
    }

    #[test]
    fn xlsx_shared_strings_resolve() {
        let bytes = make_zip(&[
            (
                "xl/sharedStrings.xml",
                r#"<sst><si><t>Plan</t></si><si><t>Enterprise</t></si></sst>"#,
            ),
            (
                "xl/worksheets/sheet1.xml",
                r#"<worksheet><sheetData>
                    <row><c t="s"><v>0</v></c><c t="s"><v>1</v></c><c><v>49</v></c></row>
                   </sheetData></worksheet>"#,
            ),
        ]);
        let tmp = tempfile::NamedTempFile::with_suffix(".xlsx").unwrap();
        std::fs::write(tmp.path(), &bytes).unwrap();
        let text = extract(tmp.path()).unwrap();
        assert!(text.contains("## Sheet 1"));
        assert!(text.contains("Plan\tEnterprise\t49"));
    }

    #[test]
    fn rtf_strips_control_words() {
        let tmp = tempfile::NamedTempFile::with_suffix(".rtf").unwrap();
        std::fs::write(
            tmp.path(),
            br"{\rtf1\ansi{\fonttbl\f0 Helvetica;}\f0\fs24 Hello {\b bold} world.\par Second line.}",
        )
        .unwrap();
        let text = extract(tmp.path()).unwrap();
        assert!(text.contains("Hello bold world."), "{text}");
        assert!(text.contains("Second line."));
    }

    #[test]
    fn odt_and_flat_odt() {
        let content = r#"<office:document-content xmlns:text="t"><office:body>
            <text:h>Title</text:h><text:p>Body paragraph.</text:p>
           </office:body></office:document-content>"#;
        let bytes = make_zip(&[("content.xml", content)]);
        let tmp = tempfile::NamedTempFile::with_suffix(".odt").unwrap();
        std::fs::write(tmp.path(), &bytes).unwrap();
        assert!(extract(tmp.path()).unwrap().contains("Body paragraph."));

        let tmp = tempfile::NamedTempFile::with_suffix(".fodt").unwrap();
        std::fs::write(tmp.path(), content).unwrap();
        assert!(extract(tmp.path()).unwrap().contains("Body paragraph."));
    }
}
