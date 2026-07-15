//! Config-overridable detector word/phrase lists (SPEC §4.4/§11).
//!
//! Every list the detector matches against is registered here with a stable
//! `id`, its family/pack, a `Kind`, and its built-in default entries (which
//! remain the single source of truth, referenced from the owning rule module).
//! `detector.lists.<id>` in config replaces a list wholesale (full-replace
//! semantics, SPEC §11.0.8): present → the config value wins; absent → the
//! built-in default; present-but-empty (`[]`) → the list is intentionally
//! emptied and its rule never fires; malformed → the built-in default (a bad
//! edit never silently disables a rule).
//!
//! Rules read their list through `ctx.lists`, so one resolved+compiled set is
//! shared across every file in a run.

use super::helpers::{self, MapEntry};
use super::Family;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// The shape of a list's entries, which fixes both its JSON representation in
/// config and how the console renders its editor.
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Whole-word tokens: JSON `["word", …]`.
    Words,
    /// Multi-word phrases matched longest-first: JSON `["a phrase", …]`.
    Phrases,
    /// Weighted words `(word, base, weight)`: JSON `[["word","base",1.5], …]`.
    Weighted,
    /// `from → to` replacement pairs: JSON `[["from","to"], …]`.
    Map,
    /// Groups of interchangeable variants: JSON `[["sign in","log in"], …]`.
    Groups,
}

/// One registered list: metadata plus its built-in default entries. Exactly one
/// of the `*_default` fields is meaningful, per `kind`.
pub struct ListDef {
    pub id: &'static str,
    pub label: &'static str,
    pub family: Family,
    pub pack: Option<&'static str>,
    pub kind: Kind,
    pub words_default: &'static [&'static str],
    pub weighted_default: &'static [(&'static str, &'static str, f64)],
    pub map_default: &'static [MapEntry],
    pub groups_default: &'static [&'static [&'static str]],
}

impl ListDef {
    const fn words(
        id: &'static str,
        label: &'static str,
        family: Family,
        pack: Option<&'static str>,
        kind: Kind,
        default: &'static [&'static str],
    ) -> ListDef {
        ListDef {
            id,
            label,
            family,
            pack,
            kind,
            words_default: default,
            weighted_default: &[],
            map_default: &[],
            groups_default: &[],
        }
    }

    const fn weighted(
        id: &'static str,
        label: &'static str,
        family: Family,
        pack: Option<&'static str>,
        default: &'static [(&'static str, &'static str, f64)],
    ) -> ListDef {
        ListDef {
            id,
            label,
            family,
            pack,
            kind: Kind::Weighted,
            words_default: &[],
            weighted_default: default,
            map_default: &[],
            groups_default: &[],
        }
    }

    const fn map(
        id: &'static str,
        label: &'static str,
        family: Family,
        pack: Option<&'static str>,
        default: &'static [MapEntry],
    ) -> ListDef {
        ListDef {
            id,
            label,
            family,
            pack,
            kind: Kind::Map,
            words_default: &[],
            weighted_default: &[],
            map_default: default,
            groups_default: &[],
        }
    }

    const fn groups(
        id: &'static str,
        label: &'static str,
        family: Family,
        pack: Option<&'static str>,
        default: &'static [&'static [&'static str]],
    ) -> ListDef {
        ListDef {
            id,
            label,
            family,
            pack,
            kind: Kind::Groups,
            words_default: &[],
            weighted_default: &[],
            map_default: &[],
            groups_default: default,
        }
    }

    /// Built-in default as JSON, for the console's "reset to default" and diff.
    pub fn default_json(&self) -> Value {
        match self.kind {
            Kind::Words | Kind::Phrases => {
                Value::Array(self.words_default.iter().map(|w| Value::from(*w)).collect())
            }
            Kind::Weighted => Value::Array(
                self.weighted_default
                    .iter()
                    .map(|(w, b, wt)| {
                        Value::Array(vec![Value::from(*w), Value::from(*b), Value::from(*wt)])
                    })
                    .collect(),
            ),
            Kind::Map => Value::Array(
                self.map_default
                    .iter()
                    .map(|m| Value::Array(vec![Value::from(m.from), Value::from(m.to)]))
                    .collect(),
            ),
            Kind::Groups => Value::Array(
                self.groups_default
                    .iter()
                    .map(|g| Value::Array(g.iter().map(|v| Value::from(*v)).collect()))
                    .collect(),
            ),
        }
    }
}

use super::{pack_google, pack_microsoft, pack_plain, rules_a, rules_b, rules_c, rules_d};
use Family::*;
use Kind::*;

/// The complete list registry. Order groups by family then pack for the console.
pub fn registry() -> &'static [ListDef] {
    static REG: OnceLock<Vec<ListDef>> = OnceLock::new();
    REG.get_or_init(|| {
        vec![
            // ── Family A — AI-slop tells ──────────────────────────────────
            ListDef::weighted(
                "overused-word",
                "AI-overused words",
                AiSlop,
                None,
                rules_a::OVERUSED,
            ),
            ListDef::words(
                "marketing-buzzword",
                "Marketing buzzwords",
                AiSlop,
                None,
                Phrases,
                rules_a::MARKETING,
            ),
            ListDef::words(
                "cliche-opener",
                "Cliché openers",
                AiSlop,
                None,
                Phrases,
                rules_a::CLICHE_OPENER,
            ),
            ListDef::words(
                "significance-boilerplate",
                "Significance boilerplate",
                AiSlop,
                None,
                Phrases,
                rules_a::SIGNIFICANCE_BOILERPLATE,
            ),
            ListDef::words(
                "sycophancy",
                "Sycophancy",
                AiSlop,
                None,
                Phrases,
                rules_a::SYCOPHANCY,
            ),
            ListDef::words(
                "hedge-overuse",
                "Hedge phrases",
                AiSlop,
                None,
                Phrases,
                rules_a::HEDGES,
            ),
            ListDef::words(
                "conversational-scaffolding",
                "Conversational scaffolding",
                AiSlop,
                None,
                Phrases,
                rules_a::CONVERSATIONAL_SCAFFOLDING,
            ),
            ListDef::words(
                "hype-intensifier",
                "Hype intensifiers",
                AiSlop,
                None,
                Phrases,
                rules_a::HYPE_INTENSIFIER,
            ),
            ListDef::words(
                "serves-as-copula",
                "Inflated copulas",
                AiSlop,
                None,
                Phrases,
                rules_a::SERVES_AS_COPULA,
            ),
            ListDef::words(
                "media-coverage-boilerplate",
                "Media-coverage boilerplate",
                AiSlop,
                None,
                Phrases,
                rules_a::MEDIA_COVERAGE,
            ),
            ListDef::words(
                "future-outlook-speculation",
                "Future-outlook speculation",
                AiSlop,
                None,
                Phrases,
                rules_a::FUTURE_OUTLOOK,
            ),
            ListDef::words(
                "conclusion-restate",
                "Conclusion markers",
                AiSlop,
                None,
                Words,
                rules_a::CONCLUSION_MARKERS,
            ),
            ListDef::words(
                "transition-scaffolding",
                "Transition scaffolding",
                AiSlop,
                None,
                Words,
                rules_a::TRANSITIONS,
            ),
            // ── Family B — clarity & concision ────────────────────────────
            ListDef::map(
                "wordy-phrase",
                "Wordy phrases",
                Clarity,
                None,
                rules_b::WORDY,
            ),
            ListDef::map(
                "complex-word",
                "Complex words",
                Clarity,
                None,
                rules_b::COMPLEX,
            ),
            ListDef::map(
                "nominalization",
                "Nominalizations",
                Clarity,
                None,
                rules_b::NOMINAL,
            ),
            ListDef::words(
                "weasel-word",
                "Weasel words",
                Clarity,
                None,
                Words,
                rules_b::WEASEL,
            ),
            ListDef::words(
                "redundant-pair",
                "Redundant pairs",
                Clarity,
                None,
                Phrases,
                rules_b::REDUNDANT_PAIRS,
            ),
            // ── Family C — shared style ───────────────────────────────────
            ListDef::map(
                "style-swap",
                "Style term swaps",
                Style,
                None,
                rules_c::SWAPS,
            ),
            ListDef::map(
                "contraction",
                "Contractions",
                Style,
                None,
                rules_c::CONTRACTIONS,
            ),
            ListDef::map(
                "singular-they",
                "Singular they",
                Style,
                None,
                rules_c::SINGULAR_THEY,
            ),
            ListDef::words(
                "redundant-acronym",
                "Redundant acronyms",
                Style,
                None,
                Phrases,
                rules_c::REDUNDANT_ACRONYMS,
            ),
            ListDef::groups(
                "terminology-consistency",
                "Terminology variants",
                Style,
                None,
                rules_c::TERM_GROUPS,
            ),
            // ── Family D — inclusive & accessible ─────────────────────────
            ListDef::map(
                "gendered-noun",
                "Gendered nouns",
                Inclusive,
                None,
                rules_d::GENDERED,
            ),
            ListDef::map(
                "ableist-term",
                "Ableist terms (warn)",
                Inclusive,
                None,
                rules_d::ABLEIST_WARN,
            ),
            ListDef::map(
                "ableist-term-advisory",
                "Ableist terms (advisory)",
                Inclusive,
                None,
                rules_d::ABLEIST_ADVISORY,
            ),
            ListDef::words(
                "vague-link-text",
                "Vague link text",
                Inclusive,
                None,
                Words,
                rules_d::VAGUE_LINK,
            ),
            ListDef::map(
                "person-first",
                "Person-first language",
                Inclusive,
                None,
                rules_d::PERSON_FIRST,
            ),
            ListDef::map(
                "tech-inclusive",
                "Non-inclusive tech (warn)",
                Inclusive,
                None,
                rules_d::TECH_WARN,
            ),
            ListDef::map(
                "tech-inclusive-advisory",
                "Non-inclusive tech (advisory)",
                Inclusive,
                None,
                rules_d::TECH_ADVISORY,
            ),
            ListDef::map(
                "violent-metaphor",
                "Violent metaphors",
                Inclusive,
                None,
                rules_d::VIOLENT,
            ),
            ListDef::map(
                "dated-term",
                "Dated / biased terms",
                Inclusive,
                None,
                rules_d::AGEIST,
            ),
            // ── Microsoft pack ────────────────────────────────────────────
            ListDef::words(
                "microsoft-accessibility",
                "Accessibility terms",
                Inclusive,
                Some("microsoft"),
                Phrases,
                pack_microsoft::ACCESSIBILITY_TERMS,
            ),
            ListDef::map(
                "microsoft-gender-bias",
                "Gender-biased terms",
                Inclusive,
                Some("microsoft"),
                pack_microsoft::GENDER_BIAS,
            ),
            ListDef::map(
                "microsoft-contractions",
                "Microsoft contractions",
                Style,
                Some("microsoft"),
                pack_microsoft::MS_CONTRACTIONS,
            ),
            ListDef::map(
                "microsoft-term-swap",
                "Microsoft term swaps",
                Style,
                Some("microsoft"),
                pack_microsoft::TERM_SWAPS,
            ),
            ListDef::map(
                "microsoft-wordiness",
                "Microsoft wordiness",
                Clarity,
                Some("microsoft"),
                pack_microsoft::MS_WORDINESS,
            ),
            ListDef::words(
                "microsoft-adverbs",
                "Microsoft adverbs",
                Clarity,
                Some("microsoft"),
                Words,
                pack_microsoft::MS_ADVERBS,
            ),
            ListDef::words(
                "microsoft-a-z",
                "Microsoft A–Z watch words",
                Style,
                Some("microsoft"),
                Words,
                pack_microsoft::AZ_WORDLIST,
            ),
            // ── Google pack ───────────────────────────────────────────────
            ListDef::map(
                "google-latinism",
                "Latinisms",
                Style,
                Some("google"),
                pack_google::LATINISMS,
            ),
            ListDef::map(
                "google-american-spelling",
                "British → American",
                Style,
                Some("google"),
                pack_google::AMERICAN,
            ),
            ListDef::map(
                "google-directional",
                "Directional references",
                Style,
                Some("google"),
                pack_google::DIRECTIONAL,
            ),
            ListDef::map(
                "google-word-swap",
                "Google word swaps",
                Style,
                Some("google"),
                pack_google::GOOGLE_WORDS,
            ),
            ListDef::words(
                "google-minimizing",
                "Minimizing words",
                Clarity,
                Some("google"),
                Words,
                pack_google::MINIMIZING,
            ),
            ListDef::words(
                "google-preannounce",
                "Time-sensitive wording",
                Style,
                Some("google"),
                Phrases,
                pack_google::PREANNOUNCE,
            ),
            // ── Plain-language pack ───────────────────────────────────────
            ListDef::map(
                "plain-hidden-verb",
                "Hidden verbs",
                Clarity,
                Some("plain"),
                pack_plain::HIDDEN_VERBS,
            ),
            ListDef::map(
                "plain-required-to",
                "Required-to phrases",
                Clarity,
                Some("plain"),
                pack_plain::REQUIRED_TO,
            ),
            ListDef::map(
                "plain-legalese-phrase",
                "Legalese phrases",
                Clarity,
                Some("plain"),
                pack_plain::LEGALESE_PHRASES,
            ),
            ListDef::words(
                "plain-legalese-word",
                "Legalese words",
                Clarity,
                Some("plain"),
                Words,
                pack_plain::LEGALESE_WORDS,
            ),
        ]
    })
}

fn def(id: &str) -> Option<&'static ListDef> {
    registry().iter().find(|d| d.id == id)
}

type WeightedEntry = (String, String, f64);

#[derive(Default)]
struct Cache {
    words: HashMap<String, Arc<Vec<String>>>,
    word_re: HashMap<String, Arc<regex::Regex>>,
    phrase_re: HashMap<String, Arc<fancy_regex::Regex>>,
    weighted: HashMap<String, Arc<Vec<WeightedEntry>>>,
    map: HashMap<String, Arc<Vec<(String, String)>>>,
    map_re: HashMap<String, Arc<fancy_regex::Regex>>,
    groups: HashMap<String, Arc<Vec<Vec<String>>>>,
}

/// Resolved lists for one detector run: the `detector.lists` override object
/// plus a lazily-populated cache of resolved entries and compiled regexes,
/// shared (via `Arc`) across every file's `Ctx`.
pub struct Lists {
    over: Value,
    cache: Mutex<Cache>,
}

impl Lists {
    /// Build from the `detector.lists` config subtree (any non-object → none).
    pub fn from_config(lists: &Value) -> Arc<Lists> {
        Arc::new(Lists {
            over: if lists.is_object() {
                lists.clone()
            } else {
                Value::Null
            },
            cache: Mutex::new(Cache::default()),
        })
    }

    /// Built-ins only (no overrides) — used where a `Ctx` is built outside a
    /// configured run (e.g. grammar location remap).
    pub fn defaults() -> Arc<Lists> {
        Arc::new(Lists {
            over: Value::Null,
            cache: Mutex::new(Cache::default()),
        })
    }

    /// The raw override array for `id`, if the user set one (else `None`).
    fn override_of(&self, id: &str) -> Option<&Value> {
        self.over.get(id).filter(|v| v.is_array())
    }

    /// Resolved word/phrase entries: override strings if set, else the default.
    pub fn words(&self, id: &str) -> Arc<Vec<String>> {
        if let Some(v) = self.cache.lock().unwrap().words.get(id) {
            return v.clone();
        }
        let resolved: Vec<String> = match self.override_of(id) {
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect(),
            _ => def(id)
                .map(|d| d.words_default.iter().map(|w| w.to_string()).collect())
                .unwrap_or_default(),
        };
        let arc = Arc::new(resolved);
        self.cache
            .lock()
            .unwrap()
            .words
            .insert(id.to_string(), arc.clone());
        arc
    }

    /// Whole-word alternation regex for a `Words` list.
    pub fn word_regex(&self, id: &str) -> Arc<regex::Regex> {
        if let Some(v) = self.cache.lock().unwrap().word_re.get(id) {
            return v.clone();
        }
        let words = self.words(id);
        let arc = Arc::new(helpers::word_list(&words));
        self.cache
            .lock()
            .unwrap()
            .word_re
            .insert(id.to_string(), arc.clone());
        arc
    }

    /// Longest-first lookaround-guarded regex for a `Phrases` list.
    pub fn phrase_regex(&self, id: &str) -> Arc<fancy_regex::Regex> {
        if let Some(v) = self.cache.lock().unwrap().phrase_re.get(id) {
            return v.clone();
        }
        let words = self.words(id);
        let arc = Arc::new(helpers::phrase_list(&words));
        self.cache
            .lock()
            .unwrap()
            .phrase_re
            .insert(id.to_string(), arc.clone());
        arc
    }

    /// Resolved `(word, base, weight)` entries for a `Weighted` list.
    pub fn weighted(&self, id: &str) -> Arc<Vec<(String, String, f64)>> {
        if let Some(v) = self.cache.lock().unwrap().weighted.get(id) {
            return v.clone();
        }
        let resolved: Vec<(String, String, f64)> = match self.override_of(id) {
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(|row| {
                    let r = row.as_array()?;
                    let w = r.first()?.as_str()?.to_string();
                    let base = r.get(1).and_then(|v| v.as_str()).unwrap_or(&w).to_string();
                    let wt = r.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0);
                    Some((w, base, wt))
                })
                .collect(),
            _ => def(id)
                .map(|d| {
                    d.weighted_default
                        .iter()
                        .map(|(w, b, wt)| (w.to_string(), b.to_string(), *wt))
                        .collect()
                })
                .unwrap_or_default(),
        };
        let arc = Arc::new(resolved);
        self.cache
            .lock()
            .unwrap()
            .weighted
            .insert(id.to_string(), arc.clone());
        arc
    }

    /// Resolved `from → to` pairs for a `Map` list.
    pub fn map(&self, id: &str) -> Arc<Vec<(String, String)>> {
        if let Some(v) = self.cache.lock().unwrap().map.get(id) {
            return v.clone();
        }
        let resolved: Vec<(String, String)> = match self.override_of(id) {
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(|row| {
                    let r = row.as_array()?;
                    let from = r.first()?.as_str()?.to_string();
                    let to = r.get(1).and_then(|v| v.as_str()).unwrap_or("").to_string();
                    Some((from, to))
                })
                .collect(),
            _ => def(id)
                .map(|d| {
                    d.map_default
                        .iter()
                        .map(|m| (m.from.to_string(), m.to.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
        };
        let arc = Arc::new(resolved);
        self.cache
            .lock()
            .unwrap()
            .map
            .insert(id.to_string(), arc.clone());
        arc
    }

    /// Longest-first regex over a `Map` list's `from` keys.
    pub fn map_regex(&self, id: &str) -> Arc<fancy_regex::Regex> {
        if let Some(v) = self.cache.lock().unwrap().map_re.get(id) {
            return v.clone();
        }
        let map = self.map(id);
        let froms: Vec<&str> = map.iter().map(|(f, _)| f.as_str()).collect();
        let arc = Arc::new(helpers::phrase_list(&froms));
        self.cache
            .lock()
            .unwrap()
            .map_re
            .insert(id.to_string(), arc.clone());
        arc
    }

    /// The replacement for a matched `from` in a `Map` list (case-insensitive).
    pub fn map_lookup(&self, id: &str, matched: &str) -> Option<String> {
        let lower = matched.to_lowercase();
        self.map(id)
            .iter()
            .find(|(f, _)| f.to_lowercase() == lower)
            .map(|(_, t)| t.clone())
    }

    /// Resolved variant groups for a `Groups` list.
    pub fn groups(&self, id: &str) -> Arc<Vec<Vec<String>>> {
        if let Some(v) = self.cache.lock().unwrap().groups.get(id) {
            return v.clone();
        }
        let resolved: Vec<Vec<String>> = match self.override_of(id) {
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(|g| {
                    let inner = g.as_array()?;
                    Some(
                        inner
                            .iter()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect::<Vec<_>>(),
                    )
                })
                .filter(|g: &Vec<String>| !g.is_empty())
                .collect(),
            _ => def(id)
                .map(|d| {
                    d.groups_default
                        .iter()
                        .map(|g| g.iter().map(|s| s.to_string()).collect())
                        .collect()
                })
                .unwrap_or_default(),
        };
        let arc = Arc::new(resolved);
        self.cache
            .lock()
            .unwrap()
            .groups
            .insert(id.to_string(), arc.clone());
        arc
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detector::runner::{detect_text, test_settings};
    use serde_json::json;

    fn count(text: &str, style: &str, lists: serde_json::Value, id: &str) -> usize {
        let mut s = test_settings(style);
        s.lists = Lists::from_config(&lists);
        detect_text("t.md", text, &s)
            .findings
            .iter()
            .filter(|f| f.rule_id == id)
            .count()
    }

    #[test]
    fn registry_ids_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for d in registry() {
            assert!(seen.insert(d.id), "duplicate list id {}", d.id);
        }
        assert_eq!(registry().len(), 49, "update the README and console count");
    }

    #[test]
    fn phrase_override_replaces_default() {
        // "synergy" is a default marketing buzzword; "wombat" is not.
        assert_eq!(
            count(
                "Pure synergy here.",
                "plain",
                json!({}),
                "marketing-buzzword"
            ),
            1
        );
        // Full-replace: synergy no longer flagged, wombat now is.
        let over = json!({ "marketing-buzzword": ["wombat"] });
        assert_eq!(
            count(
                "Pure synergy here.",
                "plain",
                over.clone(),
                "marketing-buzzword"
            ),
            0
        );
        assert_eq!(
            count(
                "A wild wombat appears.",
                "plain",
                over,
                "marketing-buzzword"
            ),
            1
        );
    }

    #[test]
    fn empty_override_disables_a_list() {
        let over = json!({ "marketing-buzzword": [] });
        assert_eq!(
            count("Pure synergy here.", "plain", over, "marketing-buzzword"),
            0
        );
    }

    #[test]
    fn map_override_changes_swap_and_message() {
        // Default complex-word maps utilize→use.
        let mut s = test_settings("plain");
        s.lists = Lists::from_config(&json!({ "complex-word": [["gonzo", "plain"]] }));
        let f = detect_text("t.md", "We gonzo the tool. We utilize it.", &s).findings;
        let complex: Vec<_> = f.iter().filter(|f| f.rule_id == "complex-word").collect();
        assert_eq!(
            complex.len(),
            1,
            "only the overridden term fires: {complex:?}"
        );
        assert!(complex[0].message.ends_with("'plain'"));
    }

    #[test]
    fn malformed_override_falls_back_to_default() {
        // A non-array value is ignored; the built-in list still applies.
        let over = json!({ "marketing-buzzword": "not an array" });
        assert_eq!(
            count("Pure synergy here.", "plain", over, "marketing-buzzword"),
            1
        );
    }

    #[test]
    fn weighted_override_applies() {
        // Replace the overused list with a single high-weight word.
        let over = json!({ "overused-word": [["frobnicate", "frobnicate", 30.0]] });
        // Two distinct default words would normally fire; now only frobnicate counts.
        assert_eq!(
            count(
                "We delve and showcase.",
                "plain",
                over.clone(),
                "overused-word"
            ),
            0
        );
        assert!(
            count(
                "Frobnicate the frobnicate now.",
                "plain",
                over,
                "overused-word"
            ) >= 1
        );
    }
}
