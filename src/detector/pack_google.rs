//! Google developer documentation style pack rules (SPEC §11.5).

use super::ctx::Ctx;
use super::helpers::{self, me, MapEntry};
use super::{Emitter, Family, Rule, Severity};
use std::sync::OnceLock;

const FAM: Family = Family::Style;
const PACK: &str = "google";

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
        r("no-gerund-heading", no_gerund_heading),
        r("no-link-in-heading", no_link_in_heading),
        r("latinism-abbreviation", latinism_abbreviation),
        r("minimizing-words", minimizing_words),
        r("no-abbreviation-as-verb", no_abbreviation_as_verb),
        r("no-periods-in-acronyms", no_periods_in_acronyms),
        r("no-exclamation", no_exclamation),
        r("american-spelling", american_spelling),
        r("no-preannounce", no_preannounce),
        r("no-directional", no_directional),
        r("google-ampm", google_ampm),
        r("google-contractions", google_contractions),
        r("google-date-format", google_date_format),
        r("google-ellipses", google_ellipses),
        r("google-dash-spacing", google_dash_spacing),
        r("google-first-person", google_first_person),
        r(
            "google-gender-neutral-pronoun",
            google_gender_neutral_pronoun,
        ),
        r("google-gender-bias", google_gender_bias),
        r("google-ly-hyphen", google_ly_hyphen),
        r("google-optional-plurals", google_optional_plurals),
        r("google-ordinal", google_ordinal),
        r("google-quote-punctuation", google_quote_punctuation),
        r("google-number-range-words", google_number_range_words),
        r("google-semicolons", google_semicolons),
        r("google-slang", google_slang),
        r("google-units-nbsp", google_units_nbsp),
        r("avoid-first-person-plural", avoid_first_person_plural),
        r("avoid-will-future-tense", avoid_will_future_tense),
        r("google-word-list", google_word_list),
    ]
}

fn no_gerund_heading(ctx: &Ctx, em: &mut Emitter) {
    for h in &ctx.headings {
        let first = h
            .text
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_matches(|c: char| !c.is_ascii_alphabetic());
        if first.len() > 4 && first.to_ascii_lowercase().ends_with("ing") {
            em.emit(
                ctx,
                "no-gerund-heading",
                FAM,
                Severity::Warn,
                h.start,
                h.text.len(),
                "Google style avoids gerund headings",
            );
        }
    }
}

fn no_link_in_heading(ctx: &Ctx, em: &mut Emitter) {
    for h in &ctx.headings {
        if h.text.contains("](") || h.text.contains("][") {
            em.emit(
                ctx,
                "no-link-in-heading",
                FAM,
                Severity::Warn,
                h.start,
                h.text.len(),
                "Google style avoids links in headings",
            );
        }
    }
}

const LATINISMS: &[MapEntry] = &[
    me("e.g.", "for example"),
    me("i.e.", "that is"),
    me("etc.", "and so on"),
    me("etc", "and so on"),
    me("via", "through"),
    me("vs.", "versus"),
    me("vs", "versus"),
];

fn latinism_abbreviation(ctx: &Ctx, em: &mut Emitter) {
    map_rule(
        ctx,
        em,
        "latinism-abbreviation",
        Severity::Warn,
        LATINISMS,
        "Google style",
    );
}

fn minimizing_words(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        helpers::word_list(&[
            "easy",
            "easily",
            "simple",
            "simply",
            "just",
            "quick",
            "quickly",
            "obviously",
            "of course",
            "merely",
            "trivial",
        ])
    });
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "minimizing-words",
            FAM,
            Severity::Warn,
            off,
            len,
            "minimizing word — it may not be easy for everyone",
        );
    });
}

fn no_abbreviation_as_verb(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::RegexBuilder::new(r"\b(ssh|rsync|scp|ftp|chmod|grep)\s+(into|to)\b")
            .case_insensitive(true)
            .build()
            .unwrap()
    });
    helpers::scan(ctx, re, |off, len, _| {
        let before = &ctx.masked[..off];
        let prefix = before
            .split_whitespace()
            .last()
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(prefix.as_str(), "use" | "using") {
            return;
        }
        em.emit(
            ctx,
            "no-abbreviation-as-verb",
            FAM,
            Severity::Advisory,
            off,
            len,
            "use the tool name as a noun, as in 'use SSH to ...'",
        );
    });
}

fn no_periods_in_acronyms(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\b(?:[A-Z]\.){2,}").unwrap());
    helpers::scan(ctx, re, |off, len, m| {
        let lower = m.to_ascii_lowercase();
        if matches!(lower.as_str(), "e.g." | "i.e.") {
            return;
        }
        em.emit(
            ctx,
            "no-periods-in-acronyms",
            FAM,
            Severity::Advisory,
            off,
            len,
            "Google style omits periods in acronyms",
        );
    });
}

fn no_exclamation(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\w!").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        let bang = off + len - 1;
        let after = ctx.masked.as_bytes().get(bang + 1).copied();
        if after == Some(b'=') {
            return;
        }
        em.emit(
            ctx,
            "no-exclamation",
            FAM,
            Severity::Warn,
            bang,
            1,
            "Google style avoids exclamation points",
        );
    });
}

const AMERICAN: &[MapEntry] = &[
    me("colour", "color"),
    me("colours", "colors"),
    me("favour", "favor"),
    me("behaviour", "behavior"),
    me("flavour", "flavor"),
    me("honour", "honor"),
    me("labour", "labor"),
    me("neighbour", "neighbor"),
    me("organise", "organize"),
    me("organised", "organized"),
    me("recognise", "recognize"),
    me("analyse", "analyze"),
    me("catalogue", "catalog"),
    me("dialogue", "dialog"),
    me("centre", "center"),
    me("metre", "meter"),
    me("licence", "license"),
    me("defence", "defense"),
    me("grey", "gray"),
    me("cancelled", "canceled"),
    me("travelling", "traveling"),
    me("modelling", "modeling"),
];

fn american_spelling(ctx: &Ctx, em: &mut Emitter) {
    map_rule(
        ctx,
        em,
        "american-spelling",
        Severity::Warn,
        AMERICAN,
        "Google style",
    );
}

fn no_preannounce(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        helpers::phrase_list(&[
            "currently",
            "presently",
            "at this time",
            "latest",
            "newest",
            "brand-new",
            "soon",
            "in the near future",
            "upcoming",
        ])
    });
    helpers::scan_fancy(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "no-preannounce",
            FAM,
            Severity::Advisory,
            off,
            len,
            "docs outlive time-sensitive wording",
        );
    });
}

const DIRECTIONAL: &[MapEntry] = &[me("above", "preceding"), me("below", "following")];

fn no_directional(ctx: &Ctx, em: &mut Emitter) {
    map_rule(
        ctx,
        em,
        "no-directional",
        Severity::Advisory,
        DIRECTIONAL,
        "directional reference",
    );
}

fn map_rule(
    ctx: &Ctx,
    em: &mut Emitter,
    id: &str,
    severity: Severity,
    map: &[MapEntry],
    label: &str,
) {
    let re = helpers::map_regex(map);
    helpers::scan_fancy(ctx, &re, |off, len, m| {
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

#[cfg(test)]
mod tests {
    use super::super::runner::{detect_text, test_settings};

    fn ids(text: &str) -> Vec<String> {
        detect_text("x.md", text, &test_settings("google"))
            .findings
            .into_iter()
            .map(|f| f.rule_id)
            .collect()
    }

    #[test]
    fn google_native_pack_rules_fire() {
        let got = ids("# Installing [CLI](x)\n\nThis is easy! SSH to the host, e.g. via the U.S. node below. The newest colour docs are upcoming.");
        for id in [
            "no-gerund-heading",
            "no-link-in-heading",
            "minimizing-words",
            "no-exclamation",
            "no-abbreviation-as-verb",
            "latinism-abbreviation",
            "no-periods-in-acronyms",
            "no-directional",
            "american-spelling",
            "no-preannounce",
        ] {
            assert!(got.iter().any(|g| g == id), "{id}: {got:?}");
        }
    }
}

// ---- Vale-parity ports (SPEC §11.5) ----

fn run_map(
    ctx: &Ctx,
    em: &mut Emitter,
    id: &str,
    family: Family,
    sev: Severity,
    map: &[MapEntry],
    what: &str,
) {
    let re = helpers::map_regex(map);
    helpers::scan_fancy(ctx, &re, |off, len, m| {
        if let Some(to) = helpers::map_lookup(map, m) {
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

fn google_ampm(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    // Number joined to am/pm forms (no space).
    let re =
        RE.get_or_init(|| regex::Regex::new(r"(?i)\b\d{1,2}(?::\d{2})?[ap]\.?m\b\.?").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "google-ampm",
            FAM,
            Severity::Warn,
            off,
            len,
            "use 'AM'/'PM' preceded by a space",
        );
    });
}

fn google_contractions(ctx: &Ctx, em: &mut Emitter) {
    const MAP: &[MapEntry] = &[
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
    run_map(
        ctx,
        em,
        "google-contractions",
        FAM,
        Severity::Advisory,
        MAP,
        "prefer the contraction",
    );
}

fn google_date_format(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let months =
        "January|February|March|April|May|June|July|August|September|October|November|December";
    let re = RE.get_or_init(|| {
        regex::Regex::new(&format!(
            r"\b\d{{1,2}}\.\d{{1,2}}\.\d{{4}}\b|\b\d{{1,2}}/\d{{1,2}}/\d{{4}}\b|\b\d{{1,2}} (?:{months}),? \d{{4}}\b"
        ))
        .unwrap()
    });
    helpers::scan(ctx, re, |off, len, _| {
        if !ctx.is_table_line(off) {
            em.emit(
                ctx,
                "google-date-format",
                FAM,
                Severity::Advisory,
                off,
                len,
                "date format — use 'July 31, 2016'",
            );
        }
    });
}

fn google_ellipses(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\.\.\.").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "google-ellipses",
            FAM,
            Severity::Advisory,
            off,
            len,
            "avoid ellipses",
        );
    });
}

fn google_dash_spacing(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(" — | – ").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "google-dash-spacing",
            FAM,
            Severity::Advisory,
            off,
            len,
            "no space around a dash",
        );
    });
}

fn google_first_person(ctx: &Ctx, em: &mut Emitter) {
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
                "google-first-person",
                FAM,
                Severity::Warn,
                off,
                len,
                "address the reader — avoid first person",
            );
        }
    }
}

fn google_gender_neutral_pronoun(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"(?i)\b(he/she|s/he|\(s\)he)\b").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "google-gender-neutral-pronoun",
            Family::Inclusive,
            Severity::Warn,
            off,
            len,
            "gendered pronoun form — use 'they'",
        );
    });
}

fn google_gender_bias(ctx: &Ctx, em: &mut Emitter) {
    // Same 36-pair list as ms-gender-bias; mankind → "human kind or humanity".
    let mut map: Vec<MapEntry> = Vec::new();
    for e in super::pack_microsoft::gender_bias_map() {
        if e.from == "mankind" {
            map.push(me("mankind", "human kind or humanity"));
        } else {
            map.push(MapEntry {
                from: e.from,
                to: e.to,
            });
        }
    }
    run_map(
        ctx,
        em,
        "google-gender-bias",
        Family::Inclusive,
        Severity::Warn,
        &map,
        "gendered term",
    );
}

fn google_ly_hyphen(ctx: &Ctx, em: &mut Emitter) {
    super::pack_microsoft::adverb_hyphen(ctx, em, "google-ly-hyphen");
}

fn google_optional_plurals(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\b[a-zA-Z]+\(s\)").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "google-optional-plurals",
            FAM,
            Severity::Advisory,
            off,
            len,
            "rewrite as the plural or 'one or more'",
        );
    });
}

fn google_ordinal(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\b\d+(st|nd|rd|th)\b").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        if !ctx.is_table_line(off) {
            em.emit(
                ctx,
                "google-ordinal",
                FAM,
                Severity::Warn,
                off,
                len,
                "spell out ordinals",
            );
        }
    });
}

fn google_quote_punctuation(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r#""[^"\n]+"[.,?]"#).unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "google-quote-punctuation",
            FAM,
            Severity::Advisory,
            off,
            len,
            "put punctuation inside the quotation marks",
        );
    });
}

fn google_number_range_words(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| regex::Regex::new(r"(?i)\b(from|between)\s+\d+\s*-\s*\d+\b").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "google-number-range-words",
            FAM,
            Severity::Advisory,
            off,
            len,
            "drop the words around a numeric range",
        );
    });
}

fn google_semicolons(ctx: &Ctx, em: &mut Emitter) {
    for (off, c) in ctx.masked.char_indices() {
        if c == ';' && !ctx.is_table_line(off) {
            em.emit(
                ctx,
                "google-semicolons",
                FAM,
                Severity::Advisory,
                off,
                1,
                "use semicolons judiciously",
            );
        }
    }
}

fn google_slang(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| helpers::phrase_list(&["tl;dr", "ymmv", "rtfm", "imo", "fwiw"]));
    helpers::scan_fancy(ctx, re, |off, len, m| {
        em.emit(
            ctx,
            "google-slang",
            FAM,
            Severity::Warn,
            off,
            len,
            format!("'{m}' is slang — spell it out"),
        );
    });
}

fn google_units_nbsp(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    // Ambiguous single-letter units deliberately excluded (SPEC §11.5).
    let re =
        RE.get_or_init(|| regex::Regex::new(r"\b\d+(?:\.\d+)?(kB|MB|GB|TB|min|ns|ms)\b").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "google-units-nbsp",
            FAM,
            Severity::Advisory,
            off,
            len,
            "put a nonbreaking space between the number and the unit",
        );
    });
}

fn avoid_first_person_plural(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE
        .get_or_init(|| regex::Regex::new(r"(?i)\b(we|we've|we're|our|ours|us|let's)\b").unwrap());
    let mut hits: Vec<(usize, usize)> = Vec::new();
    helpers::scan(ctx, re, |off, len, _| hits.push((off, len)));
    if hits.len() >= 2 {
        for (off, len) in hits {
            em.emit(
                ctx,
                "avoid-first-person-plural",
                FAM,
                Severity::Advisory,
                off,
                len,
                "avoid first-person plural — address the reader",
            );
        }
    }
}

fn avoid_will_future_tense(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"(?i)\bwill\b").unwrap());
    let mut hits: Vec<(usize, usize)> = Vec::new();
    helpers::scan(ctx, re, |off, len, _| hits.push((off, len)));
    if hits.len() >= 2 {
        for (off, len) in hits {
            em.emit(
                ctx,
                "avoid-will-future-tense",
                FAM,
                Severity::Advisory,
                off,
                len,
                "prefer present tense",
            );
        }
    }
}

const GOOGLE_WORDS: &[MapEntry] = &[
    me("dev key", "API key"),
    me("developer key", "API key"),
    me("api console key", "API key"),
    me("cellphone", "phone"),
    me("cell phone", "phone"),
    me("smartphone", "phone"),
    me("smart phone", "phone"),
    me("dev console", "API console"),
    me("developer console", "API console"),
    me("apis console", "API console"),
    me("e-mail", "email"),
    me("filepath", "path"),
    me("file path", "path"),
    me("pathname", "path"),
    me("path name", "path"),
    me("oauth2", "OAuth 2.0"),
    me("wifi", "Wi-Fi"),
    me("google i-o", "Google I/O"),
    me("google io", "Google I/O"),
    me("tap and hold", "touch & hold"),
    me("long press", "touch & hold"),
    me("uncheck", "clear"),
    me("unselect", "clear"),
    me("account name", "username"),
    me("action bar", "app bar"),
    me("ajax", "AJAX"),
    me("authn", "authentication"),
    me("authz", "authorization"),
    me("autoupdate", "automatically update"),
    me("cellular data", "mobile data"),
    me("cellular network", "mobile network"),
    me("check box", "checkbox"),
    me("click on", "click"),
    me("container engine", "Kubernetes Engine"),
    me("content type", "media type"),
    me("curated roles", "predefined roles"),
    me("data are", "data is"),
    me("file name", "filename"),
    me("k8s", "Kubernetes"),
    me("network ip address", "internal IP address"),
    me("omnibox", "address bar"),
    me("sign into", "sign in to"),
    me("stylesheet", "style sheet"),
    me("tablename", "table name"),
    me("vs.", "versus"),
    me("world wide web", "web"),
    me("approx.", "approximately"),
];

fn google_word_list(ctx: &Ctx, em: &mut Emitter) {
    let re = helpers::map_regex(GOOGLE_WORDS);
    helpers::scan_fancy(ctx, &re, |off, len, m| {
        if let Some(to) = helpers::map_lookup(GOOGLE_WORDS, m) {
            // Case-only entries (ajax→AJAX) skip when already preferred.
            if m == to {
                return;
            }
            em.emit(
                ctx,
                "google-word-list",
                FAM,
                Severity::Advisory,
                off,
                len,
                format!("Google word list — use '{to}'"),
            );
        }
    });
}

#[cfg(test)]
mod vale_tests {
    use crate::detector::runner::{detect_text, test_settings};

    fn ids(text: &str) -> Vec<String> {
        detect_text("t.md", text, &test_settings("google"))
            .findings
            .into_iter()
            .map(|f| f.rule_id)
            .collect()
    }

    #[test]
    fn google_vale_rules_fire() {
        assert!(ids("Meet at 9am today.\n").contains(&"google-ampm".to_string()));
        assert!(ids("It is what it is because that is life.\n")
            .contains(&"google-contractions".to_string()));
        assert!(ids("Shipped 31 July 2016 to all.\n").contains(&"google-date-format".to_string()));
        assert!(ids("Loading... wait.\n").contains(&"google-ellipses".to_string()));
        assert!(ids("The result — a mess.\n").contains(&"google-dash-spacing".to_string()));
        assert!(ids("I think this suits my needs.\n").contains(&"google-first-person".to_string()));
        assert!(
            ids("Ask if he/she agrees.\n").contains(&"google-gender-neutral-pronoun".to_string())
        );
        assert!(ids("The chairman spoke to the policeman.\n")
            .iter()
            .any(|i| i == "google-gender-bias"));
        assert!(ids("A newly-created file appears.\n").contains(&"google-ly-hyphen".to_string()));
        assert!(ids("Delete the file(s) now.\n").contains(&"google-optional-plurals".to_string()));
        assert!(ids("The 3rd attempt worked.\n").contains(&"google-ordinal".to_string()));
        assert!(ids("Read \"the guide\". Then start.\n")
            .contains(&"google-quote-punctuation".to_string()));
        assert!(ids("Pick from 1-10 options.\n").contains(&"google-number-range-words".to_string()));
        assert!(ids("One clause; another clause.\n").contains(&"google-semicolons".to_string()));
        assert!(ids("tl;dr it works.\n").contains(&"google-slang".to_string()));
        assert!(ids("Allocate 512MB for the heap.\n").contains(&"google-units-nbsp".to_string()));
        assert!(ids("We think we know best.\n").contains(&"avoid-first-person-plural".to_string()));
        assert!(
            ids("It will run and will pass.\n").contains(&"avoid-will-future-tense".to_string())
        );
        assert!(ids("Set the file name in the dev console.\n")
            .contains(&"google-word-list".to_string()));
    }

    #[test]
    fn google_vale_rules_stay_silent_on_good() {
        let good = "Meet at 9 AM. It's ready. Shipped July 31, 2016. Click Run. They agree. \
The third attempt worked. Read \"the guide.\" Pick 1-10. It works. Allocate 512 MB. \
You know best. It runs. Set the filename in the API console.\n";
        let got = ids(good);
        for id in [
            "google-ampm",
            "google-contractions",
            "google-date-format",
            "google-ellipses",
            "google-dash-spacing",
            "google-first-person",
            "google-gender-neutral-pronoun",
            "google-gender-bias",
            "google-ly-hyphen",
            "google-optional-plurals",
            "google-ordinal",
            "google-quote-punctuation",
            "google-number-range-words",
            "google-semicolons",
            "google-slang",
            "google-units-nbsp",
            "avoid-first-person-plural",
            "avoid-will-future-tense",
            "google-word-list",
        ] {
            assert!(
                !got.contains(&id.to_string()),
                "{id} fired on good text: {got:?}"
            );
        }
    }
}
