//! STUB — localization (SPEC §5.7).
use anyhow::Result;
use std::path::Path;
pub fn run(_args: &[String], _deep: bool, _limit: Option<usize>, _strict: bool) -> Result<i32> {
    eprintln!("not implemented yet");
    Ok(1)
}
/// Localized translation files are skipped by the detector (SPEC §11.0.6).
pub fn is_translation_file(_path: &Path) -> bool {
    false
}
/// Translation siblings of a source file, for the hook staleness note.
pub fn siblings(_path: &Path) -> Vec<std::path::PathBuf> {
    Vec::new()
}
