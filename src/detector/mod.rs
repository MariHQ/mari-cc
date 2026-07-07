pub mod ctx;
pub mod helpers;
pub mod pack_ap;
pub mod pack_chicago;
pub mod pack_google;
pub mod pack_microsoft;
pub mod pack_plain;
pub mod readability;
pub mod render;
pub mod rules_a;
pub mod rules_b;
pub mod rules_c;
pub mod rules_d;
pub mod runner;
pub mod score;

use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Advisory,
    Warn,
    Error,
}

impl Severity {
    pub fn label(&self) -> &'static str {
        match self {
            Severity::Advisory => "advisory",
            Severity::Warn => "warn",
            Severity::Error => "error",
        }
    }
}

/// Rule families per SPEC §11: A ai-slop, B clarity, C style, D inclusive,
/// grounding, grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Family {
    #[serde(rename = "ai-slop")]
    AiSlop,
    #[serde(rename = "clarity")]
    Clarity,
    #[serde(rename = "style")]
    Style,
    #[serde(rename = "inclusive")]
    Inclusive,
    #[serde(rename = "grounding")]
    Grounding,
    #[serde(rename = "grammar")]
    Grammar,
}

impl Family {
    pub fn label(&self) -> &'static str {
        match self {
            Family::AiSlop => "ai-slop",
            Family::Clarity => "clarity",
            Family::Style => "style",
            Family::Inclusive => "inclusive",
            Family::Grounding => "grounding",
            Family::Grammar => "grammar",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Finding {
    #[serde(rename = "ruleId")]
    pub rule_id: String,
    pub family: Family,
    pub severity: Severity,
    /// Byte offset into the ORIGINAL text.
    pub offset: usize,
    pub length: usize,
    /// Matched source text, whitespace-collapsed, capped at 80 chars.
    pub span: String,
    pub message: String,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,
    #[serde(skip)]
    pub line: usize,
    #[serde(skip)]
    pub col: usize,
}

pub struct Rule {
    pub id: &'static str,
    pub family: Family,
    /// Runs only when this pack is selected (None = always on).
    pub pack: Option<&'static str>,
    pub run: fn(&ctx::Ctx, &mut Emitter),
}

/// Collects findings for one file; rules emit through this.
pub struct Emitter {
    pub findings: Vec<Finding>,
    pub zero_tolerance: HashSet<String>,
}

impl Emitter {
    pub fn new(zero_tolerance: HashSet<String>) -> Self {
        Emitter {
            findings: Vec::new(),
            zero_tolerance,
        }
    }

    pub fn zero_tol(&self, id: &str) -> bool {
        self.zero_tolerance.contains(id)
    }

    /// Emit a finding anchored at `offset..offset+length` in the original text.
    pub fn emit(
        &mut self,
        ctx: &ctx::Ctx,
        id: &str,
        family: Family,
        severity: Severity,
        offset: usize,
        length: usize,
        message: impl Into<String>,
    ) {
        let span = ctx.span_at(offset, length);
        let (line, col) = ctx.locate(offset);
        self.findings.push(Finding {
            rule_id: id.to_string(),
            family,
            severity,
            offset,
            length,
            span,
            message: message.into(),
            r#ref: None,
            line,
            col,
        });
    }
}

/// The complete rule registry: always-on families plus every pack
/// (pack gating happens in the runner).
pub fn registry() -> Vec<Rule> {
    let mut all = Vec::new();
    all.extend(rules_a::rules());
    all.extend(rules_b::rules());
    all.extend(rules_c::rules());
    all.extend(pack_microsoft::rules());
    all.extend(pack_google::rules());
    all.extend(pack_ap::rules());
    all.extend(pack_chicago::rules());
    all.extend(pack_plain::rules());
    all.extend(rules_d::rules());
    all
}
