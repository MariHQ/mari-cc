//! Plain language style pack rules (SPEC §11.8).

use super::ctx::Ctx;
use super::helpers::{self, me, MapEntry};
use super::readability;
use super::{Emitter, Family, Rule, Severity};
use std::sync::OnceLock;

const FAM: Family = Family::Style;
const PACK: &str = "plain";

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
        r("plain-long-sentence", long_sentence),
        r("plain-hidden-verb", hidden_verb),
        r("plain-shall", shall),
        r("plain-required-to", required_to),
        r("plain-legalese-phrase", legalese_phrase),
        r("plain-legalese-word", legalese_word),
        r("plain-double-negative", double_negative),
        r("reading-grade", reading_grade),
    ]
}

fn long_sentence(ctx: &Ctx, em: &mut Emitter) {
    for s in &ctx.sentences {
        if (21..=30).contains(&s.words) {
            em.emit(
                ctx,
                "plain-long-sentence",
                FAM,
                Severity::Advisory,
                s.start,
                s.end - s.start,
                format!(
                    "plain language target is under 20 words — this sentence has {}",
                    s.words
                ),
            );
        }
    }
}

pub const HIDDEN_VERBS: &[MapEntry] = &[
    me("make a determination", "determine"),
    me("provide an explanation", "explain"),
    me("conduct a review", "review"),
    me("perform a calculation", "calculate"),
    me("give authorization", "authorize"),
    me("make a recommendation", "recommend"),
    me("reach a decision", "decide"),
    me("make use of", "use"),
    me("make reference to", "refer to"),
    me("provide notification", "notify"),
    me("make an adjustment", "adjust"),
    me("is in violation of", "violates"),
];

fn hidden_verb(ctx: &Ctx, em: &mut Emitter) {
    map_rule(ctx, em, "plain-hidden-verb", "hidden verb");
}

fn shall(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::RegexBuilder::new(r"\bshall\b")
            .case_insensitive(true)
            .build()
            .unwrap()
    });
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "plain-shall",
            FAM,
            Severity::Advisory,
            off,
            len,
            "plain language uses 'must' for requirements",
        );
    });
}

pub const REQUIRED_TO: &[MapEntry] = &[
    me("is required to", "must"),
    me("are required to", "must"),
    me("will be required to", "must"),
];

fn required_to(ctx: &Ctx, em: &mut Emitter) {
    map_rule(ctx, em, "plain-required-to", "plain language");
}

pub const LEGALESE_PHRASES: &[MapEntry] = &[
    me("pursuant to", "under"),
    me("in accordance with", "under"),
    me("prior to", "before"),
];

fn legalese_phrase(ctx: &Ctx, em: &mut Emitter) {
    map_rule(ctx, em, "plain-legalese-phrase", "legalese");
}

pub const LEGALESE_WORDS: &[&str] = &[
    "herein",
    "thereof",
    "aforementioned",
    "heretofore",
    "notwithstanding",
    "hereinafter",
];

fn legalese_word(ctx: &Ctx, em: &mut Emitter) {
    let re = ctx.lists.word_regex("plain-legalese-word");
    helpers::scan(ctx, &re, |off, len, _| {
        em.emit(
            ctx,
            "plain-legalese-word",
            FAM,
            Severity::Advisory,
            off,
            len,
            "legalese word — use everyday language",
        );
    });
}

fn double_negative(ctx: &Ctx, em: &mut Emitter) {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::RegexBuilder::new(
            r"\bnot\s+(uncommon|unusual|unlikely|unreasonable|unimportant|insignificant|infrequent|inexpensive|unhelpful|impractical|unclear)\b",
        )
        .case_insensitive(true)
        .build()
        .unwrap()
    });
    helpers::scan(ctx, re, |off, len, _| {
        em.emit(
            ctx,
            "plain-double-negative",
            FAM,
            Severity::Advisory,
            off,
            len,
            "double negative — state it positively",
        );
    });
}

fn reading_grade(ctx: &Ctx, em: &mut Emitter) {
    let words: Vec<&str> = ctx.masked.split_whitespace().collect();
    if words.len() < 30 {
        return;
    }
    let stats = readability::grade(&words, ctx.sentences.len());
    let target = ctx.reading_grade_target.unwrap_or(8.0);
    if stats.grade > target {
        em.emit(
            ctx,
            "reading-grade",
            FAM,
            Severity::Advisory,
            0,
            ctx.text.len().min(80),
            format!(
                "reading grade {:.1} exceeds target {:.1} ({} words, {} sentences)",
                stats.grade, target, stats.words, stats.sentences
            ),
        );
    }
}

/// The finding's rule `id` doubles as its `detector.lists` key here.
fn map_rule(ctx: &Ctx, em: &mut Emitter, id: &str, label: &str) {
    let re = ctx.lists.map_regex(id);
    helpers::scan_fancy(ctx, &re, |off, len, m| {
        if let Some(to) = ctx.lists.map_lookup(id, m) {
            em.emit(
                ctx,
                id,
                FAM,
                Severity::Advisory,
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
        detect_text("x.md", text, &test_settings("plain"))
            .findings
            .into_iter()
            .map(|f| f.rule_id)
            .collect()
    }

    #[test]
    fn plain_pack_rules_fire() {
        let got = ids(
            "The agency shall make a determination prior to review. The users are required to file the aforementioned form herein. That result is not uncommon. This sentence has enough small words to land inside the plain language advisory band for reviewers who need shorter direct public instructions today.",
        );
        for id in [
            "plain-shall",
            "plain-hidden-verb",
            "plain-legalese-phrase",
            "plain-required-to",
            "plain-legalese-word",
            "plain-double-negative",
            "plain-long-sentence",
        ] {
            assert!(got.iter().any(|g| g == id), "{id}: {got:?}");
        }
    }

    #[test]
    fn reading_grade_flags_dense_plain_text() {
        let got = ids(
            "Comprehensive institutional implementation requires interdisciplinary coordination, extensive documentation, procedural standardization, and continuous administrative evaluation before stakeholders can meaningfully understand operational responsibilities. Additional organizational considerations include authorization dependencies, regulatory interpretation, procurement complexity, and cross-functional accountability.",
        );
        assert!(got.iter().any(|g| g == "reading-grade"), "{got:?}");
    }
}
