//! AP style pack rules (SPEC §11.6).

use super::ctx::Ctx;
use super::helpers::{self, me, MapEntry};
use super::{Emitter, Family, Rule, Severity};
use std::sync::OnceLock;

const FAM: Family = Family::Style;
const PACK: &str = "ap";

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
        r("ap-serial-comma", serial_comma),
        r("ap-number-style", number_style),
        r("ap-percent", percent),
        r("ap-time-format", time_format),
        r("ap-dollar-style", dollar_style),
        r("ap-over-quantity", over_quantity),
        r("ap-toward", toward),
        r("ap-ampersand", ampersand),
    ]
}

fn serial_comma(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"\b[A-Za-z]+,\s+[A-Za-z]+,\s+(and|or)\s+[A-Za-z]+\b").unwrap()
    });
    helpers::scan(ctx, re, |off, len, _| {
        let span = &ctx.masked[off..off + len];
        if let Some(pos) = span.rfind(',') {
            em.emit(
                ctx,
                "ap-serial-comma",
                FAM,
                Severity::Advisory,
                off + pos,
                1,
                "AP style usually omits the serial comma",
            );
        }
    });
}

fn number_style(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| fancy_regex::Regex::new(r"(?<![\w.$%/-])([0-9])(?![\w.$%/-])").unwrap());
    helpers::scan_fancy(ctx, re, |off, len, _| {
        if !ctx.is_table_line(off) {
            em.emit(
                ctx,
                "ap-number-style",
                FAM,
                Severity::Advisory,
                off,
                len,
                "AP style spells out whole numbers zero through nine",
            );
        }
    });
}

fn percent(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\b\d+(?:\.\d+)?%").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        if !ctx.is_table_line(off) {
            em.emit(
                ctx,
                "ap-percent",
                FAM,
                Severity::Advisory,
                off,
                len,
                "AP style uses 'percent' instead of %",
            );
        }
    });
}

fn time_format(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::RegexBuilder::new(r"\b\d{1,2}(?::\d{2})?\s*(AM|PM)\b")
            .case_insensitive(false)
            .build()
            .unwrap()
    });
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "ap-time-format",
            FAM,
            Severity::Advisory,
            off,
            len,
            "AP style uses lowercase a.m. or p.m.",
        );
    });
}

fn dollar_style(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::RegexBuilder::new(r"\b\d+(?:\.\d+)?\s+million dollars\b")
            .case_insensitive(true)
            .build()
            .unwrap()
    });
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "ap-dollar-style",
            FAM,
            Severity::Advisory,
            off,
            len,
            "AP style writes this as '$N million'",
        );
    });
}

fn over_quantity(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::RegexBuilder::new(r"\bover\s+(?:\$?\d|\$)")
            .case_insensitive(true)
            .build()
            .unwrap()
    });
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "ap-over-quantity",
            FAM,
            Severity::Advisory,
            off,
            len,
            "AP style uses 'more than' with quantities",
        );
    });
}

const TOWARD: &[MapEntry] = &[
    me("towards", "toward"),
    me("backwards", "backward"),
    me("upwards", "upward"),
    me("downwards", "downward"),
    me("afterwards", "afterward"),
];

fn toward(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| helpers::map_regex(TOWARD));
    helpers::scan_fancy(ctx, re, |off, len, m| {
        if let Some(to) = helpers::map_lookup(TOWARD, m) {
            em.emit(
                ctx,
                "ap-toward",
                FAM,
                Severity::Advisory,
                off,
                len,
                format!("AP style uses '{to}'"),
            );
        }
    });
}

fn ampersand(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r" & ").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "ap-ampersand",
            FAM,
            Severity::Advisory,
            off,
            len,
            "AP style uses 'and' except in proper names",
        );
    });
}

#[cfg(test)]
mod tests {
    use super::super::runner::{detect_text, test_settings};

    fn ids(text: &str) -> Vec<String> {
        detect_text("x.md", text, &test_settings("ap"))
            .findings
            .into_iter()
            .map(|f| f.rule_id)
            .collect()
    }

    #[test]
    fn ap_pack_rules_fire() {
        let got = ids("One, two, and three. I saw 5 users at 12 PM. Revenue was 5 million dollars, over 5% growth & more afterwards.");
        for id in [
            "ap-serial-comma",
            "ap-number-style",
            "ap-time-format",
            "ap-dollar-style",
            "ap-over-quantity",
            "ap-percent",
            "ap-ampersand",
            "ap-toward",
        ] {
            assert!(got.iter().any(|g| g == id), "{id}: {got:?}");
        }
    }
}
