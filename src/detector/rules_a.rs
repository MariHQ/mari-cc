//! Family A — AI-slop tells (SPEC §11.1).

use super::ctx::Ctx;
use super::helpers;
use super::{Emitter, Family, Rule, Severity};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

const FAM: Family = Family::AiSlop;

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
        r("overused-word", overused_word),
        r("marketing-buzzword", marketing_buzzword),
        r("cliche-opener", cliche_opener),
        r("filler-phrase", filler_phrase),
        r("manufactured-contrast", manufactured_contrast),
        r("conclusion-restate", conclusion_restate),
        r("vague-attribution", vague_attribution),
        r("despite-challenges-closer", despite_challenges_closer),
        r("significance-boilerplate", significance_boilerplate),
        r("em-dash-overuse", em_dash_overuse),
        r("bold-lead-in-list", bold_lead_in_list),
        r("assistant-meta", assistant_meta),
        r("sycophancy", sycophancy),
        r("hedge-overuse", hedge_overuse),
        r("conversational-scaffolding", conversational_scaffolding),
        r("superficial-ing-participle", superficial_ing_participle),
        r("transition-scaffolding", transition_scaffolding),
        r("excessive-bold", excessive_bold),
        r("listicle-reflex", listicle_reflex),
        r("emphasis-as-heading", emphasis_as_heading),
        r("hype-intensifier", hype_intensifier),
        r("semicolon-overuse", semicolon_overuse),
        r("emoji-decoration", emoji_decoration),
        r("smart-quotes", smart_quotes),
        r("unicode-artifact", unicode_artifact),
        r("negative-parallelism", negative_parallelism),
        r("tricolon-overuse", tricolon_overuse),
        r("serves-as-copula", serves_as_copula),
        r("media-coverage-boilerplate", media_coverage_boilerplate),
        r("future-outlook-speculation", future_outlook_speculation),
        r("interrogative-answer", interrogative_answer),
        r("uniform-cadence", uniform_cadence),
    ]
}

pub const OVERUSED: &[(&str, &str, f64)] = &[
    ("delve", "delve", 28.0),
    ("delves", "delve", 28.0),
    ("delving", "delve", 28.0),
    ("delved", "delve", 28.0),
    ("meticulous", "meticulous", 34.7),
    ("meticulously", "meticulous", 34.7),
    ("intricate", "intricate", 11.2),
    ("intricately", "intricate", 11.2),
    ("commendable", "commendable", 9.8),
    ("commendably", "commendable", 9.8),
    ("underscore", "underscore", 13.8),
    ("underscores", "underscore", 13.8),
    ("underscoring", "underscore", 13.8),
    ("underscored", "underscore", 13.8),
    ("showcase", "showcase", 10.7),
    ("showcases", "showcase", 10.7),
    ("showcasing", "showcase", 10.7),
    ("showcased", "showcase", 10.7),
    ("realm", "realm", 4.0),
    ("pivotal", "pivotal", 4.0),
    ("garner", "garner", 4.0),
    ("garners", "garner", 4.0),
    ("garnered", "garner", 4.0),
    ("boasts", "boast", 4.0),
    ("boast", "boast", 4.0),
    ("adept", "adept", 4.0),
    ("groundbreaking", "groundbreaking", 4.0),
    ("tapestry", "tapestry", 1.5),
    ("testament", "testament", 1.5),
    ("leverage", "leverage", 1.5),
    ("leveraging", "leverage", 1.5),
    ("robust", "robust", 1.5),
    ("seamless", "seamless", 1.5),
    ("seamlessly", "seamless", 1.5),
    ("nuanced", "nuanced", 1.5),
    ("multifaceted", "multifaceted", 1.5),
    ("potential", "potential", 1.2),
    ("elevate", "elevate", 1.2),
    ("elevates", "elevate", 1.2),
    ("elevating", "elevate", 1.2),
];

fn overused_word(ctx: &Ctx, em: &mut Emitter) {
    let entries = ctx.lists.weighted("overused-word");
    let words: Vec<&str> = entries.iter().map(|(w, _, _)| w.as_str()).collect();
    let re = helpers::word_list(&words);
    let mut hits = Vec::new();
    let mut distinct = HashSet::new();
    let weight_by: HashMap<&str, (&str, f64)> = entries
        .iter()
        .map(|(w, b, wt)| (w.as_str(), (b.as_str(), *wt)))
        .collect();
    let mut score = 0.0;
    helpers::scan(ctx, &re, |off, len, m| {
        let key = m.to_lowercase();
        if let Some((base, wt)) = weight_by.get(key.as_str()) {
            hits.push((off, len, (*base).to_string()));
            distinct.insert((*base).to_string());
            score += *wt;
        }
    });
    let words = ctx.word_count.max(1) as f64;
    let density = hits.len() as f64 / words * 1000.0;
    let weighted = score / words * 1000.0;
    if distinct.len() >= 2 || (hits.len() >= 2 && density >= 4.0) {
        let sev = if distinct.len() >= 3 || weighted >= 20.0 {
            Severity::Warn
        } else {
            Severity::Advisory
        };
        for (off, len, _) in hits {
            em.emit(
                ctx,
                "overused-word",
                FAM,
                sev,
                off,
                len,
                format!(
                    "AI-overused word cluster — {:.1}/1k, weighted {:.1}",
                    density, weighted
                ),
            );
        }
    }
}

pub const MARKETING: &[&str] = &[
    "streamline",
    "streamlines",
    "streamlining",
    "empower",
    "empowers",
    "empowering",
    "supercharge",
    "supercharges",
    "world-class",
    "enterprise-grade",
    "cutting-edge",
    "game-changing",
    "game changer",
    "game-changer",
    "next-generation",
    "next-gen",
    "best-in-class",
    "turnkey",
    "mission-critical",
    "synergy",
    "synergies",
    "holistic",
    "paradigm shift",
    "frictionless",
    "bleeding-edge",
    "unparalleled",
    "unrivaled",
    "state-of-the-art",
    "unlock the full potential",
    "unlocks the full potential",
    "unlock the power",
    "harness the power",
    "harnessing the power",
];

fn marketing_buzzword(ctx: &Ctx, em: &mut Emitter) {
    let re = ctx.lists.phrase_regex("marketing-buzzword");
    helpers::scan_fancy(ctx, &re, |off, len, _| {
        em.emit(
            ctx,
            "marketing-buzzword",
            FAM,
            Severity::Warn,
            off,
            len,
            "marketing buzzword — replace with a concrete claim",
        );
    });
}

pub const CLICHE_OPENER: &[&str] = &[
    "in today's fast-paced world",
    "in today's fast-paced age",
    "in today's modern world",
    "in today's modern age",
    "in today's digital world",
    "in today's digital age",
    "in today’s fast-paced world",
    "in today’s fast-paced age",
    "in today’s modern world",
    "in today’s modern age",
    "in today’s digital world",
    "in today’s digital age",
    "in the ever-evolving landscape of",
    "in the ever-evolving world of",
    "in the ever-changing landscape of",
    "in the ever-changing world of",
    "in the rapidly changing landscape of",
    "in the rapidly changing world of",
    "in the realm of",
    "in the digital age",
    "in an era of",
    "in an age of",
    "when it comes to",
    "at its core",
    "in the world of",
];

fn cliche_opener(ctx: &Ctx, em: &mut Emitter) {
    let lower = ctx.masked.to_lowercase();
    let phrases = ctx.lists.words("cliche-opener");
    for phrase in phrases.iter() {
        let phrase = phrase.to_lowercase();
        let phrase = phrase.as_str();
        let mut from = 0usize;
        while let Some(pos) = lower[from..].find(phrase) {
            let off = from + pos;
            if ctx.is_sentence_start(off) {
                em.emit(
                    ctx,
                    "cliche-opener",
                    FAM,
                    Severity::Warn,
                    off,
                    phrase.len(),
                    "cliche opener — start with the actual subject",
                );
            }
            from = off + phrase.len();
        }
    }
}

fn filler_phrase(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| fancy_regex::RegexBuilder::new(
        r"(?i)(It'?s important to note that|It is important to note|It'?s worth noting|It is worth noting|worth mentioning that|Needless to say|At the end of the day|That being said|It should be noted that)"
    ).build().unwrap());
    helpers::scan_fancy(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "filler-phrase",
            FAM,
            Severity::Warn,
            off,
            len,
            "filler phrase — delete it or state the point directly",
        );
    });
}

fn manufactured_contrast(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| fancy_regex::RegexBuilder::new(
        r"(?i)\b(not\s+(just|only|merely|simply)[^.!?\n]{1,160}\b(it'?s|but|rather|they'?re|we'?re)\b|not only[^.!?\n]{1,160}\bbut( also)?\b)"
    ).build().unwrap());
    helpers::scan_fancy(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "manufactured-contrast",
            FAM,
            Severity::Warn,
            off,
            len,
            "manufactured contrast — make the distinction only if it carries meaning",
        );
    });
}

pub const CONCLUSION_MARKERS: &[&str] = &[
    "In conclusion",
    "In summary",
    "To sum up",
    "In essence",
    "Overall",
    "Ultimately",
    "All in all",
];

/// `(?m)^[ \t>]*(w1|w2|…)\b`, case-insensitive, built from a resolved word list;
/// a never-match regex when the list is empty.
fn line_start_alternation(words: &[String]) -> regex::Regex {
    if words.is_empty() {
        return helpers::never_match();
    }
    let alts: Vec<String> = words.iter().map(|w| regex::escape(w)).collect();
    regex::RegexBuilder::new(&format!(r"(?m)^[ \t>]*({})\b", alts.join("|")))
        .case_insensitive(true)
        .build()
        .unwrap_or_else(|_| helpers::never_match())
}

fn conclusion_restate(ctx: &Ctx, em: &mut Emitter) {
    let words = ctx.lists.words("conclusion-restate");
    let re = line_start_alternation(&words);
    helpers::scan(ctx, &re, |off, len, _| {
        em.emit(
            ctx,
            "conclusion-restate",
            FAM,
            Severity::Warn,
            off,
            len,
            "conclusion marker — avoid restating the obvious takeaway",
        );
    });
}

fn vague_attribution(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| fancy_regex::RegexBuilder::new(
        r"(?i)(studies show|research suggests|research shows|experts (say|argue|believe)|many believe|it is widely (regarded|believed|known)|industry reports|some say|critics argue)"
    ).build().unwrap());
    helpers::scan_fancy(ctx, re, |off, len, _| {
        let following = &ctx.masked[off + len..(off + len + 200).min(ctx.masked.len())];
        let cited = following.contains("](")
            || following.contains("http://")
            || following.contains("https://")
            || regex::Regex::new(r"\[[0-9]+\]|\^[0-9]+")
                .unwrap()
                .is_match(following);
        if !cited {
            em.emit(
                ctx,
                "vague-attribution",
                FAM,
                Severity::Warn,
                off,
                len,
                "vague attribution — name the source or remove the claim",
            );
        }
    });
}

fn despite_challenges_closer(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| fancy_regex::RegexBuilder::new(
        r"(?i)despite (its|these|the|ongoing|numerous)[^.!?\n]{0,120}(challenges|difficulties|obstacles|setbacks)[^.!?\n]{0,120}(continues to|remains|still) (thrive|evolve|grow|serve|play|stand|endure)"
    ).build().unwrap());
    helpers::scan_fancy(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "despite-challenges-closer",
            FAM,
            Severity::Warn,
            off,
            len,
            "stock resilience closer — state the actual outcome",
        );
    });
}

pub const SIGNIFICANCE_BOILERPLATE: &[&str] = &[
    "stands as a testament",
    "marking a pivotal moment",
    "leaving an indelible mark",
    "enduring legacy",
    "key turning point",
    "plays a vital role",
    "plays a crucial role",
    "plays a pivotal role",
    "plays a key role",
    "plays a significant role",
    "rich history",
    "rich tapestry",
    "rich tradition",
    "navigating the complexities of",
    "navigate the complexities of",
    "navigating the complex landscape of",
];

fn significance_boilerplate(ctx: &Ctx, em: &mut Emitter) {
    let re = ctx.lists.phrase_regex("significance-boilerplate");
    helpers::scan_fancy(ctx, &re, |off, len, _| {
        em.emit(
            ctx,
            "significance-boilerplate",
            FAM,
            Severity::Warn,
            off,
            len,
            "significance boilerplate — replace with a specific effect",
        );
    });
}

fn em_dash_overuse(ctx: &Ctx, em: &mut Emitter) {
    let mut first = None;
    let mut count = 0usize;
    for (i, ch) in ctx.masked.char_indices() {
        if ch == '—' {
            first.get_or_insert(i);
            count += 1;
        }
    }
    let rate = count as f64 / ctx.word_count.max(1) as f64 * 1000.0;
    if count >= 3 && rate > 4.0 {
        if let Some(off) = first {
            em.emit(
                ctx,
                "em-dash-overuse",
                FAM,
                Severity::Warn,
                off,
                "—".len(),
                format!("em dash overuse — {count} dashes, {rate:.1}/1k words"),
            );
        }
    }
}

fn bold_lead_in_list(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| regex::Regex::new(r"^\s*(\*\*[^*]+\*\*|__[^_]+__)\s*[:—-]").unwrap());
    let mut run_start: Option<usize> = None;
    let mut run_len = 0usize;
    let mut prev_line = None;
    for item in &ctx.list_items {
        let shaped = re.is_match(&item.text);
        let consecutive = prev_line.map(|l| item.line == l + 1).unwrap_or(true);
        if shaped && consecutive {
            run_start.get_or_insert(item.start);
            run_len += 1;
        } else {
            if run_len >= 3 {
                em.emit(
                    ctx,
                    "bold-lead-in-list",
                    FAM,
                    Severity::Warn,
                    run_start.unwrap(),
                    2,
                    "AI listicle template — use real headings or plain bullets",
                );
            }
            run_start = if shaped { Some(item.start) } else { None };
            run_len = usize::from(shaped);
        }
        prev_line = Some(item.line);
    }
    if run_len >= 3 {
        em.emit(
            ctx,
            "bold-lead-in-list",
            FAM,
            Severity::Warn,
            run_start.unwrap(),
            2,
            "AI listicle template — use real headings or plain bullets",
        );
    }
}

fn assistant_meta(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| fancy_regex::RegexBuilder::new(
        r"(?i)(As an AI language model|as of my (knowledge cutoff|last (update|training))|I hope this helps|Certainly!|I'd be happy to|Let me know if you|Feel free to (ask|reach)|Here's a breakdown|\[insert[^\]\n]{0,60}\](?![\[(])|\[Your Name\]|\[Your Company\])"
    ).build().unwrap());
    helpers::scan_fancy(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "assistant-meta",
            FAM,
            Severity::Error,
            off,
            len,
            "assistant meta text — remove it",
        );
    });
}

pub const SYCOPHANCY: &[&str] = &[
    "Great question",
    "You're absolutely right",
    "That's a great point",
    "Excellent question",
    "What a fascinating",
];

fn sycophancy(ctx: &Ctx, em: &mut Emitter) {
    let re = ctx.lists.phrase_regex("sycophancy");
    helpers::scan_fancy(ctx, &re, |off, len, _| {
        em.emit(
            ctx,
            "sycophancy",
            FAM,
            Severity::Warn,
            off,
            len,
            "sycophantic opener — answer directly",
        );
    });
}

pub const HEDGES: &[&str] = &[
    "it could be argued",
    "arguably",
    "to some extent",
    "in many ways",
    "in some ways",
    "more often than not",
    "generally speaking",
    "broadly speaking",
    "in a sense",
    "for all intents and purposes",
    "tends to",
    "somewhat",
    "sort of",
    "kind of",
];

fn hedge_overuse(ctx: &Ctx, em: &mut Emitter) {
    let re = ctx.lists.phrase_regex("hedge-overuse");
    let mut hits = Vec::new();
    helpers::scan_fancy(ctx, &re, |off, len, _| hits.push((off, len)));
    let rate = hits.len() as f64 / ctx.word_count.max(1) as f64 * 1000.0;
    if hits.len() >= 2 && (hits.len() >= 3 || rate >= 3.0) {
        let sev = if hits.len() >= 4 {
            Severity::Warn
        } else {
            Severity::Advisory
        };
        for (off, len) in hits {
            em.emit(
                ctx,
                "hedge-overuse",
                FAM,
                sev,
                off,
                len,
                "hedge pileup — commit or qualify once",
            );
        }
    }
}

pub const CONVERSATIONAL_SCAFFOLDING: &[&str] = &[
    "let's delve into",
    "let's break this down",
    "let's dive in",
    "let's explore",
    "let's unpack",
    "deep dive into",
    "take a deep dive",
    "think of it as",
    "think of it like",
    "imagine a world where",
    "to put it simply",
    "here's the kicker",
    "here's the thing",
    "buckle up",
    "spoiler alert",
    "plot twist",
];

fn conversational_scaffolding(ctx: &Ctx, em: &mut Emitter) {
    let re = ctx.lists.phrase_regex("conversational-scaffolding");
    helpers::scan_fancy(ctx, &re, |off, len, _| {
        em.emit(
            ctx,
            "conversational-scaffolding",
            FAM,
            Severity::Advisory,
            off,
            len,
            "conversational scaffold — remove the setup",
        );
    });
}

fn superficial_ing_participle(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::RegexBuilder::new(
        r",\s+(highlighting|underscoring|emphasizing|reflecting|symbolizing|showcasing|fostering|ensuring|contributing to|paving the way)"
    ).case_insensitive(true).build().unwrap());
    let hits: Vec<_> = re.find_iter(&ctx.masked).collect();
    if hits.len() >= 2 {
        for m in hits {
            em.emit(
                ctx,
                "superficial-ing-participle",
                FAM,
                Severity::Advisory,
                m.start(),
                m.len(),
                "vague participial gloss — state the relation directly",
            );
        }
    }
}

pub const TRANSITIONS: &[&str] = &[
    "Additionally",
    "Moreover",
    "Furthermore",
    "However",
    "Consequently",
    "Nevertheless",
];

fn transition_scaffolding(ctx: &Ctx, em: &mut Emitter) {
    let words = ctx.lists.words("transition-scaffolding");
    let re = line_start_alternation(&words);
    let hits: Vec<_> = re.find_iter(&ctx.masked).collect();
    if hits.len() >= 2 {
        for m in hits {
            em.emit(
                ctx,
                "transition-scaffolding",
                FAM,
                Severity::Advisory,
                m.start(),
                m.len(),
                "transition scaffold — use only when the relation is not obvious",
            );
        }
    }
}

fn excessive_bold(ctx: &Ctx, em: &mut Emitter) {
    let n = ctx.bold_spans.len();
    let rate = n as f64 / ctx.word_count.max(1) as f64 * 100.0;
    if n >= 4 && rate >= 3.0 {
        if let Some(b) = ctx.bold_spans.first() {
            em.emit(
                ctx,
                "excessive-bold",
                FAM,
                Severity::Advisory,
                b.start,
                b.length,
                format!("excessive bold emphasis — {n} spans, {rate:.1}/100 words"),
            );
        }
    }
}

fn listicle_reflex(ctx: &Ctx, em: &mut Emitter) {
    let n = ctx.list_items.len();
    if n < 5 {
        return;
    }
    let short = ctx
        .list_items
        .iter()
        .filter(|i| ctx.count_words(&i.text) <= 4)
        .count();
    if short * 2 >= n {
        if let Some(item) = ctx.list_items.first() {
            em.emit(
                ctx,
                "listicle-reflex",
                FAM,
                Severity::Advisory,
                item.line_start,
                1,
                "listicle reflex — expand or group short bullets",
            );
        }
    }
}

fn emphasis_as_heading(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| regex::Regex::new(r"^[ \t]*(\*\*|__)(.{1,48})(\*\*|__)[ \t]*$").unwrap());
    for line in 0..ctx.line_count() {
        if ctx.table_lines.contains(&line) {
            continue;
        }
        let text = ctx.line_text(line);
        if let Some(c) = re.captures(text) {
            let inner = c.get(2).unwrap().as_str();
            if !inner.trim_end().ends_with(['.', ':', '!', '?', ',', ';']) {
                em.emit(
                    ctx,
                    "emphasis-as-heading",
                    FAM,
                    Severity::Advisory,
                    ctx.line_start_offset(line),
                    text.len(),
                    "bold line used as a heading — use a real heading",
                );
            }
        }
    }
}

pub const HYPE_INTENSIFIER: &[&str] = &[
    "greatly",
    "vastly",
    "hugely",
    "immensely",
    "enormously",
    "tremendously",
    "remarkably",
    "crucial",
    "crucially",
    "pivotal",
    "paramount",
    "invaluable",
    "one of the most",
    "a great deal of",
];

fn hype_intensifier(ctx: &Ctx, em: &mut Emitter) {
    let re = ctx.lists.phrase_regex("hype-intensifier");
    helpers::scan_fancy(ctx, &re, |off, len, _| {
        em.emit(
            ctx,
            "hype-intensifier",
            FAM,
            Severity::Advisory,
            off,
            len,
            "hype intensifier — use only if measurable",
        );
    });
}

// ---- SPEC §11.1 rules added to complete the family ----

/// A `;` at `off` terminates an HTML entity (`&name;` `&#nnn;` `&#xhh;`).
fn is_entity_semicolon(masked: &str, off: usize) -> bool {
    let start = off.saturating_sub(12);
    let mut s = start;
    while !masked.is_char_boundary(s) {
        s += 1;
    }
    let tail = &masked[s..off];
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"&([a-zA-Z]{1,10}|#\d{1,7}|#x[0-9a-fA-F]{1,6})$").unwrap()
    });
    re.is_match(tail)
}

fn semicolon_overuse(ctx: &Ctx, em: &mut Emitter) {
    let mut hits: Vec<usize> = Vec::new();
    for (off, c) in ctx.masked.char_indices() {
        if c == ';' && !ctx.is_table_line(off) && !is_entity_semicolon(&ctx.masked, off) {
            hits.push(off);
        }
    }
    if em.zero_tol("semicolon-overuse") {
        for off in &hits {
            em.emit(
                ctx,
                "semicolon-overuse",
                FAM,
                Severity::Warn,
                *off,
                1,
                "semicolon — split the sentence",
            );
        }
        return;
    }
    let rate = hits.len() as f64 / ctx.word_count.max(1) as f64 * 1000.0;
    if hits.len() >= 3 && rate > 5.0 {
        em.emit(
            ctx,
            "semicolon-overuse",
            FAM,
            Severity::Advisory,
            hits[0],
            1,
            format!(
                "{} semicolons ({:.1}/1k words) — AI leans on them",
                hits.len(),
                rate
            ),
        );
    }
}

fn emoji_decoration(ctx: &Ctx, em: &mut Emitter) {
    for line in 0..ctx.line_count() {
        let start = ctx.line_start_offset(line);
        let masked_line: &str = {
            let end = if line + 1 < ctx.line_count() {
                ctx.line_start_offset(line + 1) - 1
            } else {
                ctx.masked.len()
            };
            &ctx.masked[start..end.max(start)]
        };
        let trimmed = masked_line.trim_start();
        let mut rest = trimmed;
        // Optional bullet marker.
        for marker in ["- ", "* ", "+ "] {
            if let Some(r) = rest.strip_prefix(marker) {
                rest = r.trim_start();
                break;
            }
        }
        if let Some(c) = rest.chars().next() {
            let cp = c as u32;
            let is_emoji = (0x2600..=0x27BF).contains(&cp)
                || (0x2B00..=0x2BFF).contains(&cp)
                || cp == 0xFE0F
                || (0x1F000..=0x1FAFF).contains(&cp);
            if is_emoji {
                let off = start + (masked_line.len() - rest.len());
                em.emit(
                    ctx,
                    "emoji-decoration",
                    FAM,
                    Severity::Warn,
                    off,
                    c.len_utf8(),
                    "line-initial emoji decoration — an AI listicle tell",
                );
            }
        }
    }
}

fn smart_quotes(ctx: &Ctx, em: &mut Emitter) {
    let mut hits: Vec<(usize, usize)> = Vec::new();
    for (i, c) in ctx.masked.char_indices() {
        if matches!(c, '\u{2018}' | '\u{2019}' | '\u{201C}' | '\u{201D}') {
            hits.push((i, c.len_utf8()));
        }
    }
    if em.zero_tol("smart-quotes") {
        for (off, len) in &hits {
            em.emit(
                ctx,
                "smart-quotes",
                FAM,
                Severity::Advisory,
                *off,
                *len,
                "smart quote — use straight quotes in source",
            );
        }
        return;
    }
    if hits.len() >= 3 {
        let (off, len) = hits[0];
        em.emit(
            ctx,
            "smart-quotes",
            FAM,
            Severity::Advisory,
            off,
            len,
            format!(
                "{} smart quotes — use straight quotes in source",
                hits.len()
            ),
        );
    }
}

fn unicode_artifact(ctx: &Ctx, em: &mut Emitter) {
    for (i, c) in ctx.masked.char_indices() {
        let cp = c as u32;
        let invisible = matches!(
            cp,
            0x00A0 | 0x202F | 0x2009 | 0x200B | 0x200C | 0x200D | 0xFEFF
        );
        if invisible {
            em.emit(
                ctx,
                "unicode-artifact",
                FAM,
                Severity::Warn,
                i,
                c.len_utf8(),
                format!("invisible character U+{cp:04X} — replace with a plain space or remove"),
            );
        }
    }
}

fn negative_parallelism(ctx: &Ctx, em: &mut Emitter) {
    static RES: OnceLock<Vec<regex::Regex>> = OnceLock::new();
    let res = RES.get_or_init(|| {
        vec![
            regex::Regex::new(r",\s+not\s+.{2,30}[.!?]").unwrap(),
            regex::Regex::new(r"Not \w+\. Not \w+").unwrap(),
            regex::Regex::new(r"\w+ rather than \w+").unwrap(),
            regex::Regex::new(r"(?m)^\s*Rather,\s").unwrap(),
        ]
    });
    let mut hits: Vec<(usize, usize)> = Vec::new();
    for re in res {
        helpers::scan(ctx, re, |off, len, _| hits.push((off, len)));
    }
    if hits.len() >= 2 {
        hits.sort();
        for (off, len) in hits {
            em.emit(
                ctx,
                "negative-parallelism",
                FAM,
                Severity::Advisory,
                off,
                len,
                "negative parallelism — a machine cadence; state it directly",
            );
        }
    }
}

fn tricolon_overuse(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\w+, \w+, and \w+").unwrap());
    let mut hits: Vec<(usize, usize)> = Vec::new();
    helpers::scan(ctx, re, |off, len, _| hits.push((off, len)));
    if hits.len() >= 3 {
        for (off, len) in hits {
            em.emit(
                ctx,
                "tricolon-overuse",
                FAM,
                Severity::Advisory,
                off,
                len,
                "tricolon reflex — vary list shapes",
            );
        }
    }
}

pub const SERVES_AS_COPULA: &[&str] = &[
    "serves as",
    "serve as",
    "stands as",
    "stand as",
    "acts as",
    "functions as",
    "represents a",
    "exemplifies",
    "embodies",
];

fn serves_as_copula(ctx: &Ctx, em: &mut Emitter) {
    let re = ctx.lists.phrase_regex("serves-as-copula");
    let mut hits: Vec<(usize, usize)> = Vec::new();
    helpers::scan_fancy(ctx, &re, |off, len, _| hits.push((off, len)));
    if hits.len() >= 2 {
        for (off, len) in hits {
            em.emit(
                ctx,
                "serves-as-copula",
                FAM,
                Severity::Advisory,
                off,
                len,
                "inflated copula — 'is' often reads cleaner",
            );
        }
    }
}

pub const MEDIA_COVERAGE: &[&str] = &[
    "featured in",
    "profiled in",
    "has been featured",
    "and other prominent outlets",
    "maintains a strong",
    "a strong social media presence",
    "an active digital presence",
    "garnered attention",
];

fn media_coverage_boilerplate(ctx: &Ctx, em: &mut Emitter) {
    let re = ctx.lists.phrase_regex("media-coverage-boilerplate");
    helpers::scan_fancy(ctx, &re, |off, len, _| {
        em.emit(
            ctx,
            "media-coverage-boilerplate",
            FAM,
            Severity::Advisory,
            off,
            len,
            "media-coverage boilerplate — name the outlet and date or cut it",
        );
    });
}

pub const FUTURE_OUTLOOK: &[&str] = &[
    "the future of",
    "evolving landscape",
    "continues to evolve",
    "is poised to",
    "on the horizon",
    "in the years to come",
    "only time will tell",
    "the road ahead",
];

fn future_outlook_speculation(ctx: &Ctx, em: &mut Emitter) {
    let re = ctx.lists.phrase_regex("future-outlook-speculation");
    helpers::scan_fancy(ctx, &re, |off, len, _| {
        em.emit(
            ctx,
            "future-outlook-speculation",
            FAM,
            Severity::Advisory,
            off,
            len,
            "future-outlook speculation — docs outlive it; cut or date it",
        );
    });
}

fn interrogative_answer(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"(^|[.!?]\s)((The|Its|Their|His|Her|Our)\s+\w+)\?\s+[A-Z]\w*\.").unwrap()
    });
    helpers::scan(ctx, re, |off, len, m| {
        // Anchor at the interrogative fragment, not a preceding terminator.
        let lead = m.len()
            - m.trim_start_matches(|c: char| c == '.' || c == '!' || c == '?' || c.is_whitespace())
                .len();
        em.emit(
            ctx,
            "interrogative-answer",
            FAM,
            Severity::Advisory,
            off + lead,
            len - lead,
            "rhetorical question-answer cadence — state it directly",
        );
    });
}

fn uniform_cadence(ctx: &Ctx, em: &mut Emitter) {
    // Whole-document aggregate; zero tolerance is a no-op (SPEC §11.0).
    let counts: Vec<f64> = ctx
        .sentences
        .iter()
        .map(|s| s.words as f64)
        .filter(|&w| w > 0.0)
        .collect();
    if counts.len() < 6 {
        return;
    }
    let mean = counts.iter().sum::<f64>() / counts.len() as f64;
    if mean < 4.0 {
        return;
    }
    let var = counts.iter().map(|w| (w - mean).powi(2)).sum::<f64>() / counts.len() as f64;
    let cv = var.sqrt() / mean;
    if cv < 0.25 {
        let off = ctx.sentences[0].start;
        em.emit(
            ctx,
            "uniform-cadence",
            FAM,
            Severity::Advisory,
            off,
            0,
            format!(
                "uniform sentence cadence (CV {:.2} < 0.25 over {} sentences) — vary rhythm",
                cv,
                counts.len()
            ),
        );
    }
}

#[cfg(test)]
mod added_tests {
    use crate::detector::runner::{detect_text, test_settings};

    fn ids(text: &str) -> Vec<String> {
        let s = test_settings("microsoft");
        detect_text("t.md", text, &s)
            .findings
            .into_iter()
            .map(|f| f.rule_id)
            .collect()
    }

    #[test]
    fn semicolon_overuse_gate() {
        let bad = "One; two; three; short words here only.\n";
        assert!(ids(bad).contains(&"semicolon-overuse".to_string()));
        let good = "One clause; another separate paragraph with plenty of additional prose words to lower the density of that single semicolon well beneath the configured rate gate for this rule to fire at all.\n";
        assert!(!ids(good).contains(&"semicolon-overuse".to_string()));
    }

    #[test]
    fn emoji_decoration_fires_on_bullet_emoji() {
        assert!(ids("- 🚀 Launch fast\n").contains(&"emoji-decoration".to_string()));
        assert!(!ids("- Launch fast\n").contains(&"emoji-decoration".to_string()));
    }

    #[test]
    fn smart_quotes_needs_three() {
        assert!(
            ids("\u{201C}a\u{201D} and \u{2018}b\u{2019} quoted twice.\n")
                .contains(&"smart-quotes".to_string())
        );
        assert!(!ids("\u{201C}one\u{201D} pair only.\n")
            .iter()
            .any(|i| i == "smart-quotes"));
    }

    #[test]
    fn unicode_artifact_per_char() {
        let found = ids("word\u{00A0}joined and\u{200B}split.\n");
        assert_eq!(found.iter().filter(|i| *i == "unicode-artifact").count(), 2);
        assert!(!ids("plain words.\n").contains(&"unicode-artifact".to_string()));
    }

    #[test]
    fn negative_parallelism_needs_two() {
        let bad = "It works, not magic. We chose speed rather than size.\n";
        assert!(ids(bad).contains(&"negative-parallelism".to_string()));
        assert!(!ids("We chose speed rather than size.\n")
            .contains(&"negative-parallelism".to_string()));
    }

    #[test]
    fn tricolon_needs_three() {
        let bad = "Fast, simple, and safe. Small, quick, and clean. Bold, clear, and firm.\n";
        assert!(ids(bad).contains(&"tricolon-overuse".to_string()));
        assert!(!ids("Fast, simple, and safe.\n").contains(&"tricolon-overuse".to_string()));
    }

    #[test]
    fn serves_as_copula_needs_two() {
        let bad = "It serves as a cache. It acts as a proxy.\n";
        assert!(ids(bad).contains(&"serves-as-copula".to_string()));
        assert!(!ids("It serves as a cache.\n").contains(&"serves-as-copula".to_string()));
    }

    #[test]
    fn media_and_future_boilerplate_per_hit() {
        assert!(ids("The project garnered attention last year.\n")
            .contains(&"media-coverage-boilerplate".to_string()));
        assert!(ids("This is poised to change everything.\n")
            .contains(&"future-outlook-speculation".to_string()));
        assert!(!ids("The project shipped last year.\n")
            .iter()
            .any(|i| i == "media-coverage-boilerplate" || i == "future-outlook-speculation"));
    }

    #[test]
    fn interrogative_answer_cadence() {
        assert!(ids("The answer? Simple.\n").contains(&"interrogative-answer".to_string()));
        assert!(!ids("The answer is simple.\n").contains(&"interrogative-answer".to_string()));
    }

    #[test]
    fn uniform_cadence_flags_flat_prose() {
        let flat = "We built the tool here. We shipped the tool fast. We fixed the bug set. We wrote the docs page. We ran the test suite. We sent the note out.\n";
        assert!(ids(flat).contains(&"uniform-cadence".to_string()));
        let varied = "We built it. After months of iteration and painful rewrites across three teams, we shipped. Tests pass. The documentation, which nobody wanted to write, finally exists and covers every command. Done.\n";
        assert!(!ids(varied).contains(&"uniform-cadence".to_string()));
    }
}
