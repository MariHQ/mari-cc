//! Detector context: masking, segmentation, and markdown structure
//! extraction per SPEC §11.0.2–§11.0.4. All offsets are byte offsets into
//! the original text; masking replaces bytes with ASCII spaces (newlines
//! preserved) so offsets survive.

use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Heading {
    pub level: usize,
    pub text: String,
    pub line: usize,
    pub start: usize,
    #[allow(dead_code)] // §11.0.4 contract
    pub raw: String,
}

#[derive(Debug, Clone)]
pub struct ListItem {
    #[allow(dead_code)] // §11.0.4 contract
    pub indent: usize,
    #[allow(dead_code)] // §11.0.4 contract
    pub marker: String,
    pub text: String,
    pub line: usize,
    /// Byte offset of the item's text (after the marker).
    pub start: usize,
    /// Byte offset of the line start.
    pub line_start: usize,
}

#[derive(Debug, Clone)]
pub struct Link {
    pub text: String,
    pub target: String,
    pub start: usize,
    pub length: usize,
}

#[derive(Debug, Clone)]
pub struct Image {
    pub alt: String,
    #[allow(dead_code)] // §11.0.4 contract
    pub target: String,
    pub start: usize,
    pub length: usize,
}

#[derive(Debug, Clone)]
pub struct BoldSpan {
    #[allow(dead_code)] // §11.0.4 contract
    pub text: String,
    pub start: usize,
    pub length: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Paragraph,
    Heading,
    ListItem,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub kind: BlockKind,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct Sentence {
    pub start: usize,
    pub end: usize,
    pub words: usize,
}

#[derive(Debug, Clone)]
pub struct RefDef {
    pub id: String,
    pub line: usize,
    pub start: usize,
    #[allow(dead_code)] // §11.0.4 contract
    pub url: String,
}

pub struct Ctx {
    pub text: String,
    pub masked: String,
    #[allow(dead_code)] // §11.0.1 contract
    pub path: String,
    line_starts: Vec<usize>,
    #[allow(dead_code)] // §11.0.3 contract
    pub blocks: Vec<Block>,
    pub sentences: Vec<Sentence>,
    pub word_count: usize,
    pub headings: Vec<Heading>,
    pub list_items: Vec<ListItem>,
    pub links: Vec<Link>,
    pub images: Vec<Image>,
    pub bold_spans: Vec<BoldSpan>,
    /// Line numbers (0-based) of thematic breaks.
    pub thematic_breaks: Vec<usize>,
    /// Line numbers (0-based) considered table lines.
    pub table_lines: HashSet<usize>,
    pub ref_defs: Vec<RefDef>,
    pub ref_uses: HashSet<String>,
    /// Byte offsets of opening code fences that carry no language token.
    pub fence_openers_no_lang: Vec<usize>,
    pub style_guide: String,
    /// Glossary variant groups from STYLE.md (each inner vec: [use, not...]).
    pub glossary_groups: Vec<Vec<String>>,
    /// PRODUCT.md reading-grade target, if configured.
    pub reading_grade_target: Option<f64>,
}

const ABBREVIATIONS: &[&str] = &[
    "mr", "mrs", "ms", "dr", "prof", "sr", "jr", "st", "vs", "etc", "inc", "ltd", "co", "no",
    "fig", "al", "eg", "ie", "e.g", "i.e", "u.s", "u.k", "a.m", "p.m", "approx",
];

impl Ctx {
    pub fn build(path: &str, text: &str, style_guide: &str) -> Ctx {
        let masked = mask(text);
        let line_starts = line_starts(text);
        let structure = extract_structure(text, &masked, &line_starts);
        let blocks = split_blocks(&masked, &line_starts);
        let sentences = split_sentences(&masked, &blocks);
        let word_count = count_words(&masked);
        Ctx {
            text: text.to_string(),
            masked,
            path: path.to_string(),
            line_starts,
            blocks,
            sentences,
            word_count,
            headings: structure.headings,
            list_items: structure.list_items,
            links: structure.links,
            images: structure.images,
            bold_spans: structure.bold_spans,
            thematic_breaks: structure.thematic_breaks,
            table_lines: structure.table_lines,
            ref_defs: structure.ref_defs,
            ref_uses: structure.ref_uses,
            fence_openers_no_lang: structure.fence_openers_no_lang,
            style_guide: style_guide.to_string(),
            glossary_groups: Vec::new(),
            reading_grade_target: None,
        }
    }

    /// (line, col), both 1-based, for a byte offset.
    pub fn locate(&self, offset: usize) -> (usize, usize) {
        let line = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let col = self.text[self.line_starts[line]..offset].chars().count() + 1;
        (line + 1, col)
    }

    /// 0-based line index for a byte offset.
    pub fn line_of(&self, offset: usize) -> usize {
        match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        }
    }

    pub fn is_table_line(&self, offset: usize) -> bool {
        self.table_lines.contains(&self.line_of(offset))
    }

    /// Whether `offset` starts a sentence: walk left over blockquote/list/
    /// emphasis markers; the preceding significant char must be `.!?`,
    /// a newline, or start-of-file (SPEC §11.0.3).
    pub fn is_sentence_start(&self, offset: usize) -> bool {
        let bytes = self.masked.as_bytes();
        let mut i = offset;
        while i > 0 {
            let c = bytes[i - 1];
            if matches!(c, b' ' | b'\t' | b'>' | b'*' | b'_' | b'#' | b'-') {
                i -= 1;
            } else {
                break;
            }
        }
        if i == 0 {
            return true;
        }
        matches!(bytes[i - 1], b'.' | b'!' | b'?' | b'\n')
    }

    pub fn count_words(&self, s: &str) -> usize {
        count_words(s)
    }

    /// ≤80-char whitespace-collapsed span from the ORIGINAL text.
    pub fn span_at(&self, offset: usize, length: usize) -> String {
        let start = floor_char(&self.text, offset.min(self.text.len()));
        let end = floor_char(&self.text, (offset + length).min(self.text.len()));
        let raw = &self.text[start..end];
        let collapsed: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        collapsed.chars().take(80).collect()
    }

    /// The raw line (from the original text) containing this 0-based index.
    pub fn line_text(&self, line: usize) -> &str {
        let start = self.line_starts[line];
        let end = self
            .line_starts
            .get(line + 1)
            .map(|e| e - 1)
            .unwrap_or(self.text.len());
        &self.text[start..end.max(start)]
    }

    pub fn line_start_offset(&self, line: usize) -> usize {
        self.line_starts[line]
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// SPEC §11.0.2: two rules verify the matched span is byte-identical in
    /// text and masked — a mismatch means the match spans a blanked hole.
    pub fn masked_identical(&self, offset: usize, length: usize) -> bool {
        let end = (offset + length).min(self.text.len());
        self.text.as_bytes()[offset..end] == self.masked.as_bytes()[offset..end]
    }
}

fn floor_char(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

pub fn count_words(s: &str) -> usize {
    // Tokens: [A-Za-z0-9]+(?:['’-][A-Za-z0-9]+)*
    let re = word_token_re();
    re.find_iter(s).count()
}

pub fn word_token_re() -> &'static regex::Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"[A-Za-z0-9]+(?:['’-][A-Za-z0-9]+)*").unwrap())
}

fn line_starts(text: &str) -> Vec<usize> {
    let mut v = vec![0usize];
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            v.push(i + 1);
        }
    }
    if *v.last().unwrap() > text.len() {
        v.pop();
    }
    v
}

/// Blank a byte range with spaces, preserving newlines.
fn blank(buf: &mut [u8], start: usize, end: usize) {
    let end = end.min(buf.len());
    for b in buf[start..end].iter_mut() {
        if *b != b'\n' {
            *b = b' ';
        }
    }
}

/// SPEC §11.0.2 masking: front matter, fenced code, inline code, HTML
/// comments, shortcodes, inline HTML tags — replaced with equal-length
/// spaces (newlines preserved).
pub fn mask(text: &str) -> String {
    let mut buf = text.as_bytes().to_vec();

    // 1. Front matter at file start.
    let fm = regex::Regex::new(r"\A(---|\+\+\+)[ \t]*\n").unwrap();
    if let Some(m) = fm.find(text) {
        let delim = &text[m.start()..m.start() + 3];
        // find closing delimiter on its own line
        let rest = &text[m.end()..];
        let close = regex::Regex::new(&format!(r"(?m)^{}[ \t]*$", regex::escape(delim))).unwrap();
        if let Some(c) = close.find(rest) {
            let end_line = m.end() + c.end();
            let end = text[end_line..]
                .find('\n')
                .map(|i| end_line + i + 1)
                .unwrap_or(text.len());
            blank(&mut buf, 0, end);
        }
    }

    // 2. Fenced code blocks (line-based state machine; unterminated fence
    // masks to EOF). Fence lines themselves are masked too.
    {
        let mut in_fence: Option<(u8, usize)> = None; // (fence char, min len)
        let mut offset = 0usize;
        for line in text.split_inclusive('\n') {
            let already_masked = buf[offset..offset + line.len()]
                .iter()
                .all(|&b| b == b' ' || b == b'\n');
            if !already_masked {
                let trimmed = line.trim_start();
                let lead = line.len() - trimmed.len();
                let fence_char = trimmed.as_bytes().first().copied();
                let is_fence = lead <= 3
                    && matches!(fence_char, Some(b'`') | Some(b'~'))
                    && trimmed
                        .bytes()
                        .take_while(|&b| Some(b) == fence_char)
                        .count()
                        >= 3;
                match (&in_fence, is_fence) {
                    (None, true) => {
                        let ch = fence_char.unwrap();
                        let len = trimmed.bytes().take_while(|&b| b == ch).count();
                        in_fence = Some((ch, len));
                        blank(&mut buf, offset, offset + line.len());
                    }
                    (Some((ch, len)), true) => {
                        let tch = fence_char.unwrap();
                        let tlen = trimmed.bytes().take_while(|&b| b == tch).count();
                        blank(&mut buf, offset, offset + line.len());
                        if tch == *ch && tlen >= *len {
                            in_fence = None;
                        }
                    }
                    (Some(_), false) => {
                        blank(&mut buf, offset, offset + line.len());
                    }
                    (None, false) => {}
                }
            } else if in_fence.is_some() {
                blank(&mut buf, offset, offset + line.len());
            }
            offset += line.len();
        }
    }

    // Work on the intermediate masked string for the remaining passes so
    // patterns can't match inside already-blanked regions.
    let mid = String::from_utf8(buf.clone()).unwrap();

    // 3. Inline code `…` (single line).
    let inline = fancy_regex::Regex::new(r"(`{1,3})[^`\n]+?\1").unwrap();
    for m in inline.find_iter(&mid).flatten() {
        blank(&mut buf, m.start(), m.end());
    }
    // 4. HTML comments.
    let comment = regex::Regex::new(r"<!--[\s\S]*?-->").unwrap();
    for m in comment.find_iter(&mid) {
        blank(&mut buf, m.start(), m.end());
    }
    // 5. Template shortcodes {{ … }} (Hugo/Liquid).
    let shortcode = regex::Regex::new(r"\{\{[\s\S]*?\}\}").unwrap();
    for m in shortcode.find_iter(&mid) {
        blank(&mut buf, m.start(), m.end());
    }
    // 6. Inline HTML tags.
    let tag = regex::Regex::new(r"</?[a-zA-Z][^>\n]*>").unwrap();
    for m in tag.find_iter(&mid) {
        blank(&mut buf, m.start(), m.end());
    }

    String::from_utf8(buf).unwrap()
}

struct Structure {
    headings: Vec<Heading>,
    list_items: Vec<ListItem>,
    links: Vec<Link>,
    images: Vec<Image>,
    bold_spans: Vec<BoldSpan>,
    thematic_breaks: Vec<usize>,
    table_lines: HashSet<usize>,
    ref_defs: Vec<RefDef>,
    ref_uses: HashSet<String>,
    fence_openers_no_lang: Vec<usize>,
}

fn extract_structure(text: &str, masked: &str, line_starts: &[usize]) -> Structure {
    let heading_re = regex::Regex::new(r"^(\s{0,3})(#{1,6})\s+(.*?)\s*#*\s*$").unwrap();
    let list_re = regex::Regex::new(r"^(\s*)([-*+]|\d+[.)])\s+(.*)$").unwrap();
    // No backreferences in the `regex` crate — expand the ([-*_])(\s*\1){2,}
    // pattern into one alternation per break char.
    let break_re =
        regex::Regex::new(r"^\s{0,3}(?:-(?:\s*-){2,}|\*(?:\s*\*){2,}|_(?:\s*_){2,})\s*$").unwrap();
    let sep_row_re =
        regex::Regex::new(r"^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)+\|?\s*$").unwrap();
    let refdef_re = regex::Regex::new(r"^\s{0,3}\[([^\]]+)\]:\s+(\S+)").unwrap();

    let mut s = Structure {
        headings: Vec::new(),
        list_items: Vec::new(),
        links: Vec::new(),
        images: Vec::new(),
        bold_spans: Vec::new(),
        thematic_breaks: Vec::new(),
        table_lines: HashSet::new(),
        ref_defs: Vec::new(),
        ref_uses: HashSet::new(),
        fence_openers_no_lang: Vec::new(),
    };

    // Fence-opener detection for `fenced-code-language` runs on RAW text
    // (fences are masked): alternate open/close.
    {
        let mut open = false;
        let mut offset = 0usize;
        for line in text.split_inclusive('\n') {
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                if !open {
                    let after = trimmed.trim_start_matches(['`', '~']).trim();
                    if after.is_empty() {
                        s.fence_openers_no_lang
                            .push(offset + (line.len() - trimmed.len()));
                    }
                    open = true;
                } else {
                    open = false;
                }
            }
            offset += line.len();
        }
    }

    for (lineno, &start) in line_starts.iter().enumerate() {
        let end = line_starts
            .get(lineno + 1)
            .map(|e| e - 1)
            .unwrap_or(text.len());
        if end < start {
            continue;
        }
        let raw = &text[start..end];
        let m = &masked[start..end];
        // A line fully blanked by masking is skipped.
        if !raw.trim().is_empty() && m.trim().is_empty() {
            continue;
        }

        if let Some(c) = heading_re.captures(m) {
            let g2 = c.get(2).unwrap();
            let g3 = c.get(3).unwrap();
            s.headings.push(Heading {
                level: g2.as_str().len(),
                text: raw[g3.start()..g3.end()].to_string(),
                line: lineno,
                start: start + g3.start(),
                raw: raw.to_string(),
            });
        } else if let Some(c) = list_re.captures(m) {
            let g1 = c.get(1).unwrap();
            let g2 = c.get(2).unwrap();
            let g3 = c.get(3).unwrap();
            s.list_items.push(ListItem {
                indent: g1.as_str().len(),
                marker: g2.as_str().to_string(),
                text: raw[g3.start()..g3.end()].to_string(),
                line: lineno,
                start: start + g3.start(),
                line_start: start,
            });
        }

        if break_re.is_match(m) {
            s.thematic_breaks.push(lineno);
        }

        // Table lines: starts with |, separator row, or ≥2 pipes.
        let pipes = m.matches('|').count();
        if m.trim_start().starts_with('|') || sep_row_re.is_match(m) || pipes >= 2 {
            s.table_lines.insert(lineno);
        }

        if let Some(c) = refdef_re.captures(m) {
            let id = c.get(1).unwrap();
            s.ref_defs.push(RefDef {
                id: raw.get(id.start()..id.end()).unwrap_or("").to_lowercase(),
                line: lineno,
                start,
                url: c.get(2).unwrap().as_str().to_string(),
            });
        }
    }

    // Images first (so links skip them), then links — over masked text.
    let img_re = regex::Regex::new(r"!\[([^\]]*)\]\(([^)\s]*)[^)]*\)").unwrap();
    let mut image_starts: HashSet<usize> = HashSet::new();
    for c in img_re.captures_iter(masked) {
        let whole = c.get(0).unwrap();
        image_starts.insert(whole.start() + 1); // where the [ sits
        s.images.push(Image {
            alt: text[c.get(1).unwrap().range()].to_string(),
            target: c.get(2).unwrap().as_str().to_string(),
            start: whole.start(),
            length: whole.len(),
        });
    }
    let link_re = regex::Regex::new(r"\[([^\]]*)\]\(([^)\s]*)[^)]*\)").unwrap();
    for c in link_re.captures_iter(masked) {
        let whole = c.get(0).unwrap();
        if whole.start() > 0 && masked.as_bytes()[whole.start() - 1] == b'!' {
            continue;
        }
        if image_starts.contains(&whole.start()) {
            continue;
        }
        s.links.push(Link {
            text: text[c.get(1).unwrap().range()].to_string(),
            target: c.get(2).unwrap().as_str().to_string(),
            start: whole.start(),
            length: whole.len(),
        });
    }

    // Bold spans (single line).
    let bold_re = regex::Regex::new(r"\*\*([^*\n]+)\*\*|__([^_\n]+)__").unwrap();
    for c in bold_re.captures_iter(masked) {
        let whole = c.get(0).unwrap();
        let inner = c.get(1).or_else(|| c.get(2)).unwrap();
        s.bold_spans.push(BoldSpan {
            text: text[inner.range()].to_string(),
            start: whole.start(),
            length: whole.len(),
        });
    }

    // Reference uses: `][id]` plus shortcut `[id]` (not followed by [, (, :;
    // not an image).
    let use_re = fancy_regex::Regex::new(r"\]\[([^\]]+)\]").unwrap();
    for c in use_re.captures_iter(masked).flatten() {
        s.ref_uses.insert(c.get(1).unwrap().as_str().to_lowercase());
    }
    let shortcut_re = fancy_regex::Regex::new(r"(?<!\!)\[([^\]\[]+)\](?![\[(:])").unwrap();
    for c in shortcut_re.captures_iter(masked).flatten() {
        s.ref_uses.insert(c.get(1).unwrap().as_str().to_lowercase());
    }

    s
}

fn split_blocks(masked: &str, line_starts: &[usize]) -> Vec<Block> {
    let heading_re = regex::Regex::new(r"^\s{0,3}#{1,6}\s").unwrap();
    let list_re = regex::Regex::new(r"^\s*([-*+]|\d+[.)])\s").unwrap();
    let mut blocks: Vec<Block> = Vec::new();
    let mut para_start: Option<usize> = None;
    let mut prev_end = 0usize;

    for (lineno, &start) in line_starts.iter().enumerate() {
        let end = line_starts
            .get(lineno + 1)
            .map(|e| e - 1)
            .unwrap_or(masked.len());
        let line = &masked[start..end.max(start)];
        let blank_line = line.trim().is_empty();

        if blank_line {
            if let Some(ps) = para_start.take() {
                blocks.push(Block {
                    kind: BlockKind::Paragraph,
                    start: ps,
                    end: prev_end,
                });
            }
        } else if heading_re.is_match(line) {
            if let Some(ps) = para_start.take() {
                blocks.push(Block {
                    kind: BlockKind::Paragraph,
                    start: ps,
                    end: prev_end,
                });
            }
            blocks.push(Block {
                kind: BlockKind::Heading,
                start,
                end,
            });
        } else if list_re.is_match(line) {
            if let Some(ps) = para_start.take() {
                blocks.push(Block {
                    kind: BlockKind::Paragraph,
                    start: ps,
                    end: prev_end,
                });
            }
            blocks.push(Block {
                kind: BlockKind::ListItem,
                start,
                end,
            });
        } else if para_start.is_none() {
            para_start = Some(start);
        }
        prev_end = end;
    }
    if let Some(ps) = para_start {
        blocks.push(Block {
            kind: BlockKind::Paragraph,
            start: ps,
            end: prev_end,
        });
    }
    blocks
}

fn split_sentences(masked: &str, blocks: &[Block]) -> Vec<Sentence> {
    let term_re = regex::Regex::new(r#"[.!?]+["')\]”’]?(\s+|$)"#).unwrap();
    let mut out = Vec::new();
    for b in blocks {
        if b.kind == BlockKind::Heading {
            continue;
        }
        let seg = &masked[b.start..b.end];
        let mut cursor = 0usize;
        for m in term_re.find_iter(seg) {
            let term_start = m.start();
            // Decimal suppression: digit before '.', digit after.
            let bytes = seg.as_bytes();
            if bytes[term_start] == b'.'
                && term_start > 0
                && bytes[term_start - 1].is_ascii_digit()
                && bytes.get(m.end()).map_or(false, |c| c.is_ascii_digit())
            {
                continue;
            }
            // Abbreviation suppression: trailing word before the period.
            if bytes[term_start] == b'.' {
                let before = &seg[cursor..term_start];
                let last_word = before
                    .rsplit(|c: char| c.is_whitespace() || c == '(')
                    .next()
                    .unwrap_or("")
                    .to_lowercase();
                if ABBREVIATIONS.contains(&last_word.as_str())
                    || ABBREVIATIONS.contains(&last_word.trim_end_matches('.'))
                {
                    continue;
                }
            }
            let s_start = b.start + cursor;
            let s_end = b.start + m.end();
            let text_slice = &seg[cursor..m.end()];
            let words = count_words(text_slice);
            if words > 0 {
                out.push(Sentence {
                    start: s_start,
                    end: s_end,
                    words,
                });
            }
            cursor = m.end();
        }
        // Trailing text after the last terminator.
        if cursor < seg.len() && !seg[cursor..].trim().is_empty() {
            let words = count_words(&seg[cursor..]);
            if words > 0 {
                out.push(Sentence {
                    start: b.start + cursor,
                    end: b.end,
                    words,
                });
            }
        }
    }
    out
}
