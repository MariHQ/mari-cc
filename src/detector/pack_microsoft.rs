//! Microsoft style pack native rules (SPEC §11.4).

use super::ctx::Ctx;
use super::helpers;
use super::{Emitter, Family, Rule, Severity};
use std::sync::OnceLock;

const FAM: Family = Family::Style;
const PACK: &str = "microsoft";

fn r(id: &'static str, run: fn(&Ctx, &mut Emitter)) -> Rule {
    Rule {
        id,
        family: FAM,
        pack: Some(PACK),
        run,
    }
}

pub fn rules() -> Vec<Rule> {
    vec![
        r("no-space-em-dash", no_space_em_dash),
        r("no-internal-caps", no_internal_caps),
        r("omit-you-can", omit_you_can),
        r("avoid-we", avoid_we),
        r("spell-out-small-numbers", spell_out_small_numbers),
        r("no-numeral-sentence-start", no_numeral_sentence_start),
        r("large-number-grouping", large_number_grouping),
        r("no-k-m-b", no_k_m_b),
        r("leading-zero", leading_zero),
        r("microsoft-ampm", microsoft_ampm),
        r("microsoft-accessibility", microsoft_accessibility),
        r("microsoft-auto-hyphenation", microsoft_auto_hyphenation),
        r("microsoft-avoid-words", microsoft_avoid_words),
        r("microsoft-contractions", microsoft_contractions),
        r("ms-date-format", ms_date_format),
        r("ms-date-numbers", ms_date_numbers),
        r("ms-date-order", ms_date_order),
        r("ms-ellipses", ms_ellipses),
        r("ms-first-person", ms_first_person),
        r("ms-foreign-abbrev", ms_foreign_abbrev),
        r("ms-gender-slash", ms_gender_slash),
        r("ms-gender-bias", ms_gender_bias),
        r("microsoft-general-url", microsoft_general_url),
        r("microsoft-heading-acronyms", microsoft_heading_acronyms),
        r("microsoft-heading-colons", microsoft_heading_colons),
        r("ms-adverb-hyphen", ms_adverb_hyphen),
        r("ms-negative-number-endash", ms_negative_number_endash),
        r("ms-ordinal-ly", ms_ordinal_ly),
        r("ms-percentages", ms_percentages),
        r("ms-plurals-parenthetical", ms_plurals_parenthetical),
        r("microsoft-quotes-punctuation", microsoft_quotes_punctuation),
        r("microsoft-range-time", microsoft_range_time),
        r("microsoft-semicolon", microsoft_semicolon),
        r("ms-suspended-hyphen", ms_suspended_hyphen),
        r("ms-term-swaps", ms_term_swaps),
        r("ms-url-of", ms_url_of),
        r("ms-units-spelled-number", ms_units_spelled_number),
        r("ms-vocab-az-wordlist", ms_vocab_az_wordlist),
        r("ms-wordiness", ms_wordiness),
        Rule {
            id: "microsoft-adverbs",
            family: Family::Clarity,
            pack: Some(PACK),
            run: microsoft_adverbs,
        },
    ]
}

fn no_space_em_dash(ctx: &Ctx, em: &mut Emitter) {
    let count = ctx.masked.matches(" — ").count();
    if count == 0 {
        return;
    }
    if let Some(off) = ctx.masked.find(" — ") {
        em.emit(
            ctx,
            "no-space-em-dash",
            FAM,
            Severity::Advisory,
            off,
            " — ".len(),
            format!("Microsoft style closes up spaced em dashes ({count} found)"),
        );
    }
}

fn no_internal_caps(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    static ALLOW: &[&str] = &[
        "JavaScript",
        "TypeScript",
        "GitHub",
        "GitLab",
        "GraphQL",
        "PostgreSQL",
        "MySQL",
        "iPhone",
        "iPad",
        "iOS",
        "macOS",
        "YouTube",
        "PayPal",
        "WordPress",
        "LinkedIn",
        "DevOps",
        "WiFi",
        "eBay",
        "OpenAI",
        "npm",
    ];
    let re = RE.get_or_init(|| regex::Regex::new(r"\b[a-z]+[A-Z]\w*\b").unwrap());
    helpers::scan(ctx, re, |off, len, m| {
        if ALLOW.contains(&m) || m.chars().any(|c| c.is_ascii_digit()) || m.len() > 16 {
            return;
        }
        let uppercase = m.chars().filter(|c| c.is_ascii_uppercase()).count();
        if uppercase > 1 {
            return;
        }
        em.emit(
            ctx,
            "no-internal-caps",
            FAM,
            Severity::Advisory,
            off,
            len,
            "avoid internal capitalization in prose",
        );
    });
}

fn omit_you_can(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::RegexBuilder::new(r"\byou can\b")
            .case_insensitive(true)
            .build()
            .unwrap()
    });
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "omit-you-can",
            FAM,
            Severity::Advisory,
            off,
            len,
            "often cut 'you can' and use the imperative",
        );
    });
}

fn avoid_we(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::RegexBuilder::new(r"\b(we|we're|our|us)\b")
            .case_insensitive(true)
            .build()
            .unwrap()
    });
    let mut first = None;
    let mut count = 0usize;
    helpers::scan(ctx, re, |off, len, _| {
        first.get_or_insert((off, len));
        count += 1;
    });
    if count >= 3 {
        let (off, len) = first.unwrap();
        em.emit(
            ctx,
            "avoid-we",
            FAM,
            Severity::Advisory,
            off,
            len,
            format!("avoid first-person plural in docs ({count} hits)"),
        );
    }
}

fn spell_out_small_numbers(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| fancy_regex::Regex::new(r"(?<![\w.$%/-])([0-9])(?![\w.$%/-])").unwrap());
    helpers::scan_fancy(ctx, re, |off, len, _| {
        if !ctx.is_table_line(off) {
            em.emit(
                ctx,
                "spell-out-small-numbers",
                FAM,
                Severity::Advisory,
                off,
                len,
                "spell out standalone single-digit numbers",
            );
        }
    });
}

fn no_numeral_sentence_start(ctx: &Ctx, em: &mut Emitter) {
    static ORDERED_LIST: OnceLock<regex::Regex> = OnceLock::new();
    let ordered = ORDERED_LIST.get_or_init(|| regex::Regex::new(r"^\s*\d+[.)]\s*$").unwrap());
    for s in &ctx.sentences {
        let text = &ctx.masked[s.start..s.end];
        let trimmed = text.trim_start();
        let off = s.start + (text.len() - trimmed.len());
        if trimmed
            .as_bytes()
            .first()
            .map(|b| b.is_ascii_digit())
            .unwrap_or(false)
        {
            let line_start = ctx.masked[..off].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let line_prefix = &ctx.masked[line_start..off];
            if ordered.is_match(line_prefix) {
                continue;
            }
            em.emit(
                ctx,
                "no-numeral-sentence-start",
                FAM,
                Severity::Advisory,
                off,
                1,
                "don't start a sentence with a numeral",
            );
        }
    }
}

fn large_number_grouping(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\b\d{5,}\b").unwrap());
    helpers::scan(ctx, re, |off, len, m| {
        if ctx.is_table_line(off) {
            return;
        }
        let grouped = group_digits(m);
        em.emit(
            ctx,
            "large-number-grouping",
            FAM,
            Severity::Advisory,
            off,
            len,
            format!("group large numbers with commas — use '{grouped}'"),
        );
    });
}

fn group_digits(s: &str) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

fn no_k_m_b(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\$?\b\d+(?:\.\d+)?\s?[KMB]\b").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "no-k-m-b",
            FAM,
            Severity::Advisory,
            off,
            len,
            "spell out thousand, million, or billion",
        );
    });
}

fn leading_zero(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| fancy_regex::Regex::new(r"(?<!\d)\.\d+").unwrap());
    helpers::scan_fancy(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "leading-zero",
            FAM,
            Severity::Advisory,
            off,
            len,
            "use a leading zero before a decimal",
        );
    });
}

#[cfg(test)]
mod tests {
    use super::super::runner::{detect_text, test_settings};

    fn ids(text: &str) -> Vec<String> {
        detect_text("x.md", text, &test_settings("microsoft"))
            .findings
            .into_iter()
            .map(|f| f.rule_id)
            .collect()
    }

    #[test]
    fn microsoft_native_pack_rules_fire() {
        let got = ids(
            "7 users saw 12000 events — then you can retry. We saw our team tell us that fooBar handles $5M at .5 scale.",
        );
        for id in [
            "no-numeral-sentence-start",
            "spell-out-small-numbers",
            "large-number-grouping",
            "no-space-em-dash",
            "omit-you-can",
            "avoid-we",
            "no-internal-caps",
            "no-k-m-b",
            "leading-zero",
        ] {
            assert!(got.iter().any(|g| g == id), "{id}: {got:?}");
        }
    }
}

// ---- Vale-parity ports (SPEC §11.4) ----

use super::helpers::{me, MapEntry};

fn run_map(
    ctx: &Ctx,
    em: &mut Emitter,
    id: &str,
    family: Family,
    sev: Severity,
    list_id: &str,
    what: &str,
) {
    let re = ctx.lists.map_regex(list_id);
    helpers::scan_fancy(ctx, &re, |off, len, m| {
        if let Some(to) = ctx.lists.map_lookup(list_id, m) {
            em.emit(
                ctx,
                id,
                family,
                sev,
                off,
                len,
                format!("{what} — use '{to}'"),
            );
        }
    });
}

fn microsoft_ampm(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"(?i)\b\d{1,2}(?::\d{2})?( ?)([ap])(\.?)m\b\.?").unwrap()
    });
    helpers::scan(ctx, re, |off, len, m| {
        if ctx.is_table_line(off) {
            return;
        }
        // Valid form: space + uppercase AM/PM, no periods.
        let ok = regex::Regex::new(r"\d(?::\d{2})? [AP]M$")
            .unwrap()
            .is_match(m);
        if !ok {
            em.emit(
                ctx,
                "microsoft-ampm",
                FAM,
                Severity::Advisory,
                off,
                len,
                "time format — use 'AM' or 'PM' (preceded by a space)",
            );
        }
    });
}

pub const ACCESSIBILITY_TERMS: &[&str] = &[
    "a victim of",
    "able-bodied",
    "an epileptic",
    "birth defect",
    "crippled",
    "differently abled",
    "disabled",
    "dumb",
    "handicapped",
    "handicaps",
    "healthy person",
    "hearing-impaired",
    "lame",
    "maimed",
    "mentally handicapped",
    "missing a limb",
    "mute",
    "non-verbal",
    "normal person",
    "sight-impaired",
    "slow learner",
    "stricken with",
    "suffers from",
    "vision-impaired",
];

fn microsoft_accessibility(ctx: &Ctx, em: &mut Emitter) {
    let re = ctx.lists.phrase_regex("microsoft-accessibility");
    helpers::scan_fancy(ctx, &re, |off, len, _| {
        em.emit(
            ctx,
            "microsoft-accessibility",
            Family::Inclusive,
            Severity::Advisory,
            off,
            len,
            "don't define people by disability — describe the person, not the condition",
        );
    });
}

fn microsoft_auto_hyphenation(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"(?i)\bauto-[a-z]+").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "microsoft-auto-hyphenation",
            FAM,
            Severity::Advisory,
            off,
            len,
            "in general, don't hyphenate 'auto' prefixes",
        );
    });
}

fn microsoft_avoid_words(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        fancy_regex::RegexBuilder::new(
            r"(?<![A-Za-z0-9_])(abortion|and so on|app(?:lication)?s? (?:developer|program)|app(?:lication)? file|backbone|backend|contiguous selection)(?![A-Za-z0-9_])",
        )
        .case_insensitive(true)
        .build()
        .unwrap()
    });
    helpers::scan_fancy(ctx, re, |off, len, m| {
        em.emit(
            ctx,
            "microsoft-avoid-words",
            FAM,
            Severity::Advisory,
            off,
            len,
            format!("'{m}' is on the Microsoft avoid list"),
        );
    });
}

pub const MS_CONTRACTIONS: &[MapEntry] = &[
    me("how is", "how's"),
    me("it is", "it's"),
    me("that is", "that's"),
    me("they are", "they're"),
    me("we are", "we're"),
    me("we have", "we've"),
    me("what is", "what's"),
    me("when is", "when's"),
    me("where is", "where's"),
];

fn microsoft_contractions(ctx: &Ctx, em: &mut Emitter) {
    run_map(
        ctx,
        em,
        "microsoft-contractions",
        FAM,
        Severity::Advisory,
        "microsoft-contractions",
        "prefer the contraction",
    );
}

const MONTHS: &str =
    "January|February|March|April|May|June|July|August|September|October|November|December";

fn ms_date_format(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(&format!(r"\b\d{{1,2}} (?:{MONTHS}),? \d{{4}}\b")).unwrap()
    });
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "ms-date-format",
            FAM,
            Severity::Advisory,
            off,
            len,
            "date format — use 'July 31, 2016'",
        );
    });
}

fn ms_date_numbers(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(&format!(
            r"(?i)\b(?:{MONTHS}) (?:first|second|third|fourth|fifth|sixth|seventh|eighth|ninth|tenth|eleventh|twelfth|thirteenth|fourteenth|fifteenth|sixteenth|seventeenth|eighteenth|nineteenth|twentieth|twenty-first|twenty-second|twenty-third|twenty-fourth|twenty-fifth|twenty-sixth|twenty-seventh|twenty-eighth|twenty-ninth|thirtieth|thirty-first)\b"
        ))
        .unwrap()
    });
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "ms-date-numbers",
            FAM,
            Severity::Advisory,
            off,
            len,
            "don't use ordinals for dates — use the numeral",
        );
    });
}

fn ms_date_order(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\b\d{1,2}/\d{1,2}/\d{2,4}\b").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        if !ctx.is_table_line(off) {
            em.emit(
                ctx,
                "ms-date-order",
                FAM,
                Severity::Advisory,
                off,
                len,
                "always spell out the month — 'July 31, 2016'",
            );
        }
    });
}

fn ms_ellipses(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\.\.\.|…").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        if !ctx.is_table_line(off) {
            em.emit(
                ctx,
                "ms-ellipses",
                FAM,
                Severity::Advisory,
                off,
                len,
                "avoid ellipses in UI text and docs",
            );
        }
    });
}

fn ms_first_person(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"\b(I|I'd|I'll|I'm|I've)\b|(?i)\b(me|my|mine)\b").unwrap()
    });
    let mut hits: Vec<(usize, usize)> = Vec::new();
    helpers::scan(ctx, re, |off, len, _| hits.push((off, len)));
    if hits.len() >= 2 {
        for (off, len) in hits {
            em.emit(
                ctx,
                "ms-first-person",
                FAM,
                Severity::Warn,
                off,
                len,
                "use first person sparingly",
            );
        }
    }
}

fn ms_foreign_abbrev(ctx: &Ctx, em: &mut Emitter) {
    // Dotted forms match either case; bare eg/ie/ergo must be lowercase and
    // followed by space/comma (so 'IE' the browser doesn't flag).
    static DOTTED: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let dotted = DOTTED.get_or_init(|| {
        fancy_regex::RegexBuilder::new(r"(?<![A-Za-z0-9_])(e\.g\.|i\.e\.|viz\.)(?![A-Za-z0-9_])")
            .case_insensitive(true)
            .build()
            .unwrap()
    });
    let repl = |m: &str| match m.to_lowercase().trim_end_matches('.') {
        "e.g" | "eg" => "for example",
        "i.e" | "ie" => "that is",
        "viz" => "namely",
        _ => "therefore",
    };
    helpers::scan_fancy(ctx, dotted, |off, len, m| {
        em.emit(
            ctx,
            "ms-foreign-abbrev",
            FAM,
            Severity::Advisory,
            off,
            len,
            format!("foreign abbreviation — use '{}'", repl(m)),
        );
    });
    static BARE: OnceLock<regex::Regex> = OnceLock::new();
    let bare = BARE.get_or_init(|| regex::Regex::new(r"\b(eg|ie|ergo)[ ,]").unwrap());
    helpers::scan(ctx, bare, |off, len, m| {
        em.emit(
            ctx,
            "ms-foreign-abbrev",
            FAM,
            Severity::Advisory,
            off,
            len - 1,
            format!(
                "foreign abbreviation — use '{}'",
                repl(m.trim_end_matches([' ', ',']))
            ),
        );
    });
}

fn ms_gender_slash(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"(?i)\b(he/she|s/he)\b").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "ms-gender-slash",
            Family::Inclusive,
            Severity::Warn,
            off,
            len,
            "gendered slash form — use 'they'",
        );
    });
}

pub const GENDER_BIAS: &[MapEntry] = &[
    me("alumna", "graduate"),
    me("alumnus", "graduate"),
    me("alumnae", "graduates"),
    me("alumni", "graduates"),
    me("airman", "pilot(s)"),
    me("airmen", "pilot(s)"),
    me("airwoman", "pilot(s)"),
    me("airwomen", "pilot(s)"),
    me("anchorman", "anchor(s)"),
    me("anchormen", "anchor(s)"),
    me("anchorwoman", "anchor(s)"),
    me("anchorwomen", "anchor(s)"),
    me("authoress", "author"),
    me("cameraman", "camera operator(s)"),
    me("cameramen", "camera operator(s)"),
    me("camerawoman", "camera operator(s)"),
    me("camerawomen", "camera operator(s)"),
    me("doorman", "concierge(s)"),
    me("doormen", "concierge(s)"),
    me("doorwoman", "concierge(s)"),
    me("doorwomen", "concierge(s)"),
    me("draftsman", "drafter(s)"),
    me("draftsmen", "drafter(s)"),
    me("draftswoman", "drafter(s)"),
    me("draftswomen", "drafter(s)"),
    me("fireman", "firefighter(s)"),
    me("firemen", "firefighter(s)"),
    me("firewoman", "firefighter(s)"),
    me("firewomen", "firefighter(s)"),
    me("fisherman", "fisher(s)"),
    me("fishermen", "fisher(s)"),
    me("fisherwoman", "fisher(s)"),
    me("fisherwomen", "fisher(s)"),
    me("freshman", "first-year student(s)"),
    me("freshmen", "first-year student(s)"),
    me("freshwoman", "first-year student(s)"),
    me("freshwomen", "first-year student(s)"),
    me("garbageman", "waste collector(s)"),
    me("garbagemen", "waste collector(s)"),
    me("garbagewoman", "waste collector(s)"),
    me("garbagewomen", "waste collector(s)"),
    me("lady lawyer", "lawyer"),
    me("ladylike", "courteous"),
    me("mailman", "mail carriers"),
    me("mailmen", "mail carriers"),
    me("mailwoman", "mail carriers"),
    me("mailwomen", "mail carriers"),
    me("man and wife", "husband and wife"),
    me("man enough", "strong enough"),
    me("mankind", "human kind"),
    me("manmade", "manufactured"),
    me("manpower", "personnel"),
    me("middleman", "intermediary"),
    me("middlemen", "intermediary"),
    me("middlewoman", "intermediary"),
    me("middlewomen", "intermediary"),
    me("newsman", "journalist(s)"),
    me("newsmen", "journalist(s)"),
    me("newswoman", "journalist(s)"),
    me("newswomen", "journalist(s)"),
    me("ombudsman", "ombuds"),
    me("ombudsmen", "ombuds"),
    me("ombudswoman", "ombuds"),
    me("ombudswomen", "ombuds"),
    me("oneupmanship", "upstaging"),
    me("poetess", "poet"),
    me("policeman", "police officer(s)"),
    me("policemen", "police officer(s)"),
    me("policewoman", "police officer(s)"),
    me("policewomen", "police officer(s)"),
    me("repairman", "technician(s)"),
    me("repairmen", "technician(s)"),
    me("repairwoman", "technician(s)"),
    me("repairwomen", "technician(s)"),
    me("salesman", "salesperson or sales people"),
    me("salesmen", "salesperson or sales people"),
    me("saleswoman", "salesperson or sales people"),
    me("saleswomen", "salesperson or sales people"),
    me("serviceman", "soldier(s)"),
    me("servicemen", "soldier(s)"),
    me("servicewoman", "soldier(s)"),
    me("servicewomen", "soldier(s)"),
    me("steward", "flight attendant"),
    me("stewardess", "flight attendant"),
    me("tribesman", "tribe member(s)"),
    me("tribesmen", "tribe member(s)"),
    me("tribeswoman", "tribe member(s)"),
    me("tribeswomen", "tribe member(s)"),
    me("waitress", "waiter"),
    me("woman doctor", "doctor"),
    me("woman scientist", "scientist(s)"),
    me("woman scientists", "scientist(s)"),
    me("workman", "worker(s)"),
    me("workmen", "worker(s)"),
    me("workwoman", "worker(s)"),
    me("workwomen", "worker(s)"),
];

/// Shared with the Google pack (same 36-pair list; SPEC §11.5).
pub fn gender_bias_map() -> &'static [MapEntry] {
    GENDER_BIAS
}

fn ms_gender_bias(ctx: &Ctx, em: &mut Emitter) {
    run_map(
        ctx,
        em,
        "ms-gender-bias",
        Family::Inclusive,
        Severity::Warn,
        "microsoft-gender-bias",
        "gendered term",
    );
}

fn microsoft_general_url(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\bURLs?\b").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "microsoft-general-url",
            FAM,
            Severity::Advisory,
            off,
            len,
            "for a general audience, use 'address'",
        );
    });
}

fn microsoft_heading_acronyms(ctx: &Ctx, em: &mut Emitter) {
    let re = regex::Regex::new(r"\b[A-Z]{2,4}\b").unwrap();
    for h in &ctx.headings {
        if let Some(m) = re.find(&h.text) {
            em.emit(
                ctx,
                "microsoft-heading-acronyms",
                FAM,
                Severity::Advisory,
                h.start + m.start(),
                m.len(),
                "avoid acronyms in headings",
            );
        }
    }
}

fn microsoft_heading_colons(ctx: &Ctx, em: &mut Emitter) {
    let re = regex::Regex::new(r": [a-z]").unwrap();
    for h in &ctx.headings {
        if let Some(m) = re.find(&h.text) {
            em.emit(
                ctx,
                "microsoft-heading-colons",
                FAM,
                Severity::Advisory,
                h.start + m.start(),
                m.len(),
                "capitalize the first word after a colon in a heading",
            );
        }
    }
}

/// Shared non-adverb -ly exception set (also used by google-ly-hyphen).
pub const LY_EXCEPTIONS: &[&str] = &[
    "family",
    "early",
    "only",
    "supply",
    "apply",
    "reply",
    "assembly",
    "friendly",
    "daily",
    "weekly",
    "monthly",
    "yearly",
    "hourly",
    "ally",
    "holy",
    "ugly",
    "lovely",
    "lonely",
    "lively",
    "costly",
    "deadly",
    "silly",
    "jelly",
    "belly",
    "italy",
    "curly",
    "burly",
    "surly",
    "wobbly",
    "bubbly",
    "gnarly",
    "melancholy",
    "anomaly",
    "monopoly",
    "panoply",
    "wholly",
    "homely",
    "timely",
    "orderly",
    "elderly",
    "likely",
    "unlikely",
];

pub fn adverb_hyphen(ctx: &Ctx, em: &mut Emitter, id: &str) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"(?i)\b([a-z]+ly)-([a-z]+)").unwrap());
    helpers::scan(ctx, re, |off, len, m| {
        let word = m.split('-').next().unwrap_or("").to_lowercase();
        if !LY_EXCEPTIONS.contains(&word.as_str()) {
            em.emit(
                ctx,
                id,
                FAM,
                Severity::Advisory,
                off,
                len,
                "an -ly adverb needs no hyphen",
            );
        }
    });
}

fn ms_adverb_hyphen(ctx: &Ctx, em: &mut Emitter) {
    adverb_hyphen(ctx, em, "ms-adverb-hyphen");
}

fn ms_negative_number_endash(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| fancy_regex::Regex::new(r"(?<=\s)-\d+").unwrap());
    helpers::scan_fancy(ctx, re, |off, len, _| {
        if !ctx.is_table_line(off) {
            em.emit(
                ctx,
                "ms-negative-number-endash",
                FAM,
                Severity::Advisory,
                off,
                len,
                "form negative numbers with an en dash, not a hyphen",
            );
        }
    });
}

fn ms_ordinal_ly(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"(?i)\b(firstly|secondly|thirdly)\b").unwrap());
    helpers::scan(ctx, re, |off, len, m| {
        em.emit(
            ctx,
            "ms-ordinal-ly",
            FAM,
            Severity::Advisory,
            off,
            len,
            format!("'{m}' — use '{}'", m.to_lowercase().trim_end_matches("ly")),
        );
    });
}

const SPELLED_NUMBERS: &str = "zero|one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|twenty|thirty|forty|fifty|sixty|seventy|eighty|ninety|hundred";

fn ms_percentages(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(&format!(r"(?i)\b(?:{SPELLED_NUMBERS}) percent\b")).unwrap()
    });
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "ms-percentages",
            FAM,
            Severity::Advisory,
            off,
            len,
            "use a numeral with 'percent'",
        );
    });
}

fn ms_plurals_parenthetical(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\b[a-zA-Z]+\((e?s)\)").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "ms-plurals-parenthetical",
            FAM,
            Severity::Advisory,
            off,
            len,
            "use the plural, not '(s)'",
        );
    });
}

fn microsoft_quotes_punctuation(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\u{201C}[^\u{201D}\n]+\u{201D}[.,]").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "microsoft-quotes-punctuation",
            FAM,
            Severity::Warn,
            off,
            len,
            "put the period or comma inside the quotation marks",
        );
    });
}

fn microsoft_range_time(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"(?i)\b\d{1,2}(?::\d{2})? ?[AP]M ?[–—-] ?\d{1,2}(?::\d{2})? ?[AP]M\b")
            .unwrap()
    });
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "microsoft-range-time",
            FAM,
            Severity::Advisory,
            off,
            len,
            "use 'to' in time ranges, not a dash",
        );
    });
}

fn is_entity_semi(masked: &str, off: usize) -> bool {
    let start = off.saturating_sub(12);
    let mut s = start;
    while !masked.is_char_boundary(s) {
        s += 1;
    }
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"&([a-zA-Z]{1,10}|#\d{1,7}|#x[0-9a-fA-F]{1,6})$").unwrap()
    });
    re.is_match(&masked[s..off])
}

fn microsoft_semicolon(ctx: &Ctx, em: &mut Emitter) {
    for (off, c) in ctx.masked.char_indices() {
        if c == ';' && !ctx.is_table_line(off) && !is_entity_semi(&ctx.masked, off) {
            em.emit(
                ctx,
                "microsoft-semicolon",
                FAM,
                Severity::Advisory,
                off,
                1,
                "Try to simplify this sentence.",
            );
        }
    }
}

fn ms_suspended_hyphen(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| regex::Regex::new(r"(?i)\b[a-z]+- (?:and|or) [a-z]+-[a-z]+").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "ms-suspended-hyphen",
            FAM,
            Severity::Advisory,
            off,
            len,
            "avoid suspended hyphenation — repeat the full word",
        );
    });
}

pub const TERM_SWAPS: &[MapEntry] = &[
    me("adaptor", "adapter"),
    me("administrate", "administer"),
    me("alphanumerical", "alphanumeric"),
    me("an url", "a URL"),
    me("anti-aliasing", "antialiasing"),
    me("anti-malware", "antimalware"),
    me("anti-spyware", "antispyware"),
    me("anti-virus", "antivirus"),
    me("appendixes", "appendices"),
    me("afterwards", "afterward"),
    me("keypress", "keystroke"),
    me("conversation-as-a-platform", "conversation as a platform"),
    me("audio-book", "audiobook"),
    me("audio book", "audiobook"),
    me("back-light", "backlight"),
    me("smart phone", "phone"),
    me("smartphone", "phone"),
    me("mobile phone", "phone"),
    me("24/7", "every day"),
    me("web robot", "bot"),
    me("internet bot", "bot"),
    me("machine language", "assembly language"),
    me("virtual assistant", "personal digital assistant"),
    me(
        "intelligent personal assistant",
        "personal digital assistant",
    ),
    me("chat bot", "chatbot"),
    me("chat bots", "chatbot"),
    me("chatbots", "chatbot"),
];

fn ms_term_swaps(ctx: &Ctx, em: &mut Emitter) {
    run_map(
        ctx,
        em,
        "ms-term-swaps",
        FAM,
        Severity::Advisory,
        "microsoft-term-swap",
        "term swap",
    );
}

fn ms_url_of(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\bURL for\b").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "ms-url-of",
            FAM,
            Severity::Advisory,
            off,
            len,
            "say 'URL of', not 'URL for' — use 'URL of'",
        );
    });
}

fn ms_units_spelled_number(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(&format!(
            r"(?i)\b(?:{SPELLED_NUMBERS}|thousand|million) (?:(?:centi|milli)?meters|(?:kilo)?grams|(?:kilo)?meters|(?:mega)?pixels|cm|inches|lb|miles|pounds)\b"
        ))
        .unwrap()
    });
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "ms-units-spelled-number",
            FAM,
            Severity::Warn,
            off,
            len,
            "use a numeral with a measurement unit",
        );
    });
}

pub const AZ_WORDLIST: &[&str] = &[
    "above",
    "accessible",
    "actionable",
    "against",
    "alarm",
    "alert",
    "alias",
    "allow",
    "allows",
    "and/or",
    "as well as",
    "assure",
    "author",
    "avg",
    "beta",
    "ensure",
    "he",
    "insure",
    "sample",
    "she",
];

fn ms_vocab_az_wordlist(ctx: &Ctx, em: &mut Emitter) {
    let re = ctx.lists.phrase_regex("microsoft-a-z");
    let mut hits: Vec<(usize, usize, String)> = Vec::new();
    helpers::scan_fancy(ctx, &re, |off, len, m| hits.push((off, len, m.to_string())));
    if hits.len() >= 2 {
        for (off, len, m) in hits {
            em.emit(
                ctx,
                "ms-vocab-az-wordlist",
                FAM,
                Severity::Advisory,
                off,
                len,
                format!("'{m}' — verify against the Microsoft A–Z word list"),
            );
        }
    }
}

pub const MS_WORDINESS: &[MapEntry] = &[
    me("sufficient number of", "enough"),
    me("sufficient number", "enough"),
    me("take away", "remove"),
    me("eliminate", "remove"),
    me("as a means to", "to"),
    me("as a means of", "to"),
    me("in an effort to", "to"),
    me("inform", "tell"),
    me("let me know", "tell"),
    me("previous to", "before"),
    me("prior to", "before"),
    me("utilize", "use"),
    me("make use of", "use"),
    me("a large majority of", "most"),
    me("a majority of", "most"),
    me("a large number of", "many"),
    me("a number of", "many"),
    me("a myriad of", "myriad"),
    me("adversely impact", "hurt"),
    me("all across", "across"),
    me("all of a sudden", "suddenly"),
    me("all of these", "these"),
    me("all of", "all"),
    me("all-time record", "record"),
    me("almost all", "most"),
    me("almost never", "seldom"),
    me("along the lines of", "similar to"),
    me("an adequate number of", "enough"),
    me("an appreciable number of", "many"),
    me("an estimated", "about"),
    me("any and all", "all"),
    me("are in agreement", "agree"),
    me("as a matter of fact", "in fact"),
    me("as a result of", "because of"),
    me("as of yet", "yet"),
    me("as per", "per"),
    me("at a later date", "later"),
    me("at all times", "always"),
    me("at the present time", "now"),
    me("at this point in time", "at this point"),
    me("based in large part on", "based on"),
    me("based on the fact that", "because"),
    me("basic necessity", "necessity"),
    me("because of the fact that", "because"),
    me("came to a realization", "realized"),
    me("came to an abrupt end", "ended abruptly"),
    me("carry out an evaluation of", "evaluate"),
    me("close down", "close"),
    me("closed down", "closed"),
    me("complete stranger", "stranger"),
    me("completely separate", "separate"),
    me("concerning the matter of", "regarding"),
    me("conduct a review of", "review"),
    me("conduct an investigation", "investigate"),
    me("conduct experiments", "experiment"),
    me("continue on", "continue"),
    me("despite the fact that", "although"),
    me("disappear from sight", "disappear"),
    me("doomed to fail", "doomed"),
    me("drag and drop", "drag"),
    me("drag-and-drop", "drag"),
    me("due to the fact that", "because"),
    me("during the period of", "during"),
    me("during the time that", "while"),
    me("emergency situation", "emergency"),
    me("establish connectivity", "connect"),
    me("except when", "unless"),
    me("excessive number", "too many"),
    me("extend an invitation", "invite"),
    me("fall down", "fall"),
    me("fell down", "fell"),
    me("for the duration of", "during"),
    me("gather together", "gather"),
    me("has the ability to", "can"),
    me("has the capacity to", "can"),
    me("has the opportunity to", "could"),
    me("hold a meeting", "meet"),
    me("if this is not the case", "if not"),
    me("in a careful manner", "carefully"),
    me("in a thoughtful manner", "thoughtfully"),
    me("in a timely manner", "timely"),
    me("in addition", "also"),
    me("in between", "between"),
    me("in lieu of", "instead of"),
    me("in many cases", "often"),
    me("in most cases", "usually"),
    me("in some cases", "sometimes"),
    me("in spite of the fact that", "although"),
    me("in spite of", "despite"),
    me("in the very near future", "soon"),
    me("in the near future", "soon"),
    me("in the event that", "if"),
    me("in the neighborhood of", "roughly"),
    me("in the vicinity of", "close to"),
    me("it would appear that", "apparently"),
    me("lift up", "lift"),
    me("made reference to", "referred to"),
    me("make reference to", "refer to"),
    me("mix together", "mix"),
    me("none at all", "none"),
    me("not in a position to", "unable"),
    me("not possible", "impossible"),
    me("of major importance", "important"),
    me("perform an assessment of", "assess"),
    me("pertaining to", "about"),
    me("place an order", "order"),
    me("plays a key role in", "is essential to"),
    me("present time", "now"),
    me("readily apparent", "apparent"),
    me("some of the", "some"),
    me("span across", "span"),
    me("subsequent to", "after"),
    me("successfully complete", "complete"),
    me("take action", "act"),
    me("take into account", "consider"),
    me("the question as to whether", "whether"),
    me("there is no doubt but that", "doubtless"),
    me("this day and age", "this age"),
    me("this is a subject that", "this subject"),
    me("time frame", "time"),
    me("time period", "time"),
    me("under the provisions of", "under"),
    me("until such time as", "until"),
    me("used for fuel purposes", "used for fuel"),
    me("whether or not", "whether"),
    me("with regard to", "regarding"),
    me("with the exception of", "except for"),
];

fn ms_wordiness(ctx: &Ctx, em: &mut Emitter) {
    run_map(
        ctx,
        em,
        "ms-wordiness",
        FAM,
        Severity::Advisory,
        "microsoft-wordiness",
        "wordy phrase",
    );
}

/// The Vale Microsoft adverb list (SPEC §11.2 — family B, pack microsoft).
pub const MS_ADVERBS: &[&str] = &[
    "abnormally",
    "absentmindedly",
    "accidentally",
    "adventurously",
    "anxiously",
    "arrogantly",
    "awkwardly",
    "bashfully",
    "beautifully",
    "bitterly",
    "bleakly",
    "blindly",
    "blissfully",
    "boastfully",
    "boldly",
    "bravely",
    "briefly",
    "brightly",
    "briskly",
    "broadly",
    "busily",
    "calmly",
    "carefully",
    "carelessly",
    "cautiously",
    "cheerfully",
    "cleverly",
    "closely",
    "coaxingly",
    "colorfully",
    "continually",
    "coolly",
    "courageously",
    "crossly",
    "cruelly",
    "curiously",
    "daintily",
    "dearly",
    "deceivingly",
    "deeply",
    "defiantly",
    "deliberately",
    "delightfully",
    "diligently",
    "dimly",
    "doubtfully",
    "dreamily",
    "easily",
    "effectively",
    "elegantly",
    "energetically",
    "enthusiastically",
    "excitedly",
    "extremely",
    "fairly",
    "faithfully",
    "famously",
    "ferociously",
    "fervently",
    "fiercely",
    "fondly",
    "foolishly",
    "fortunately",
    "frankly",
    "frantically",
    "freely",
    "frenetically",
    "frightfully",
    "furiously",
    "generally",
    "generously",
    "gently",
    "gladly",
    "gleefully",
    "gracefully",
    "gratefully",
    "greatly",
    "greedily",
    "happily",
    "hastily",
    "healthily",
    "heavily",
    "helplessly",
    "honestly",
    "hopelessly",
    "hungrily",
    "innocently",
    "inquisitively",
    "intensely",
    "intently",
    "interestingly",
    "inwardly",
    "irritably",
    "jaggedly",
    "jealously",
    "jovially",
    "joyfully",
    "joyously",
    "jubilantly",
    "judgmentally",
    "justly",
    "keenly",
    "kiddingly",
    "kindheartedly",
    "knavishly",
    "knowingly",
    "knowledgeably",
    "lazily",
    "lightly",
    "limply",
    "lively",
    "loftily",
    "longingly",
    "loosely",
    "loudly",
    "lovingly",
    "loyally",
    "madly",
    "majestically",
    "meaningfully",
    "mechanically",
    "merrily",
    "miserably",
    "mockingly",
    "mortally",
    "mysteriously",
    "naturally",
    "nearly",
    "neatly",
    "nervously",
    "nicely",
    "noisily",
    "obediently",
    "obnoxiously",
    "oddly",
    "offensively",
    "optimistically",
    "overconfidently",
    "painfully",
    "partially",
    "patiently",
    "perfectly",
    "playfully",
    "politely",
    "poorly",
    "positively",
    "potentially",
    "powerfully",
    "promptly",
    "properly",
    "punctually",
    "quaintly",
    "queasily",
    "queerly",
    "questionably",
    "quickly",
    "quietly",
    "quirkily",
    "quite",
    "quizzically",
    "randomly",
    "rapidly",
    "rarely",
    "readily",
    "really",
    "reassuringly",
    "recklessly",
    "regularly",
    "reluctantly",
    "repeatedly",
    "reproachfully",
    "restfully",
    "righteously",
    "rightfully",
    "rigidly",
    "roughly",
    "rudely",
    "safely",
    "scarcely",
    "scarily",
    "searchingly",
    "sedately",
    "seemingly",
    "selfishly",
    "separately",
    "seriously",
    "shakily",
    "sharply",
    "sheepishly",
    "shrilly",
    "shyly",
    "silently",
    "sleepily",
    "slowly",
    "smoothly",
    "softly",
    "solemnly",
    "solidly",
    "speedily",
    "stealthily",
    "sternly",
    "strictly",
    "suddenly",
    "supposedly",
    "surprisingly",
    "suspiciously",
    "sweetly",
    "swiftly",
    "sympathetically",
    "tenderly",
    "tensely",
    "terribly",
    "thankfully",
    "thoroughly",
    "thoughtfully",
    "tightly",
    "tremendously",
    "triumphantly",
    "truthfully",
    "ultimately",
    "unabashedly",
    "unaccountably",
    "unbearably",
    "unethically",
    "unexpectedly",
    "unfortunately",
    "unimpressively",
    "unnaturally",
    "unnecessarily",
    "urgently",
    "usefully",
    "uselessly",
    "utterly",
    "vacantly",
    "vaguely",
    "vainly",
    "valiantly",
    "vastly",
    "verbally",
    "very",
    "viciously",
    "victoriously",
    "violently",
    "vivaciously",
    "voluntarily",
    "warmly",
    "weakly",
    "wearily",
    "wetly",
    "wholly",
    "wildly",
    "willfully",
    "wisely",
    "woefully",
    "wonderfully",
    "worriedly",
    "yawningly",
    "yearningly",
    "yieldingly",
    "youthfully",
    "zealously",
    "zestfully",
    "zestily",
];

fn microsoft_adverbs(ctx: &Ctx, em: &mut Emitter) {
    let re = ctx.lists.word_regex("microsoft-adverbs");
    let mut hits: Vec<(usize, usize)> = Vec::new();
    helpers::scan(ctx, &re, |off, len, _| hits.push((off, len)));
    if hits.len() >= 2 {
        for (off, len) in hits {
            em.emit(
                ctx,
                "microsoft-adverbs",
                Family::Clarity,
                Severity::Advisory,
                off,
                len,
                "Remove it if it's not important to the meaning",
            );
        }
    }
}

#[cfg(test)]
mod vale_tests {
    use crate::detector::runner::{detect_text, test_settings};

    fn ids(text: &str) -> Vec<String> {
        detect_text("t.md", text, &test_settings("microsoft"))
            .findings
            .into_iter()
            .map(|f| f.rule_id)
            .collect()
    }

    #[test]
    fn ampm_and_dates() {
        assert!(ids("Meet at 12am sharp.\n").contains(&"microsoft-ampm".to_string()));
        assert!(!ids("Meet at 12 AM sharp.\n").contains(&"microsoft-ampm".to_string()));
        assert!(ids("Released 31 July 2016 worldwide.\n").contains(&"ms-date-format".to_string()));
        assert!(ids("Released July first that year.\n").contains(&"ms-date-numbers".to_string()));
        assert!(ids("Due 07/31/2016 at noon.\n").contains(&"ms-date-order".to_string()));
        assert!(!ids("Released July 31, 2016 worldwide.\n")
            .iter()
            .any(|i| i.starts_with("ms-date")));
    }

    #[test]
    fn accessibility_and_gender() {
        assert!(ids("He suffers from epilepsy.\n").contains(&"microsoft-accessibility".to_string()));
        assert!(
            ids("Ask the chairman or the policeman about the mailman.\n")
                .iter()
                .any(|i| i == "ms-gender-bias")
        );
        assert!(ids("Whether he/she agrees is open.\n").contains(&"ms-gender-slash".to_string()));
        assert!(!ids("Ask the chair or the police officer.\n")
            .iter()
            .any(|i| i == "ms-gender-bias"));
    }

    #[test]
    fn wordiness_and_term_swaps() {
        assert!(ids("We did this in an effort to help.\n").contains(&"ms-wordiness".to_string()));
        assert!(ids("Plug in the adaptor now.\n").contains(&"ms-term-swaps".to_string()));
        assert!(!ids("We did this to help.\n")
            .iter()
            .any(|i| i == "ms-wordiness" || i == "ms-term-swaps"));
    }

    #[test]
    fn adverbs_need_two() {
        assert!(
            ids("It runs quickly and fails suddenly.\n").contains(&"microsoft-adverbs".to_string())
        );
        assert!(!ids("It runs quickly.\n").contains(&"microsoft-adverbs".to_string()));
    }

    #[test]
    fn misc_ms_rules() {
        assert!(ids("Set auto-update in settings.\n")
            .contains(&"microsoft-auto-hyphenation".to_string()));
        assert!(ids("It is how is done; that is fine.\n")
            .contains(&"microsoft-contractions".to_string()));
        assert!(ids("Wait... loading.\n").contains(&"ms-ellipses".to_string()));
        assert!(ids("I think this suits my needs.\n").contains(&"ms-first-person".to_string()));
        assert!(ids("Use tags, e.g. canonical ones.\n").contains(&"ms-foreign-abbrev".to_string()));
        assert!(ids("Open the URL in a browser.\n").contains(&"microsoft-general-url".to_string()));
        assert!(ids("## About HTTP servers\n").contains(&"microsoft-heading-acronyms".to_string()));
        assert!(ids("## Setup: the basics\n").contains(&"microsoft-heading-colons".to_string()));
        assert!(ids("A newly-created file appears.\n").contains(&"ms-adverb-hyphen".to_string()));
        assert!(!ids("A family-owned shop.\n").contains(&"ms-adverb-hyphen".to_string()));
        assert!(
            ids("The delta was -5 degrees.\n").contains(&"ms-negative-number-endash".to_string())
        );
        assert!(ids("Firstly, install it.\n").contains(&"ms-ordinal-ly".to_string()));
        assert!(ids("About twenty percent failed.\n").contains(&"ms-percentages".to_string()));
        assert!(
            ids("Select the file(s) to copy.\n").contains(&"ms-plurals-parenthetical".to_string())
        );
        assert!(ids("Read \u{201C}the guide\u{201D}. Then start.\n")
            .contains(&"microsoft-quotes-punctuation".to_string()));
        assert!(ids("Open 9 AM - 5 PM daily.\n").contains(&"microsoft-range-time".to_string()));
        assert!(ids("First clause; second clause.\n").contains(&"microsoft-semicolon".to_string()));
        assert!(ids("Run pre- and post-processing steps.\n")
            .contains(&"ms-suspended-hyphen".to_string()));
        assert!(ids("Copy the URL for the page.\n").contains(&"ms-url-of".to_string()));
        assert!(
            ids("It weighs five pounds total.\n").contains(&"ms-units-spelled-number".to_string())
        );
        assert!(
            ids("The alarm and the alert fired.\n").contains(&"ms-vocab-az-wordlist".to_string())
        );
        assert!(ids("The user can abortion this and so on.\n")
            .contains(&"microsoft-avoid-words".to_string()));
    }
}
