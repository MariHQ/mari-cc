//! Family D — inclusive & accessible (SPEC §11.9). Always on, no pack gate.

use super::ctx::Ctx;
use super::helpers::{self, me, MapEntry};
use super::{Emitter, Family, Rule, Severity};
use std::sync::OnceLock;

const FAM: Family = Family::Inclusive;

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
        r("gendered-language", gendered_language),
        r("ableist-language", ableist_language),
        r("vague-link-text", vague_link_text),
        r("skipped-heading", skipped_heading),
        r("person-first-language", person_first_language),
        r("gendered-address", gendered_address),
        r("tech-historical-terms", tech_historical_terms),
        r("violent-tech-metaphor", violent_tech_metaphor),
        r("ageist-classist-cultural", ageist_classist_cultural),
        r("missing-alt-text", missing_alt_text),
        r("all-caps-shouting", all_caps_shouting),
        r("bare-url", bare_url),
    ]
}

/// Emit one finding per match of a map rule; message ends with the
/// replacement in single quotes.
fn run_map(
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
                format!("{} — use '{}'", label, to),
            );
        }
    });
}

/// ±`pad`-byte context window around a match, clamped to char boundaries.
fn window(s: &str, start: usize, end: usize, pad: usize) -> &str {
    let mut a = start.saturating_sub(pad);
    while a > 0 && !s.is_char_boundary(a) {
        a -= 1;
    }
    let mut b = (end + pad).min(s.len());
    while b < s.len() && !s.is_char_boundary(b) {
        b += 1;
    }
    &s[a..b]
}

// ---------------------------------------------------------------- gendered

const GENDERED: &[MapEntry] = &[
    me("chairman", "chair"),
    me("chairmen", "chairs"),
    me("mankind", "humanity"),
    me("manpower", "workforce"),
    me("man-hours", "person-hours"),
    me("manned", "staffed"),
    me("salesman", "salesperson"),
    me("salesmen", "salespeople"),
    me("policeman", "police officer"),
    me("policemen", "police officers"),
    me("layman", "layperson"),
    me("laymen", "laypeople"),
    me("freshman", "first-year student"),
    me("fireman", "firefighter"),
    me("firemen", "firefighters"),
    me("stewardess", "flight attendant"),
    me("mailman", "mail carrier"),
    me("businessman", "businessperson"),
    me("man-made", "artificial"),
];

/// Pack precedence: the Microsoft/Google gender-bias pack rules also match
/// these; suppress here so one token never reports twice.
const PACK_OVERLAP: &[&str] = &[
    "mankind",
    "manpower",
    "salesman",
    "salesmen",
    "policeman",
    "policemen",
    "fireman",
    "firemen",
    "stewardess",
    "mailman",
    "freshman",
];

fn gendered_language(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| helpers::map_regex(GENDERED));
    let suppress = matches!(ctx.style_guide.as_str(), "microsoft" | "google");
    helpers::scan_fancy(ctx, re, |off, len, m| {
        let lower = m.to_lowercase();
        if suppress && PACK_OVERLAP.contains(&lower.as_str()) {
            return;
        }
        if let Some(to) = helpers::map_lookup(GENDERED, m) {
            em.emit(
                ctx,
                "gendered-language",
                FAM,
                Severity::Warn,
                off,
                len,
                format!("gendered term — use '{}'", to),
            );
        }
    });
}

// ---------------------------------------------------------------- ableist

const ABLEIST_WARN: &[MapEntry] = &[
    me("crazy", "wild / baffling"),
    me("insane", "extreme"),
    me("psycho", "erratic"),
    me("lame", "weak"),
    me("dumb", "foolish"),
    me("tone-deaf", "insensitive"),
    me("cripple", "degrade"),
    me("cripples", "degrades"),
    me("crippling", "degrading"),
];

const ABLEIST_ADVISORY: &[MapEntry] = &[
    me("sanity check", "consistency check"),
    me("sane", "reasonable"),
    me("dummy value", "placeholder value"),
];

fn ableist_language(ctx: &Ctx, em: &mut Emitter) {
    static WARN_RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    static ADV_RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let warn_re = WARN_RE.get_or_init(|| helpers::map_regex(ABLEIST_WARN));
    let adv_re = ADV_RE.get_or_init(|| helpers::map_regex(ABLEIST_ADVISORY));
    run_map(
        ctx,
        em,
        "ableist-language",
        Severity::Warn,
        warn_re,
        ABLEIST_WARN,
        "ableist term",
    );
    run_map(
        ctx,
        em,
        "ableist-language",
        Severity::Advisory,
        adv_re,
        ABLEIST_ADVISORY,
        "ableist idiom",
    );
}

// ---------------------------------------------------------------- vague links

const VAGUE_LINK: &[&str] = &[
    "click here",
    "here",
    "read more",
    "this",
    "this link",
    "link",
    "more",
];

fn vague_link_text(ctx: &Ctx, em: &mut Emitter) {
    for l in &ctx.links {
        let t = l.text.trim().to_lowercase();
        if VAGUE_LINK.contains(&t.as_str()) {
            em.emit(
                ctx,
                "vague-link-text",
                FAM,
                Severity::Warn,
                l.start,
                l.length,
                "vague link text — describe the destination (WCAG)",
            );
        }
    }
}

// ---------------------------------------------------------------- headings

fn skipped_heading(ctx: &Ctx, em: &mut Emitter) {
    let mut prev: Option<usize> = None;
    let mut h1_seen = false;
    for h in &ctx.headings {
        if let Some(p) = prev {
            if h.level > p + 1 {
                em.emit(
                    ctx,
                    "skipped-heading",
                    FAM,
                    Severity::Warn,
                    h.start,
                    h.text.len(),
                    format!(
                        "heading skips from h{} to h{} — don't skip levels",
                        p, h.level
                    ),
                );
            }
        }
        if h.level == 1 {
            if h1_seen {
                em.emit(
                    ctx,
                    "skipped-heading",
                    FAM,
                    Severity::Advisory,
                    h.start,
                    h.text.len(),
                    "second h1 — use a single top-level heading",
                );
            }
            h1_seen = true;
        }
        prev = Some(h.level);
    }
}

// ---------------------------------------------------------------- person-first

const PERSON_FIRST: &[MapEntry] = &[
    me("suffers from", "has"),
    me("suffering from", "living with"),
    me("victim of", "person affected by"),
    me("wheelchair-bound", "wheelchair user"),
    me("confined to a wheelchair", "uses a wheelchair"),
    me("an epileptic", "a person with epilepsy"),
    me("the disabled", "disabled people"),
    me("the mentally ill", "people with mental illness"),
    me("normal people", "people without disabilities"),
];

fn person_first_language(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| helpers::map_regex(PERSON_FIRST));
    run_map(
        ctx,
        em,
        "person-first-language",
        Severity::Warn,
        re,
        PERSON_FIRST,
        "person-first language",
    );
}

// ---------------------------------------------------------------- gendered address

fn gendered_address(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| helpers::word_list(&["guys", "gentlemen", "ladies"]));
    helpers::scan(ctx, re, |off, len, _m| {
        em.emit(
            ctx,
            "gendered-address",
            FAM,
            Severity::Advisory,
            off,
            len,
            "gendered address — use 'everyone / folks'",
        );
    });
}

// ---------------------------------------------------------------- tech-historical

const TECH_WARN: &[MapEntry] = &[
    me("blacklist", "blocklist"),
    me("blacklists", "blocklists"),
    me("blacklisted", "blocked"),
    me("whitelist", "allowlist"),
    me("whitelists", "allowlists"),
    me("whitelisted", "allowed"),
    me("master/slave", "primary/replica"),
    me("grandfathered", "legacy"),
    me("grandfather", "legacy"),
    me("blackhat", "unethical"),
    me("whitehat", "ethical"),
    me("first-class citizen", "fully supported"),
    me("sanity", "confidence"),
];

const TECH_ADVISORY: &[MapEntry] = &[
    me("master", "primary / main"),
    me("slave", "replica / worker"),
    me("native", "built-in"),
    me("primitive", "basic"),
    me("tribe", "team"),
];

const TECH_EXEMPT: &str =
    r"master's|scrum master|master class|native speaker|primitive type|native to";

fn tech_historical_terms(ctx: &Ctx, em: &mut Emitter) {
    static WARN_RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    static ADV_RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    static EXEMPT_RE: OnceLock<regex::Regex> = OnceLock::new();
    let warn_re = WARN_RE.get_or_init(|| helpers::map_regex(TECH_WARN));
    let adv_re = ADV_RE.get_or_init(|| helpers::map_regex(TECH_ADVISORY));
    let exempt = EXEMPT_RE.get_or_init(|| {
        regex::RegexBuilder::new(TECH_EXEMPT)
            .case_insensitive(true)
            .build()
            .unwrap()
    });

    // Warn map first; remember its ranges so the advisory singles (`master`,
    // `slave`) never re-report inside `master/slave`.
    let mut warn_ranges: Vec<(usize, usize)> = Vec::new();
    helpers::scan_fancy(ctx, warn_re, |off, len, m| {
        if let Some(to) = helpers::map_lookup(TECH_WARN, m) {
            warn_ranges.push((off, off + len));
            em.emit(
                ctx,
                "tech-historical-terms",
                FAM,
                Severity::Warn,
                off,
                len,
                format!("legacy tech term — use '{}'", to),
            );
        }
    });
    helpers::scan_fancy(ctx, adv_re, |off, len, m| {
        if warn_ranges.iter().any(|&(a, b)| off < b && off + len > a) {
            return;
        }
        if exempt.is_match(window(&ctx.masked, off, off + len, 12)) {
            return;
        }
        if let Some(to) = helpers::map_lookup(TECH_ADVISORY, m) {
            em.emit(
                ctx,
                "tech-historical-terms",
                FAM,
                Severity::Advisory,
                off,
                len,
                format!("context-dependent legacy term — consider '{}'", to),
            );
        }
    });
}

// ---------------------------------------------------------------- violent

const VIOLENT: &[MapEntry] = &[
    me("abort", "stop"),
    me("aborts", "stops"),
    me("kill", "end"),
    me("killing", "ending"),
    me("hang", "stop responding"),
    me("hangs", "stops responding"),
    me("blast radius", "scope of impact"),
    me("dmz", "perimeter network"),
];

fn violent_tech_metaphor(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    static NUM_RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| helpers::map_regex(VIOLENT));
    let num = NUM_RE.get_or_init(|| regex::Regex::new(r"^[ \t]*-?\d").unwrap());
    helpers::scan_fancy(ctx, re, |off, len, m| {
        // Suppressed when followed by a number (`kill -9`).
        if num.is_match(&ctx.masked[off + len..]) {
            return;
        }
        if let Some(to) = helpers::map_lookup(VIOLENT, m) {
            em.emit(
                ctx,
                "violent-tech-metaphor",
                FAM,
                Severity::Advisory,
                off,
                len,
                format!("violent metaphor — use '{}'", to),
            );
        }
    });
}

// ---------------------------------------------------------------- ageist etc.

const AGEIST: &[MapEntry] = &[
    me("ghetto", "makeshift"),
    me("gypsy", "traveler"),
    me("gypped", "cheated"),
    me("oriental", "Asian"),
    me("eskimo", "Inuit"),
    me("third-world", "developing"),
    me("third world", "developing"),
    me("the elderly", "older adults"),
    me("illegal immigrant", "undocumented immigrant"),
    me("illegal alien", "undocumented immigrant"),
    me("sketchy", "questionable"),
];

fn ageist_classist_cultural(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| helpers::map_regex(AGEIST));
    run_map(
        ctx,
        em,
        "ageist-classist-cultural",
        Severity::Advisory,
        re,
        AGEIST,
        "loaded term",
    );
}

// ---------------------------------------------------------------- alt text

fn missing_alt_text(ctx: &Ctx, em: &mut Emitter) {
    for img in &ctx.images {
        if img.alt.trim().is_empty() {
            em.emit(
                ctx,
                "missing-alt-text",
                FAM,
                Severity::Warn,
                img.start,
                img.length,
                "image has empty alt text — describe it for screen readers",
            );
        }
    }
}

// ---------------------------------------------------------------- shouting

fn all_caps_shouting(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\b[A-Z]{2,}(?:[ \t]+[A-Z]{2,}){2,}\b").unwrap());
    helpers::scan(ctx, re, |off, len, _m| {
        em.emit(
            ctx,
            "all-caps-shouting",
            FAM,
            Severity::Advisory,
            off,
            len,
            "all-caps run — screen readers spell it out",
        );
    });
}

// ---------------------------------------------------------------- bare URL

fn bare_url(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| fancy_regex::Regex::new(r#"(?<![("'<=\]])https?://\S+"#).unwrap());
    helpers::scan_fancy(ctx, re, |off, len, _m| {
        let line = ctx.line_of(off);
        if ctx.ref_defs.iter().any(|d| d.line == line) {
            return;
        }
        em.emit(
            ctx,
            "bare-url",
            FAM,
            Severity::Advisory,
            off,
            len,
            "bare URL in prose — use descriptive link text",
        );
    });
}

// ================================================================= tests

#[cfg(test)]
mod tests {
    use crate::detector::runner::{detect_text, test_settings};
    use crate::detector::{Finding, Severity};

    fn findings(text: &str, style: &str) -> Vec<Finding> {
        detect_text("test.md", text, &test_settings(style)).findings
    }

    fn of(text: &str, style: &str, id: &str) -> Vec<Finding> {
        findings(text, style)
            .into_iter()
            .filter(|f| f.rule_id == id)
            .collect()
    }

    fn fires(text: &str, id: &str) -> bool {
        !of(text, "plain", id).is_empty()
    }

    #[test]
    fn gendered_language_bad_good() {
        let f = of("The chairman spoke.", "plain", "gendered-language");
        assert_eq!(f.len(), 1);
        assert!(f[0].message.ends_with("'chair'"));
        assert!(fires("It was man-made.", "gendered-language"));
        assert!(!fires("The chair spoke.", "gendered-language"));
    }

    #[test]
    fn gendered_language_pack_precedence() {
        // Overlapping terms suppressed under microsoft/google.
        assert!(fires("All of mankind agrees.", "gendered-language"));
        assert!(of("All of mankind agrees.", "microsoft", "gendered-language").is_empty());
        assert!(of(
            "The mailman and the fireman.",
            "google",
            "gendered-language"
        )
        .is_empty());
        // Non-overlapping terms still fire under microsoft.
        assert_eq!(
            of("The chairman spoke.", "microsoft", "gendered-language").len(),
            1
        );
    }

    #[test]
    fn ableist_language_bad_good() {
        let warn = of("That plan is crazy.", "plain", "ableist-language");
        assert_eq!(warn.len(), 1);
        assert_eq!(warn[0].severity, Severity::Warn);
        assert!(warn[0].message.ends_with("'wild / baffling'"));
        let adv = of("Insert a dummy value there.", "plain", "ableist-language");
        assert_eq!(adv.len(), 1);
        assert_eq!(adv[0].severity, Severity::Advisory);
        assert!(fires("Run a sanity check first.", "ableist-language"));
        assert!(fires("The bug cripples throughput.", "ableist-language"));
        assert!(!fires("That plan is wild.", "ableist-language"));
        assert!(!fires("Run a consistency check first.", "ableist-language"));
    }

    #[test]
    fn vague_link_text_bad_good() {
        assert!(fires(
            "See [click here](https://x.example) for setup.",
            "vague-link-text"
        ));
        assert!(fires(
            "Read [ More ](https://x.example) about it.",
            "vague-link-text"
        ));
        assert!(!fires(
            "See the [install guide](https://x.example) for setup.",
            "vague-link-text"
        ));
    }

    #[test]
    fn skipped_heading_bad_good() {
        let f = of("# a\n\n## b\n\n#### c\n", "plain", "skipped-heading");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, Severity::Warn);
        let f2 = of("# a\n\nbody text.\n\n# b\n", "plain", "skipped-heading");
        assert_eq!(f2.len(), 1);
        assert_eq!(f2[0].severity, Severity::Advisory);
        assert!(!fires("# a\n\n## b\n\n### c\n", "skipped-heading"));
    }

    #[test]
    fn person_first_language_bad_good() {
        let f = of(
            "He suffers from epilepsy.",
            "plain",
            "person-first-language",
        );
        assert_eq!(f.len(), 1);
        assert!(f[0].message.ends_with("'has'"));
        assert!(fires("She is wheelchair-bound.", "person-first-language"));
        assert!(!fires("He has epilepsy.", "person-first-language"));
    }

    #[test]
    fn gendered_address_bad_good() {
        let f = of("Hey guys, welcome back.", "plain", "gendered-address");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, Severity::Advisory);
        assert!(f[0].message.ends_with("'everyone / folks'"));
        assert!(!fires("Hey folks, welcome back.", "gendered-address"));
    }

    #[test]
    fn tech_historical_warn_bad_good() {
        let f = of(
            "Add the domain to the blacklist.",
            "plain",
            "tech-historical-terms",
        );
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, Severity::Warn);
        assert!(f[0].message.ends_with("'blocklist'"));
        assert!(!fires(
            "Add the domain to the blocklist.",
            "tech-historical-terms"
        ));
    }

    #[test]
    fn tech_historical_advisory_and_exemptions() {
        let f = of(
            "Merge to the master branch.",
            "plain",
            "tech-historical-terms",
        );
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, Severity::Advisory);
        // ±12-char exemption regex.
        assert!(!fires(
            "She holds a master's degree.",
            "tech-historical-terms"
        ));
        assert!(!fires(
            "Ask the scrum master today.",
            "tech-historical-terms"
        ));
        assert!(!fires("Cats are native to Egypt.", "tech-historical-terms"));
        assert!(!fires(
            "Every primitive type works.",
            "tech-historical-terms"
        ));
        // `master/slave` reports once (warn), not three times.
        let f2 = of(
            "It uses master/slave replication.",
            "plain",
            "tech-historical-terms",
        );
        assert_eq!(f2.len(), 1);
        assert_eq!(f2[0].severity, Severity::Warn);
        assert!(f2[0].message.ends_with("'primary/replica'"));
    }

    #[test]
    fn violent_tech_metaphor_bad_good() {
        let f = of(
            "This will kill the worker.",
            "plain",
            "violent-tech-metaphor",
        );
        assert_eq!(f.len(), 1);
        assert!(f[0].message.ends_with("'end'"));
        assert!(fires(
            "Reduce the blast radius of the change.",
            "violent-tech-metaphor"
        ));
        // Suppressed when followed by a number.
        assert!(!fires(
            "Run kill -9 on the worker.",
            "violent-tech-metaphor"
        ));
        // `hit` deliberately excluded.
        assert!(!fires(
            "A cache hit means we hit the endpoint.",
            "violent-tech-metaphor"
        ));
        assert!(!fires("This will end the worker.", "violent-tech-metaphor"));
    }

    #[test]
    fn ageist_classist_cultural_bad_good() {
        let f = of(
            "That vendor seems sketchy.",
            "plain",
            "ageist-classist-cultural",
        );
        assert_eq!(f.len(), 1);
        assert!(f[0].message.ends_with("'questionable'"));
        assert!(fires("Built for the elderly.", "ageist-classist-cultural"));
        assert!(!fires(
            "That vendor seems questionable.",
            "ageist-classist-cultural"
        ));
    }

    #[test]
    fn missing_alt_text_bad_good() {
        assert!(fires("A diagram: ![](flow.png)", "missing-alt-text"));
        assert!(!fires(
            "A diagram: ![flow of requests](flow.png)",
            "missing-alt-text"
        ));
    }

    #[test]
    fn all_caps_shouting_bad_good() {
        let f = of(
            "You must NEVER EVER DELETE the key.",
            "plain",
            "all-caps-shouting",
        );
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, Severity::Advisory);
        // Two caps words are fine, as are acronym pairs.
        assert!(!fires("Use the HTTP API for uploads.", "all-caps-shouting"));
        assert!(!fires(
            "You must never delete the key.",
            "all-caps-shouting"
        ));
    }

    #[test]
    fn bare_url_bad_good() {
        let f = of(
            "Docs live at https://example.com today.",
            "plain",
            "bare-url",
        );
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, Severity::Advisory);
        // Link target, autolink, and reference definition are all fine.
        assert!(!fires(
            "See the [docs](https://example.com) today.",
            "bare-url"
        ));
        assert!(!fires("See <https://example.com> today.", "bare-url"));
        assert!(!fires("[docs]: https://example.com\n", "bare-url"));
    }
}
