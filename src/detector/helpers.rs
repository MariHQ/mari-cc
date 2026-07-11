//! Matching helpers per SPEC §11.0.5.

use super::ctx::Ctx;

/// A regex that can never match — used when a (possibly config-emptied) list
/// resolves to zero entries, so an alternation is never built from `""`.
pub fn never_match() -> regex::Regex {
    regex::Regex::new(r"[^\s\S]").unwrap()
}

fn never_match_fancy() -> fancy_regex::Regex {
    fancy_regex::Regex::new(r"[^\s\S]").unwrap()
}

/// `\b(w1|w2|…)\b`, case-insensitive, entries escaped. Accepts `&str` or
/// `String` entries so both built-in `&[&str]` defaults and config-resolved
/// `Vec<String>` overrides compile through the same path.
pub fn word_list<S: AsRef<str>>(words: &[S]) -> regex::Regex {
    if words.is_empty() {
        return never_match();
    }
    let alts: Vec<String> = words.iter().map(|w| regex::escape(w.as_ref())).collect();
    regex::RegexBuilder::new(&format!(r"\b({})\b", alts.join("|")))
        .case_insensitive(true)
        .build()
        .unwrap_or_else(|_| never_match())
}

/// Longest-first alternation guarded by lookarounds instead of `\b`
/// (keys may end in punctuation where a trailing `\b` never matches).
pub fn phrase_list<S: AsRef<str>>(phrases: &[S]) -> fancy_regex::Regex {
    if phrases.is_empty() {
        return never_match_fancy();
    }
    let mut sorted: Vec<&str> = phrases.iter().map(|p| p.as_ref()).collect();
    sorted.sort_by_key(|p| std::cmp::Reverse(p.len()));
    let alts: Vec<String> = sorted.iter().map(|p| regex::escape(p)).collect();
    fancy_regex::RegexBuilder::new(&format!(
        r"(?<![A-Za-z0-9_])({})(?![A-Za-z0-9_])",
        alts.join("|")
    ))
    .case_insensitive(true)
    .build()
    .unwrap_or_else(|_| never_match_fancy())
}

/// Iterate all matches over `ctx.masked`, advancing one char on zero-width.
pub fn scan<F: FnMut(usize, usize, &str)>(ctx: &Ctx, re: &regex::Regex, mut cb: F) {
    let mut pos = 0usize;
    while pos <= ctx.masked.len() {
        match re.find_at(&ctx.masked, pos) {
            Some(m) => {
                cb(m.start(), m.len(), m.as_str());
                pos = if m.end() > m.start() {
                    m.end()
                } else {
                    next_char(&ctx.masked, m.end())
                };
            }
            None => break,
        }
    }
}

/// Same, for fancy-regex (lookaround rules).
pub fn scan_fancy<F: FnMut(usize, usize, &str)>(ctx: &Ctx, re: &fancy_regex::Regex, mut cb: F) {
    let mut pos = 0usize;
    while pos <= ctx.masked.len() {
        match re.find_from_pos(&ctx.masked, pos).ok().flatten() {
            Some(m) => {
                cb(m.start(), m.end() - m.start(), m.as_str());
                pos = if m.end() > m.start() {
                    m.end()
                } else {
                    next_char(&ctx.masked, m.end())
                };
            }
            None => break,
        }
    }
}

fn next_char(s: &str, i: usize) -> usize {
    let mut j = i + 1;
    while j < s.len() && !s.is_char_boundary(j) {
        j += 1;
    }
    j.min(s.len() + 1)
}

/// A phrase→replacement map rule entry.
pub struct MapEntry {
    pub from: &'static str,
    pub to: &'static str,
}

pub const fn me(from: &'static str, to: &'static str) -> MapEntry {
    MapEntry { from, to }
}

/// Build one lookaround-guarded regex over a whole map, longest-first, and
/// return (regex, lookup fn input->replacement) via linear scan on match.
pub fn map_regex(map: &[MapEntry]) -> fancy_regex::Regex {
    let phrases: Vec<&str> = map.iter().map(|m| m.from).collect();
    phrase_list(&phrases)
}

pub fn map_lookup<'a>(map: &'a [MapEntry], matched: &str) -> Option<&'a str> {
    let lower = matched.to_lowercase();
    map.iter()
        .find(|m| m.from.to_lowercase() == lower)
        .map(|m| m.to)
}
