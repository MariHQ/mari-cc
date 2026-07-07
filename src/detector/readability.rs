//! Readability internals per SPEC §11.12.

const EXCEPTIONS: &[(&str, usize)] = &[
    ("every", 2),
    ("business", 2),
    ("different", 3),
    ("comfortable", 3),
    ("vegetable", 3),
    ("february", 4),
    ("area", 3),
    ("idea", 3),
    ("science", 2),
    ("being", 2),
    ("create", 2),
    ("people", 2),
    ("simile", 3),
    ("queue", 1),
    ("the", 1),
    ("average", 3),
    ("naive", 2),
    ("real", 1),
    ("cereal", 3),
];

pub fn syllables(word: &str) -> usize {
    let w: String = word
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .collect();
    if w.is_empty() {
        return 0;
    }
    if let Some((_, n)) = EXCEPTIONS.iter().find(|(e, _)| *e == w) {
        return *n;
    }
    let mut s = w.clone();
    // Strip silent endings: -es after non-l vowel-consonant, -ed, silent -e.
    if s.ends_with("es") && s.len() > 3 {
        let bytes = s.as_bytes();
        let before = bytes[s.len() - 3];
        let before2 = bytes.get(s.len().wrapping_sub(4)).copied();
        let is_vowel = |b: u8| matches!(b, b'a' | b'e' | b'i' | b'o' | b'u' | b'y');
        if !is_vowel(before) && before != b'l' && before2.map(is_vowel).unwrap_or(false) {
            s.truncate(s.len() - 2);
        }
    } else if s.ends_with("ed") && s.len() > 3 {
        s.truncate(s.len() - 2);
    } else if s.ends_with('e') && !s.ends_with("le") && s.len() > 2 {
        s.truncate(s.len() - 1);
    }
    let s = s.strip_prefix('y').unwrap_or(&s).to_string();
    // Count vowel groups [aeiouy]{1,2}.
    let mut count = 0usize;
    let mut in_group = 0usize;
    for c in s.chars() {
        if matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y') {
            in_group += 1;
            if in_group == 1 {
                count += 1;
            } else if in_group > 2 {
                count += 1;
                in_group = 1;
            }
        } else {
            in_group = 0;
        }
    }
    // +1 consonant+le endings; +1 hiatus.
    if w.len() > 2 && w.ends_with("le") {
        let b = w.as_bytes()[w.len() - 3];
        if !matches!(b, b'a' | b'e' | b'i' | b'o' | b'u' | b'y') {
            count += 1;
        }
    }
    for h in ["ia", "io", "ua", "eo"] {
        if w.contains(h) {
            count += 1;
            break;
        }
    }
    count.max(1)
}

pub struct GradeStats {
    pub words: usize,
    pub sentences: usize,
    pub syllables: usize,
    pub letters: usize,
    pub grade: f64,
}

/// FKGL/CLI mean over already-segmented prose.
pub fn grade(words: &[&str], sentence_count: usize) -> GradeStats {
    let w = words.len().max(1) as f64;
    let s = sentence_count.max(1) as f64;
    let syl: usize = words.iter().map(|t| syllables(t)).sum();
    let letters: usize = words
        .iter()
        .map(|t| t.chars().filter(|c| c.is_ascii_alphanumeric()).count())
        .sum();
    let fkgl = 0.39 * (w / s) + 11.8 * (syl as f64 / w) - 15.59;
    let cli = 0.0588 * (letters as f64 / w * 100.0) - 0.296 * (s / w * 100.0) - 15.8;
    GradeStats {
        words: words.len(),
        sentences: sentence_count,
        syllables: syl,
        letters,
        grade: (fkgl + cli) / 2.0,
    }
}
