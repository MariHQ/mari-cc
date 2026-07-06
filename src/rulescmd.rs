//! STUB — hooks mgmt, ignores, zero, rules, nudges (SPEC §5.1/§4.7).
use anyhow::Result;
pub fn hooks(_args: &[String], _reason: Option<&str>) -> Result<i32> { eprintln!("not implemented yet"); Ok(1) }
pub fn ignores(_args: &[String], _reason: Option<&str>) -> Result<i32> { eprintln!("not implemented yet"); Ok(1) }
pub fn zero(_args: &[String]) -> Result<i32> { eprintln!("not implemented yet"); Ok(1) }
pub fn rules(_args: &[String], _json: bool, _write: bool, _paths: Option<&str>, _notify: Option<&str>, _exclude: Option<&str>) -> Result<i32> { eprintln!("not implemented yet"); Ok(1) }
pub fn nudge(_args: &[String], _json: bool, _when: Option<&str>, _edit: &[String], _message: Option<&str>, _exclude: Option<&str>) -> Result<i32> { eprintln!("not implemented yet"); Ok(1) }
