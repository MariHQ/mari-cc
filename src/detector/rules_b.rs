//! Family B — clarity and concision (SPEC §11.2).

use super::ctx::Ctx;
use super::helpers::{self, me, MapEntry};
use super::{Emitter, Family, Rule, Severity};
use std::sync::OnceLock;

const FAM: Family = Family::Clarity;

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
        r("passive-voice", passive_voice),
        r("long-sentence", long_sentence),
        r("wordy-phrase", wordy_phrase),
        r("complex-word", complex_word),
        r("nominalization", nominalization),
        r("weasel-word", weasel_word),
        r("redundant-pair", redundant_pair),
        r("repeated-word", repeated_word),
        r("there-is-expletive", there_is_expletive),
        r("adverb-overuse", adverb_overuse),
        r("undefined-acronym", undefined_acronym),
    ]
}

fn emit_map(
    ctx: &Ctx,
    em: &mut Emitter,
    id: &str,
    severity: Severity,
    re: &fancy_regex::Regex,
    map: &[MapEntry],
    label: &str,
) {
    helpers::scan_fancy(ctx, re, |off, len, m| {
        if let Some(to) = helpers::map_lookup(map, m) {
            em.emit(
                ctx,
                id,
                FAM,
                severity,
                off,
                len,
                format!("{label} — use '{to}'"),
            );
        }
    });
}

fn passive_voice(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::RegexBuilder::new(
        r"\b(am|is|are|was|were|be|been|being)\s+(?:\w+ly\s+){0,2}([A-Za-z]+(?:ed|en)|arisen|awoken|beaten|begun|broken|brought|built|chosen|done|drawn|driven|eaten|fallen|forgotten|frozen|given|gone|grown|hidden|known|made|paid|seen|sold|sent|shown|taken|thrown|told|thought|woven|written|found|held|kept|led|lost|meant|met|put|read|run|set)\b(?:\s+by\b)?"
    ).case_insensitive(true).build().unwrap());
    static PSEUDO: &[&str] = &[
        "even", "often", "seven", "open", "aspen", "been", "keen", "teen", "green", "screen",
        "then", "when", "hen", "pen", "ten", "amen", "omen", "alien", "barren", "brazen", "dozen",
        "garden", "golden", "heaven", "eleven", "hyphen", "kitchen", "linen", "listen", "oxygen",
        "siren", "sudden", "wooden", "woolen", "children", "happen", "chicken", "token", "red",
        "bed", "shed", "wed", "hundred", "indeed", "sacred", "naked", "wicked", "wretched",
        "crooked", "rugged", "ragged", "jagged", "hatred", "kindred",
    ];
    static ADJ: &[&str] = &[
        "interested",
        "located",
        "excited",
        "based",
        "related",
        "done",
        "born",
        "involved",
        "supposed",
        "used",
        "pleased",
        "concerned",
        "tired",
        "limited",
        "known",
        "given",
        "dedicated",
        "committed",
        "advanced",
        "detailed",
    ];
    helpers::scan(ctx, re, |off, len, m| {
        if !ctx.masked_identical(off, len) {
            return;
        }
        let part = re
            .captures(m)
            .and_then(|c| c.get(2))
            .map(|m| m.as_str().to_lowercase())
            .unwrap_or_default();
        if PSEUDO.contains(&part.as_str()) {
            return;
        }
        let has_by = m.to_lowercase().contains(" by");
        if ADJ.contains(&part.as_str()) && !has_by {
            return;
        }
        let after = &ctx.masked[off + len..(off + len + 16).min(ctx.masked.len())].to_lowercase();
        if !has_by
            && regex::Regex::new(r"^\s+(in|about|with|at|of|to|for)\b")
                .unwrap()
                .is_match(after)
        {
            return;
        }
        let sev = if has_by {
            Severity::Warn
        } else {
            Severity::Advisory
        };
        em.emit(
            ctx,
            "passive-voice",
            FAM,
            sev,
            off,
            len,
            "passive voice — make the actor the subject when possible",
        );
    });
}

fn long_sentence(ctx: &Ctx, em: &mut Emitter) {
    for s in &ctx.sentences {
        if s.words > 30 {
            em.emit(
                ctx,
                "long-sentence",
                FAM,
                Severity::Warn,
                s.start,
                s.end - s.start,
                format!("long sentence — {count} words", count = s.words),
            );
        }
    }
}

const WORDY: &[MapEntry] = &[
    me("in order to", "to"),
    me("due to the fact that", "because"),
    me("at this point in time", "now"),
    me("at the present time", "now"),
    me("in the event that", "if"),
    me("in spite of the fact that", "although"),
    me("with regard to", "about"),
    me("with respect to", "about"),
    me("for the purpose of", "to"),
    me("has the ability to", "can"),
    me("have the ability to", "can"),
    me("a number of", "some"),
    me("a majority of", "most"),
    me("in the near future", "soon"),
    me("on a regular basis", "regularly"),
    me("in close proximity to", "near"),
    me("take into consideration", "consider"),
];

fn wordy_phrase(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| helpers::map_regex(WORDY));
    emit_map(
        ctx,
        em,
        "wordy-phrase",
        Severity::Warn,
        re,
        WORDY,
        "wordy phrase",
    );
}

const COMPLEX: &[MapEntry] = &[
    me("utilize", "use"),
    me("utilizes", "use"),
    me("utilizing", "using"),
    me("utilization", "use"),
    me("facilitate", "help"),
    me("facilitates", "helps"),
    me("commence", "start"),
    me("commences", "starts"),
    me("endeavor", "try"),
    me("ascertain", "find out"),
    me("numerous", "many"),
    me("sufficient", "enough"),
    me("methodology", "method"),
    me("additional", "more"),
    me("approximately", "about"),
    me("demonstrate", "show"),
    me("demonstrates", "shows"),
    me("individuals", "people"),
    me("subsequently", "later"),
    me("prior", "before"),
    me("initiate", "start"),
    me("terminate", "end"),
    me("component", "part"),
    me("functionality", "features"),
];

fn complex_word(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| helpers::map_regex(COMPLEX));
    emit_map(
        ctx,
        em,
        "complex-word",
        Severity::Advisory,
        re,
        COMPLEX,
        "complex word",
    );
}

const NOMINAL: &[MapEntry] = &[
    me("make a decision", "decide"),
    me("made a decision", "decided"),
    me("conduct an investigation", "investigate"),
    me("provide assistance", "assist"),
    me("give consideration to", "consider"),
    me("reach a conclusion", "conclude"),
    me("perform an analysis", "analyze"),
    me("make an assumption", "assume"),
    me("come to an agreement", "agree"),
    me("take action", "act"),
    me("make a contribution", "contribute"),
    me("provide a description", "describe"),
    me("make an improvement", "improve"),
];

fn nominalization(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| helpers::map_regex(NOMINAL));
    emit_map(
        ctx,
        em,
        "nominalization",
        Severity::Advisory,
        re,
        NOMINAL,
        "nominalization",
    );
}

fn weasel_word(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        helpers::word_list(&[
            "very",
            "really",
            "quite",
            "fairly",
            "rather",
            "somewhat",
            "just",
            "basically",
            "actually",
            "simply",
            "literally",
            "extremely",
            "incredibly",
            "totally",
        ])
    });
    let hits: Vec<_> = re.find_iter(&ctx.masked).collect();
    let rate = hits.len() as f64 / ctx.word_count.max(1) as f64 * 1000.0;
    if hits.len() >= 2 && (hits.len() >= 3 || rate >= 4.0) {
        for m in hits {
            em.emit(
                ctx,
                "weasel-word",
                FAM,
                Severity::Advisory,
                m.start(),
                m.len(),
                "weasel word — remove it or make the claim precise",
            );
        }
    }
}

fn redundant_pair(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        helpers::phrase_list(&[
            "each and every",
            "first and foremost",
            "end result",
            "free gift",
            "past history",
            "future plans",
            "various different",
            "absolutely essential",
            "advance planning",
            "close proximity",
            "basic fundamentals",
            "completely eliminate",
            "final outcome",
            "unexpected surprise",
            "added bonus",
            "new innovation",
            "true fact",
        ])
    });
    helpers::scan_fancy(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "redundant-pair",
            FAM,
            Severity::Warn,
            off,
            len,
            "redundant pair — keep one word",
        );
    });
}

fn repeated_word(ctx: &Ctx, em: &mut Emitter) {
    let re = super::ctx::word_token_re();
    let mut prev: Option<(String, usize, usize)> = None;
    for m in re.find_iter(&ctx.masked) {
        let cur = m.as_str().to_lowercase();
        if let Some((p, poff, pend)) = &prev {
            let between = &ctx.masked[*pend..m.start()];
            if between.contains(['.', '!', '?', '\n']) {
                prev = Some((cur, m.start(), m.end()));
                continue;
            }
            if p == &cur {
                let phrase = format!("{p} {cur}");
                if phrase != "that that" && phrase != "had had" {
                    em.emit(
                        ctx,
                        "repeated-word",
                        FAM,
                        Severity::Warn,
                        *poff,
                        m.end() - *poff,
                        "repeated word",
                    );
                }
            }
        }
        prev = Some((cur, m.start(), m.end()));
    }
}

fn there_is_expletive(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        fancy_regex::RegexBuilder::new(
            r"(?i)(There (is|are|was|were)|It is)\s+[^.!?\n]{3,40}\s+(that|who|which)\b",
        )
        .build()
        .unwrap()
    });
    helpers::scan_fancy(ctx, re, |off, len, _| {
        if ctx.is_sentence_start(off) {
            em.emit(
                ctx,
                "there-is-expletive",
                FAM,
                Severity::Advisory,
                off,
                len,
                "expletive construction — start with the subject",
            );
        }
    });
}

fn adverb_overuse(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::RegexBuilder::new(r"\b[A-Za-z]{3,}ly\b")
            .case_insensitive(true)
            .build()
            .unwrap()
    });
    static STOP: &[&str] = &[
        "only", "family", "reply", "apply", "supply", "july", "italy", "ally", "rely", "multiply",
        "early", "ugly", "holy", "likely", "lonely", "friendly", "daily", "weekly", "monthly",
        "yearly", "silly", "jelly", "belly", "fully",
    ];
    let hits: Vec<_> = re
        .find_iter(&ctx.masked)
        .filter(|m| !STOP.contains(&m.as_str().to_lowercase().as_str()))
        .collect();
    let rate = hits.len() as f64 / ctx.word_count.max(1) as f64 * 1000.0;
    if hits.len() >= 5 && rate >= 25.0 {
        if let Some(m) = hits.first() {
            em.emit(
                ctx,
                "adverb-overuse",
                FAM,
                Severity::Advisory,
                m.start(),
                m.len(),
                format!("adverb overuse — {} hits, {:.1}/1k words", hits.len(), rate),
            );
        }
    }
}

fn undefined_acronym(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\b[A-Z]{3,5}s?\b").unwrap());
    static ALLOW: &[&str] = &[
        "API", "URL", "URI", "URN", "HTTP", "HTTPS", "JSON", "XML", "YAML", "TOML", "HTML", "CSS",
        "SQL", "DDL", "DML", "DOM", "UID", "UUID", "GUID", "CLI", "GUI", "CPU", "GPU", "PDF",
        "CSV", "FAQ", "USA", "UTF", "UTC", "MIT", "BSD", "GPL", "SaaS", "PaaS", "IaaS", "AWS",
        "GCP", "K8S", "JWT", "CORS", "XSS", "CSRF", "TODO", "FIXME", "JIRA",
    ];
    let mut seen = std::collections::HashSet::new();
    for m in re.find_iter(&ctx.masked) {
        let tok = m.as_str().trim_end_matches('s');
        if ALLOW.contains(&tok) || !seen.insert(tok.to_string()) {
            continue;
        }
        let defined =
            ctx.masked.contains(&format!("({tok})")) || ctx.masked.contains(&format!("{tok})"));
        if !defined {
            em.emit(
                ctx,
                "undefined-acronym",
                FAM,
                Severity::Advisory,
                m.start(),
                m.len(),
                "undefined acronym — spell it out on first use",
            );
        }
    }
}
