//! Chicago style pack rules (SPEC §11.7).

use super::ctx::Ctx;
use super::helpers::{self, me, MapEntry};
use super::{Emitter, Family, Rule, Severity};
use std::sync::OnceLock;

const FAM: Family = Family::Style;
const PACK: &str = "chicago";

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
        r("chicago-number-style", number_style),
        r("chicago-directional-s", directional_s),
        r("chicago-percent-symbol", percent_symbol),
        r("chicago-em-dash-spacing", em_dash_spacing),
        r("chicago-ellipsis", ellipsis),
        r("chicago-united-states-noun", united_states_noun),
        r("chicago-ibid", ibid),
    ]
}

fn number_style(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        fancy_regex::Regex::new(r"(?<![\w.$%/-])([1-9][0-9]?|100)(?![\w.$%/-])").unwrap()
    });
    helpers::scan_fancy(ctx, re, |off, len, _| {
        if !ctx.is_table_line(off) {
            em.emit(
                ctx,
                "chicago-number-style",
                FAM,
                Severity::Advisory,
                off,
                len,
                "Chicago style spells out whole numbers through one hundred",
            );
        }
    });
}

const DIRECTIONAL: &[MapEntry] = &[
    me("towards", "toward"),
    me("afterwards", "afterward"),
    me("backwards", "backward"),
    me("upwards", "upward"),
    me("downwards", "downward"),
    me("onwards", "onward"),
];

fn directional_s(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| helpers::map_regex(DIRECTIONAL));
    helpers::scan_fancy(ctx, re, |off, len, m| {
        if let Some(to) = helpers::map_lookup(DIRECTIONAL, m) {
            em.emit(
                ctx,
                "chicago-directional-s",
                FAM,
                Severity::Advisory,
                off,
                len,
                format!("Chicago style uses '{to}'"),
            );
        }
    });
}

fn percent_symbol(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\b\d+(?:\.\d+)?%").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        if !ctx.is_table_line(off) {
            em.emit(
                ctx,
                "chicago-percent-symbol",
                FAM,
                Severity::Advisory,
                off,
                len,
                "Chicago style spells out percent in running prose",
            );
        }
    });
}

fn em_dash_spacing(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(" — ").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "chicago-em-dash-spacing",
            FAM,
            Severity::Advisory,
            off,
            len,
            "Chicago style closes up em dashes",
        );
    });
}

fn ellipsis(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new("…").unwrap());
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "chicago-ellipsis",
            FAM,
            Severity::Advisory,
            off,
            len,
            "Chicago style uses three spaced periods: . . .",
        );
    });
}

fn united_states_noun(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        fancy_regex::Regex::new(
            r"(?i:\bthe\s+U\.?S\.?)(?=\s+(?:is|are|was|were|has|have|had|will|would)\b|[.!?,;:])",
        )
        .unwrap()
    });
    helpers::scan_fancy(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "chicago-united-states-noun",
            FAM,
            Severity::Advisory,
            off,
            len,
            "spell out United States when used as a noun",
        );
    });
}

fn ibid(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| helpers::phrase_list(&["ibid.", "op. cit.", "loc. cit."]));
    helpers::scan_fancy(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "chicago-ibid",
            FAM,
            Severity::Advisory,
            off,
            len,
            "Chicago 17 prefers shortened citations over ibid./op. cit./loc. cit.",
        );
    });
}

#[cfg(test)]
mod tests {
    use super::super::runner::{detect_text, test_settings};

    fn ids(text: &str) -> Vec<String> {
        detect_text("x.md", text, &test_settings("chicago"))
            .findings
            .into_iter()
            .map(|f| f.rule_id)
            .collect()
    }

    #[test]
    fn chicago_pack_rules_fire() {
        let got = ids(
            "The U.S. is large — really large… It grew 9% and had 12 teams afterwards. See ibid.",
        );
        for id in [
            "chicago-united-states-noun",
            "chicago-em-dash-spacing",
            "chicago-ellipsis",
            "chicago-percent-symbol",
            "chicago-number-style",
            "chicago-directional-s",
            "chicago-ibid",
        ] {
            assert!(got.iter().any(|g| g == id), "{id}: {got:?}");
        }
    }
}
