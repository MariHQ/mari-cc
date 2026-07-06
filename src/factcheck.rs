//! STUB — grounding (SPEC §5.5/§11.10).
use anyhow::Result;
pub struct FactcheckArgs {
    pub file: String,
    pub source: Option<String>,
    pub kb: bool,
    pub models: bool,
    pub decompose: bool,
    pub claims: Option<String>,
    pub emit_claim_targets: bool,
    pub deep: bool,
    pub ground: Option<String>,
    pub threshold: Option<f64>,
    pub json: bool,
    pub strict: bool,
    pub quiet: bool,
    pub lookback: Option<i64>,
}
pub fn run(_args: FactcheckArgs) -> Result<i32> {
    eprintln!("not implemented yet");
    Ok(1)
}
