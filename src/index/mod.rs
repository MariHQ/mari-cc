//! STUB — catalog, chunking, search, sync (SPEC §7/§8).
pub mod search;
pub mod sync;
use anyhow::Result;
pub fn sqlcmd(_query: Option<&str>, _global: bool) -> Result<i32> {
    eprintln!("not implemented yet");
    Ok(1)
}
