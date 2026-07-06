//! STUB — hybrid retrieval (SPEC §7).
use anyhow::Result;
pub struct SearchArgs {
    pub query: String,
    pub full: Option<usize>,
    pub variants: Vec<String>,
    pub k: Option<usize>,
    pub source: Option<String>,
    pub doc: Option<String>,
    pub author: Option<String>,
    pub since: Option<String>,
    pub before: Option<String>,
    pub tag: Option<String>,
    pub no_tag: Option<String>,
    pub expand: Option<usize>,
    pub json: bool,
}
pub fn run(_args: SearchArgs) -> Result<i32> { eprintln!("not implemented yet"); Ok(1) }
pub fn recent(_source: Option<&str>, _doc: Option<&str>, _author: Option<&str>, _since: Option<&str>, _before: Option<&str>, _limit: usize, _full: Option<usize>, _json: bool) -> Result<i32> { eprintln!("not implemented yet"); Ok(1) }
pub fn doc(_r: &str, _source: Option<&str>, _full: Option<usize>) -> Result<i32> { eprintln!("not implemented yet"); Ok(1) }
pub fn neighbors(_chunk: &str, _radius: usize, _full: Option<usize>) -> Result<i32> { eprintln!("not implemented yet"); Ok(1) }
pub fn related(_r: &str, _source: Option<&str>, _limit: usize, _full: Option<usize>) -> Result<i32> { eprintln!("not implemented yet"); Ok(1) }
