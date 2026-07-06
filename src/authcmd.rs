//! STUB — `mari auth` / `mari scope` (SPEC §5.1).
use anyhow::Result;
pub struct AuthFlags {
    pub token: Option<String>,
    pub url: Option<String>,
    pub email: Option<String>,
    pub subdomain: Option<String>,
    pub key: Option<String>,
    pub secret: Option<String>,
    pub method: Option<String>,
}
pub fn run(_provider: &str, _flags: AuthFlags) -> Result<i32> {
    eprintln!("not implemented yet");
    Ok(1)
}
pub fn scope(_source: Option<&str>, _scope: Option<&str>) -> Result<i32> {
    eprintln!("not implemented yet");
    Ok(1)
}
