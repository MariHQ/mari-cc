//! Family C — shared style rules (SPEC §11.3 plus citation hygiene).

use super::ctx::Ctx;
use super::helpers::{self, me, MapEntry};
use super::{Emitter, Family, Rule, Severity};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

const FAM: Family = Family::Style;

fn r(id: &'static str, run: fn(&Ctx, &mut Emitter)) -> Rule {
    Rule {
        id,
        family: FAM,
        pack: None,
        run,
    }
}

pub fn rules() -> Vec<Rule> {
    vec![
        r("sentence-case-heading", sentence_case_heading),
        r("heading-end-punctuation", heading_end_punctuation),
        r("word-swap", word_swap),
        r("serial-comma", serial_comma),
        r("intro-comma", intro_comma),
        r("use-contractions", use_contractions),
        r("second-person", second_person),
        r("present-tense", present_tense),
        r("singular-they", singular_they),
        r("no-please-instructions", no_please_instructions),
        r("terminology-consistency", terminology_consistency),
        r("acronym-case", acronym_case),
        r("acronym-plural", acronym_plural),
        r("fenced-code-language", fenced_code_language),
        r("duplicate-heading", duplicate_heading),
        r("markup-leak", markup_leak),
        r("inconsistent-capitalization", inconsistent_capitalization),
        r("malformed-doi-isbn", malformed_doi_isbn),
        r(
            "thematic-break-before-heading",
            thematic_break_before_heading,
        ),
        r("bullet-overuse", bullet_overuse),
        r("double-space", double_space),
        r("redundant-acronym", redundant_acronym),
        r("indefinite-article", indefinite_article),
        r("placeholder-citation", placeholder_citation),
        r("tracking-param-in-citation", tracking_param_in_citation),
        r("unused-named-ref", unused_named_ref),
    ]
}

fn sentence_case_heading(ctx: &Ctx, em: &mut Emitter) {
    static SMALL: &[&str] = &[
        "a", "an", "the", "and", "or", "but", "for", "nor", "of", "to", "in", "on", "at", "by",
        "as", "is", "are", "with", "from", "into", "via", "per", "vs",
    ];
    for h in &ctx.headings {
        let lead = h.text.split([':', '—']).next().unwrap_or(&h.text);
        let words: Vec<&str> = lead.split_whitespace().collect();
        if words.len() < 3 {
            continue;
        }
        let mut capped = 0;
        for w in words.iter().skip(1) {
            let clean = w.trim_matches(|c: char| !c.is_alphanumeric());
            if clean.chars().all(|c| c.is_ascii_uppercase())
                || SMALL.contains(&clean.to_lowercase().as_str())
            {
                continue;
            }
            if clean
                .chars()
                .next()
                .map(|c| c.is_ascii_uppercase())
                .unwrap_or(false)
            {
                capped += 1;
            }
        }
        if capped >= 2 {
            let rewrite = sentence_case(&h.text);
            em.emit(
                ctx,
                "sentence-case-heading",
                FAM,
                Severity::Advisory,
                h.start,
                h.text.len(),
                format!("heading should use sentence case — use '{rewrite}'"),
            );
        }
    }
}

fn sentence_case(s: &str) -> String {
    let mut out = String::new();
    for (i, w) in s.split_whitespace().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        if i == 0 || w.chars().all(|c| !c.is_ascii_lowercase()) {
            out.push_str(w);
        } else {
            out.push_str(&w.to_lowercase());
        }
    }
    out
}

fn heading_end_punctuation(ctx: &Ctx, em: &mut Emitter) {
    for h in &ctx.headings {
        if h.text.trim_end().ends_with(['.', ':', '!']) {
            em.emit(
                ctx,
                "heading-end-punctuation",
                FAM,
                Severity::Warn,
                h.start,
                h.text.len(),
                "heading ends with punctuation — remove it",
            );
        }
    }
}

pub const SWAPS: &[MapEntry] = &[
    me("leverage", "use"),
    me("e.g.", "for example"),
    me("i.e.", "that is"),
    me("etc", "and so on"),
    me("execute", "run"),
    me("grayed out", "unavailable"),
    me("and/or", "or"),
    me("deselect", "clear"),
    me("login", "sign in (verb)"),
    me("log in", "sign in"),
    me("e-mail", "email"),
    me("check box", "checkbox"),
    me("drop-down", "dropdown"),
];

fn word_swap(ctx: &Ctx, em: &mut Emitter) {
    let re = ctx.lists.map_regex("style-swap");
    helpers::scan_fancy(ctx, &re, |off, len, m| {
        let lower = m.to_lowercase();
        if matches!(ctx.style_guide.as_str(), "microsoft") && (lower == "e.g." || lower == "i.e.") {
            return;
        }
        if matches!(ctx.style_guide.as_str(), "google")
            && (lower == "e.g." || lower == "i.e." || lower == "etc")
        {
            return;
        }
        if let Some(to) = ctx.lists.map_lookup("style-swap", m) {
            em.emit(
                ctx,
                "word-swap",
                FAM,
                Severity::Advisory,
                off,
                len,
                format!("style swap — use '{to}'"),
            );
        }
    });
}

fn serial_comma(ctx: &Ctx, em: &mut Emitter) {
    if ctx.style_guide == "ap" {
        return;
    }
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"\b[A-Za-z]+,\s+[A-Za-z]+\s+(and|or)\s+[A-Za-z]+\b").unwrap()
    });
    helpers::scan(ctx, re, |off, len, _| {
        if !ctx.is_sentence_start(off) {
            em.emit(
                ctx,
                "serial-comma",
                FAM,
                Severity::Advisory,
                off,
                len,
                "missing serial comma",
            );
        }
    });
}

fn intro_comma(ctx: &Ctx, em: &mut Emitter) {
    static ADV: OnceLock<regex::Regex> = OnceLock::new();
    let adv = ADV.get_or_init(|| regex::RegexBuilder::new(
        r"(?m)^[ \t>*_#-]*(moreover|furthermore|nevertheless|nonetheless|consequently|meanwhile|additionally|therefore|conversely|accordingly)\s+[A-Za-z]"
    ).case_insensitive(true).build().unwrap());
    helpers::scan(ctx, adv, |off, len, _| {
        em.emit(
            ctx,
            "intro-comma",
            FAM,
            Severity::Advisory,
            off,
            len,
            "introductory adverb needs a comma",
        );
    });
}

pub const CONTRACTIONS: &[MapEntry] = &[
    me("do not", "don't"),
    me("does not", "doesn't"),
    me("did not", "didn't"),
    me("is not", "isn't"),
    me("are not", "aren't"),
    me("was not", "wasn't"),
    me("were not", "weren't"),
    me("cannot", "can't"),
    me("can not", "can't"),
    me("will not", "won't"),
    me("would not", "wouldn't"),
    me("should not", "shouldn't"),
    me("could not", "couldn't"),
    me("have not", "haven't"),
    me("has not", "hasn't"),
];

fn use_contractions(ctx: &Ctx, em: &mut Emitter) {
    let re = ctx.lists.map_regex("contraction");
    helpers::scan_fancy(ctx, &re, |off, len, m| {
        if let Some(to) = ctx.lists.map_lookup("contraction", m) {
            em.emit(
                ctx,
                "use-contractions",
                FAM,
                Severity::Advisory,
                off,
                len,
                format!("use the contraction '{to}'"),
            );
        }
    });
}

fn second_person(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::RegexBuilder::new(
        r"\b(the user|users)\s+(should|can|must|may|need to|needs to|will|might|have|has|access|get)\b"
    ).case_insensitive(true).build().unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "second-person",
            FAM,
            Severity::Advisory,
            off,
            len,
            "use second person — address the reader as 'you'",
        );
    });
}

fn present_tense(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::RegexBuilder::new(r"\byou will\s+[a-z]+\b")
            .case_insensitive(true)
            .build()
            .unwrap()
    });
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "present-tense",
            FAM,
            Severity::Advisory,
            off,
            len,
            "use present tense — 'you <verb>'",
        );
    });
}

pub const SINGULAR_THEY: &[MapEntry] = &[
    me("he or she", "they"),
    me("she or he", "they"),
    me("his or her", "their"),
    me("her or his", "their"),
    me("him or her", "them"),
    me("he/she", "they"),
    me("(s)he", "they"),
    me("s/he", "they"),
    me("his/her", "their"),
];

fn singular_they(ctx: &Ctx, em: &mut Emitter) {
    let re = ctx.lists.map_regex("singular-they");
    helpers::scan_fancy(ctx, &re, |off, len, m| {
        if let Some(to) = ctx.lists.map_lookup("singular-they", m) {
            em.emit(
                ctx,
                "singular-they",
                FAM,
                Severity::Warn,
                off,
                len,
                format!("use singular they — use '{to}'"),
            );
        }
    });
}

fn no_please_instructions(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| helpers::word_list(&["please"]));
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "no-please-instructions",
            FAM,
            Severity::Advisory,
            off,
            len,
            "drop 'please' in instructions",
        );
    });
}

pub const TERM_GROUPS: &[&[&str]] = &[
    &["sign in", "log in", "login"],
    &["email", "e-mail"],
    &["dropdown", "drop-down"],
    &["website", "web site"],
    &["checkbox", "check box"],
    &["filename", "file name"],
    &["setup", "set-up"],
    &["username", "user name"],
];

fn terminology_consistency(ctx: &Ctx, em: &mut Emitter) {
    let mut groups: Vec<Vec<String>> = ctx.lists.groups("terminology-consistency").as_ref().clone();
    groups.extend(ctx.glossary_groups.clone());
    for group in groups {
        let terms: Vec<&str> = group.iter().map(|s| s.as_str()).collect();
        let re = helpers::phrase_list(&terms);
        let mut seen: Vec<(String, usize, usize)> = Vec::new();
        helpers::scan_fancy(ctx, &re, |off, len, m| {
            let lower = m.to_lowercase();
            if !seen.iter().any(|(s, _, _)| s == &lower) {
                seen.push((lower, off, len));
            }
        });
        if seen.len() >= 2 {
            let (_, off, len) = seen[1].clone();
            em.emit(
                ctx,
                "terminology-consistency",
                FAM,
                Severity::Advisory,
                off,
                len,
                format!(
                    "mixed terminology — choose one form, preferably '{}'",
                    group[0]
                ),
            );
        }
    }
}

fn acronym_case(ctx: &Ctx, em: &mut Emitter) {
    static UPPER: OnceLock<regex::Regex> = OnceLock::new();
    let upper = UPPER.get_or_init(|| regex::Regex::new(r"\b[A-Z]{2,5}\b").unwrap());
    let mut uppers = HashSet::new();
    for m in upper.find_iter(&ctx.masked) {
        uppers.insert(m.as_str().to_string());
    }
    for acro in uppers {
        let lower = acro.to_lowercase();
        if [
            "note", "tip", "info", "warning", "error", "debug", "and", "or", "not", "null", "true",
            "false", "get", "put", "post", "head", "ok", "us",
        ]
        .contains(&lower.as_str())
        {
            continue;
        }
        let re = regex::Regex::new(&format!(r"\b{}\b", regex::escape(&lower))).unwrap();
        if let Some(m) = re.find(&ctx.masked) {
            em.emit(
                ctx,
                "acronym-case",
                FAM,
                Severity::Advisory,
                m.start(),
                m.len(),
                format!("inconsistent acronym case — use '{acro}'"),
            );
        }
    }
}

fn acronym_plural(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\b[A-Z]{2,5}'s\b").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "acronym-plural",
            FAM,
            Severity::Advisory,
            off,
            len,
            "use plain s for acronym plurals; keep apostrophe-s for possessives",
        );
    });
}

fn fenced_code_language(ctx: &Ctx, em: &mut Emitter) {
    for &off in &ctx.fence_openers_no_lang {
        em.emit(
            ctx,
            "fenced-code-language",
            FAM,
            Severity::Advisory,
            off,
            3,
            "code fence missing a language",
        );
    }
}

fn duplicate_heading(ctx: &Ctx, em: &mut Emitter) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    for h in &ctx.headings {
        let key = h.text.trim().to_lowercase();
        if seen.insert(key, h.start).is_some() {
            em.emit(
                ctx,
                "duplicate-heading",
                FAM,
                Severity::Advisory,
                h.start,
                h.text.len(),
                "duplicate heading text",
            );
        }
    }
}

fn markup_leak(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"(?m)^#{1,6}[^#\s]").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "markup-leak",
            FAM,
            Severity::Advisory,
            off,
            len,
            "heading marker needs a following space",
        );
    });
}

fn thematic_break_before_heading(ctx: &Ctx, em: &mut Emitter) {
    for &line in &ctx.thematic_breaks {
        let mut next = line + 1;
        while next < ctx.line_count() && ctx.line_text(next).trim().is_empty() {
            next += 1;
        }
        if next >= ctx.line_count() {
            continue;
        }
        let next_text = ctx.line_text(next).trim_start();
        if next_text.starts_with('#')
            && next_text
                .chars()
                .nth(1)
                .is_some_and(|c| c == ' ' || c == '#')
        {
            let off = ctx.line_start_offset(line);
            em.emit(
                ctx,
                "thematic-break-before-heading",
                FAM,
                Severity::Advisory,
                off,
                ctx.line_text(line).len(),
                "thematic break before heading reads like scaffold — remove it",
            );
        }
    }
}

fn bullet_overuse(ctx: &Ctx, em: &mut Emitter) {
    let list_items = ctx.list_items.len();
    if list_items < 8 {
        return;
    }
    let non_blank = ctx
        .text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    if non_blank > 0 && list_items * 2 >= non_blank {
        let off = ctx.list_items.first().map(|i| i.line_start).unwrap_or(0);
        em.emit(
            ctx,
            "bullet-overuse",
            FAM,
            Severity::Advisory,
            off,
            ctx.line_text(ctx.list_items[0].line).len(),
            "too much of the document is bullets",
        );
    }
}

fn double_space(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"([^\s.!?:;])(  )(\S)").unwrap());
    helpers::scan(ctx, re, |off, _len, _| {
        let space_off = off + 1;
        if !ctx.is_table_line(space_off) {
            em.emit(
                ctx,
                "double-space",
                FAM,
                Severity::Advisory,
                space_off,
                2,
                "double space between words",
            );
        }
    });
}

pub const REDUNDANT_ACRONYMS: &[&str] = &[
    "ATM machine",
    "PIN number",
    "LCD display",
    "HIV virus",
    "RAM memory",
    "PDF format",
    "ISBN number",
    "GPS system",
    "CPU unit",
    "UPC code",
    "NIC card",
    "please RSVP",
    "HTTP protocol",
    "IP protocol",
    "SIN number",
    "VIN number",
];

fn redundant_acronym(ctx: &Ctx, em: &mut Emitter) {
    let re = ctx.lists.phrase_regex("redundant-acronym");
    helpers::scan_fancy(ctx, &re, |off, len, _| {
        em.emit(
            ctx,
            "redundant-acronym",
            FAM,
            Severity::Warn,
            off,
            len,
            "redundant acronym phrase",
        );
    });
}

fn indefinite_article(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::RegexBuilder::new(r"\b(a|an)\s+([A-Za-z][A-Za-z-]*)\b")
            .case_insensitive(true)
            .build()
            .unwrap()
    });
    helpers::scan(ctx, re, |off, len, m| {
        if !ctx.masked_identical(off, len) {
            return;
        }
        let before = ctx.masked.as_bytes().get(off.wrapping_sub(1)).copied();
        let after = ctx.masked.as_bytes().get(off + len).copied();
        if matches!(before, Some(b'&' | b'.')) || matches!(after, Some(b'.')) {
            return;
        }
        let Some(caps) = re.captures(m) else { return };
        let article = caps.get(1).unwrap().as_str().to_ascii_lowercase();
        let word = caps.get(2).unwrap().as_str().to_ascii_lowercase();
        let wants_an = wants_an(&word);
        let expected = if wants_an { "an" } else { "a" };
        if article != expected {
            em.emit(
                ctx,
                "indefinite-article",
                FAM,
                Severity::Advisory,
                off,
                caps.get(1).unwrap().len(),
                format!("use '{expected}' before '{word}'"),
            );
        }
    });
}

fn wants_an(word: &str) -> bool {
    if [
        "university",
        "unicorn",
        "unique",
        "unit",
        "user",
        "used",
        "useful",
        "european",
        "one",
        "once",
        "ubiquitous",
        "url",
        "ui",
        "utility",
        "eulogy",
    ]
    .contains(&word)
    {
        return false;
    }
    if ["hour", "honest", "honor", "heir", "honour"].contains(&word) {
        return true;
    }
    matches!(
        word.as_bytes().first(),
        Some(b'a' | b'e' | b'i' | b'o' | b'u')
    )
}

fn placeholder_citation(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::RegexBuilder::new(
            r"\[citation needed\]|\(Author,\s*Year\)|\(\d{4}\)|\[REF\]|\[TODO\]|\[TK\]|\[\?\?\]",
        )
        .case_insensitive(true)
        .build()
        .unwrap()
    });
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "placeholder-citation",
            FAM,
            Severity::Warn,
            off,
            len,
            "placeholder citation",
        );
    });
}

fn tracking_param_in_citation(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE
        .get_or_init(|| regex::Regex::new(r"(?i)([?&](utm_[A-Za-z0-9_]+|fbclid|gclid)=)").unwrap());
    for l in &ctx.links {
        if let Some(m) = re.find(&l.target) {
            let start = l.start + l.length.saturating_sub(l.target.len() + 1) + m.start();
            em.emit(
                ctx,
                "tracking-param-in-citation",
                FAM,
                Severity::Warn,
                start,
                m.len(),
                "tracking parameter in citation URL — remove it",
            );
        }
    }
}

fn unused_named_ref(ctx: &Ctx, em: &mut Emitter) {
    for r in &ctx.ref_defs {
        if !ctx.ref_uses.contains(&r.id) {
            em.emit(
                ctx,
                "unused-named-ref",
                FAM,
                Severity::Advisory,
                r.start,
                r.id.len() + 2,
                "unused named reference definition",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::detector::runner::{detect_text, test_settings};

    fn ids(text: &str) -> Vec<String> {
        detect_text("test.md", text, &test_settings("microsoft"))
            .findings
            .into_iter()
            .map(|f| f.rule_id)
            .collect()
    }

    #[test]
    fn markdown_hygiene_rules_fire() {
        let text = "#Heading\n\n---\n\n## Next\n\n- one\n- two\n- three\n- four\n- five\n- six\n- seven\n- eight\n";
        let got = ids(text);
        for id in [
            "markup-leak",
            "thematic-break-before-heading",
            "bullet-overuse",
        ] {
            assert!(got.iter().any(|g| g == id), "{id}: {got:?}");
        }
    }

    #[test]
    fn inline_style_hygiene_rules_fire() {
        let text = "Use a hour and an user with a  double space. The PDF format cites a source.\n\n[unused]: https://example.com";
        let got = ids(text);
        for id in [
            "indefinite-article",
            "double-space",
            "redundant-acronym",
            "unused-named-ref",
        ] {
            assert!(got.iter().any(|g| g == id), "{id}: {got:?}");
        }
    }

    #[test]
    fn citation_placeholders_fire() {
        let got = ids("This needs proof [citation needed], [REF], [TK], and (Author, Year).");
        assert!(got.iter().any(|g| g == "placeholder-citation"), "{got:?}");
    }

    #[test]
    fn good_article_and_identifiers_stay_quiet() {
        let text = "Use an hour and a user.\n\n[used]: https://example.com\n\nSee [used].";
        let got = ids(text);
        assert!(!got.iter().any(|g| g == "indefinite-article"), "{got:?}");
        assert!(!got.iter().any(|g| g == "unused-named-ref"), "{got:?}");
    }
}

// ---- SPEC §11.3 rules added to complete the family ----

const CAP_STOPWORDS: &[&str] = &[
    "the", "a", "an", "this", "that", "these", "those", "it", "he", "she", "they", "we", "you",
    "i", "if", "when", "while", "for", "and", "but", "or", "not", "as", "at", "by", "in", "on",
    "to", "of", "is", "are", "was", "were", "be", "note", "tip", "see", "use", "run", "add", "get",
    "set", "so", "such", "each", "any", "all",
];

fn inconsistent_capitalization(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\b[A-Z][a-z]+(?: [A-Z][a-z]+)+\b").unwrap());
    let heading_lines: HashSet<usize> = ctx.headings.iter().map(|h| h.line).collect();
    let mut flagged: HashSet<String> = HashSet::new();
    let lower_all = ctx.masked.to_lowercase();
    let mut findings: Vec<(usize, usize, String)> = Vec::new();
    helpers::scan(ctx, re, |off, _len, m| {
        let line = ctx.line_of(off);
        if heading_lines.contains(&line) || ctx.is_table_line(off) {
            return;
        }
        // Shed leading sentence-initial stopwords.
        let mut words: Vec<&str> = m.split(' ').collect();
        let mut shed = 0usize;
        while let Some(first) = words.first() {
            if CAP_STOPWORDS.contains(&first.to_lowercase().as_str()) {
                shed += first.len() + 1;
                words.remove(0);
            } else {
                break;
            }
        }
        if words.len() < 2 {
            return;
        }
        let phrase = words.join(" ");
        if flagged.contains(&phrase.to_lowercase()) {
            return;
        }
        // Also appears fully lowercase elsewhere?
        let lower = phrase.to_lowercase();
        let mut found_lower = false;
        let mut from = 0usize;
        while let Some(i) = lower_all[from..].find(&lower) {
            let at = from + i;
            let boundary_ok = (at == 0 || !lower_all.as_bytes()[at - 1].is_ascii_alphanumeric())
                && (at + lower.len() == lower_all.len()
                    || !lower_all.as_bytes()[at + lower.len()].is_ascii_alphanumeric());
            if boundary_ok && ctx.masked[at..at + lower.len()] == lower {
                found_lower = true;
                break;
            }
            from = at + 1;
        }
        if found_lower {
            flagged.insert(lower);
            findings.push((
                off + shed,
                phrase.len(),
                format!("'{phrase}' is also written lowercase elsewhere — pick one form"),
            ));
        }
    });
    for (off, len, msg) in findings {
        em.emit(
            ctx,
            "inconsistent-capitalization",
            FAM,
            Severity::Advisory,
            off,
            len,
            msg,
        );
    }
}

fn malformed_doi_isbn(ctx: &Ctx, em: &mut Emitter) {
    static DOI: OnceLock<regex::Regex> = OnceLock::new();
    static ISBN: OnceLock<regex::Regex> = OnceLock::new();
    let doi = DOI.get_or_init(|| regex::Regex::new(r"(?i)\bdoi:\s*(\S+)").unwrap());
    let good_doi = regex::Regex::new(r"^10\.\d{4,}/\S+$").unwrap();
    helpers::scan(ctx, doi, |off, len, m| {
        let value = m.split_once(':').map(|x| x.1).unwrap_or("").trim();
        if !good_doi.is_match(value.trim_end_matches(['.', ',', ')'])) {
            em.emit(
                ctx,
                "malformed-doi-isbn",
                FAM,
                Severity::Advisory,
                off,
                len,
                "DOI does not match '10.NNNN/suffix'",
            );
        }
    });
    let isbn = ISBN.get_or_init(|| {
        regex::Regex::new(r"(?i)\bISBN[:\s-]*([0-9Xx][0-9Xx \-]{2,20}[0-9Xx])").unwrap()
    });
    helpers::scan(ctx, isbn, |off, len, m| {
        let digits: usize = m
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == 'X' || *c == 'x')
            .count();
        if digits != 10 && digits != 13 {
            em.emit(
                ctx,
                "malformed-doi-isbn",
                FAM,
                Severity::Advisory,
                off,
                len,
                format!("ISBN has {digits} digits — must be 10 or 13"),
            );
        }
    });
}

#[cfg(test)]
mod added_tests {
    use crate::detector::runner::{detect_text, test_settings};

    fn ids(text: &str) -> Vec<String> {
        detect_text("t.md", text, &test_settings("microsoft"))
            .findings
            .into_iter()
            .map(|f| f.rule_id)
            .collect()
    }

    #[test]
    fn inconsistent_capitalization_fires_on_mixed_forms() {
        let bad = "Use the Access Token here. Later, refresh the access token again.\n";
        assert!(ids(bad).contains(&"inconsistent-capitalization".to_string()));
        let good = "Use the Access Token here. Later, refresh the Access Token again.\n";
        assert!(!ids(good).contains(&"inconsistent-capitalization".to_string()));
    }

    #[test]
    fn malformed_doi_and_isbn() {
        assert!(ids("See doi:banana for details.\n").contains(&"malformed-doi-isbn".to_string()));
        assert!(!ids("See doi:10.1234/abc.5 for details.\n")
            .contains(&"malformed-doi-isbn".to_string()));
        assert!(ids("Cite ISBN 12345 in the notes.\n").contains(&"malformed-doi-isbn".to_string()));
        assert!(!ids("Cite ISBN 978-3-16-148410-0 in the notes.\n")
            .contains(&"malformed-doi-isbn".to_string()));
    }
}
