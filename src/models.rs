//! Local model provisioning (SPEC §7 security / §17): download GGUFs from
//! pinned Hugging Face revisions into `~/.mari/models`, verify against a
//! known SHA-256, resume interrupted transfers, and never accept a truncated
//! or tampered file. Shared by the embedding (`index::vector`) and attention
//! (`attn`) tiers and driven explicitly by `mari model pull`.

use crate::{config, workspace};
use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::PathBuf;

pub struct ModelSpec {
    pub file: &'static str,
    pub url: &'static str,
    /// Expected SHA-256, lowercase hex. Empty string disables verification
    /// (until the revision is pinned and the hash captured).
    pub sha256: &'static str,
    pub approx_mb: u64,
    pub kind: &'static str,
    pub auto_download: bool,
}

use std::sync::Once;

/// Suppress llama.cpp / ggml's verbose tensor-loading and inference logs
/// (they otherwise dump tens of KB to stderr per model load, cluttering
/// output and confusing agents that read stderr). Errors still pass through.
/// Idempotent and thread-safe.
pub fn quiet_llama_logs() {
    static INIT: Once = Once::new();
    INIT.call_once(|| unsafe {
        llama_cpp_sys_2::llama_log_set(Some(quiet_log_cb), std::ptr::null_mut());
        llama_cpp_sys_2::ggml_log_set(Some(quiet_log_cb), std::ptr::null_mut());
    });
}

unsafe extern "C" fn quiet_log_cb(
    level: llama_cpp_sys_2::ggml_log_level,
    text: *const std::os::raw::c_char,
    _user_data: *mut std::os::raw::c_void,
) {
    // Only surface genuine errors; drop info/warn/debug/cont chatter.
    if level == llama_cpp_sys_2::GGML_LOG_LEVEL_ERROR && !text.is_null() {
        let msg = std::ffi::CStr::from_ptr(text).to_string_lossy();
        eprint!("{msg}");
    }
}

pub fn models_dir() -> PathBuf {
    config::mari_home().join("models")
}

pub fn model_path(file: &str) -> PathBuf {
    models_dir().join(file)
}

/// Return the model path, downloading + verifying on first use when allowed.
pub fn ensure_gguf(spec: &ModelSpec) -> Result<PathBuf> {
    let path = model_path(spec.file);
    if path.exists() {
        // If a checksum is pinned, verify a cached file once and reject a
        // corrupted cache rather than loading garbage.
        if !spec.sha256.is_empty() {
            let marker = path.with_extension("gguf.verified");
            if !marker.exists() {
                verify_sha256(&path, spec.sha256)?;
                let _ = std::fs::write(&marker, spec.sha256);
            }
        }
        return Ok(path);
    }
    if !spec.auto_download {
        return Err(anyhow!(
            "{} model {} is missing and auto_download is off — run `mari model pull {}` or place the GGUF at {}",
            spec.kind,
            spec.file,
            spec.kind,
            path.display()
        ));
    }
    download(spec, &path)?;
    Ok(path)
}

/// Force a (re)download regardless of cache — the `mari model pull` path.
pub fn pull(spec: &ModelSpec) -> Result<PathBuf> {
    let path = model_path(spec.file);
    download(spec, &path)?;
    Ok(path)
}

fn download(spec: &ModelSpec, path: &std::path::Path) -> Result<()> {
    workspace::ensure_dir(path.parent().unwrap())?;
    let part = path.with_extension("part");
    // Resume: if a partial download exists, request the remaining byte range.
    let resume_from = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    eprintln!(
        "downloading {} (~{} MB, one-time){} …",
        spec.file,
        spec.approx_mb,
        if resume_from > 0 {
            format!(", resuming at {} MB", resume_from >> 20)
        } else {
            String::new()
        }
    );
    let mut req = ureq::get(spec.url).timeout(std::time::Duration::from_secs(3600));
    if resume_from > 0 {
        req = req.set("Range", &format!("bytes={resume_from}-"));
    }
    let resp = req
        .call()
        .map_err(|e| anyhow!("model download failed: {e}"))?;
    let appending = resp.status() == 206;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(appending)
        .truncate(!appending)
        .open(&part)?;
    let mut reader = resp.into_reader();
    let mut buf = [0u8; 1 << 20];
    let mut total = if appending { resume_from } else { 0 };
    let mut since_report = 0u64;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        total += n as u64;
        since_report += n as u64;
        if since_report >= 64 << 20 {
            eprintln!("  … {} MB", total >> 20);
            since_report = 0;
        }
    }
    file.flush()?;
    drop(file);
    if total < (spec.approx_mb.saturating_sub(spec.approx_mb / 4)) << 20 {
        let _ = std::fs::remove_file(&part);
        return Err(anyhow!(
            "model download truncated ({} MB of ~{} MB) — retry `mari model pull {}`",
            total >> 20,
            spec.approx_mb,
            spec.kind
        ));
    }
    if !spec.sha256.is_empty() {
        verify_sha256(&part, spec.sha256).inspect_err(|_| {
            let _ = std::fs::remove_file(&part);
        })?;
        let _ = std::fs::write(path.with_extension("gguf.verified"), spec.sha256);
    }
    std::fs::rename(&part, path)?;
    eprintln!("  ✓ {} model ready at {}", spec.kind, path.display());
    Ok(())
}

fn verify_sha256(path: &std::path::Path, expected: &str) -> Result<()> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let got = format!("{:x}", hasher.finalize());
    if !got.eq_ignore_ascii_case(expected) {
        return Err(anyhow!(
            "checksum mismatch for {}: expected {expected}, got {got} — refusing to use a tampered or corrupted model",
            path.display()
        ));
    }
    Ok(())
}

/// `mari model pull [embedding|attention|all]` — explicit, resumable,
/// checksum-verified provisioning.
pub fn run(args: &[String]) -> Result<i32> {
    // Surface: `mari model pull [target]` | `mari model status`. Strip a
    // leading `pull` verb so `mari model pull attention` names the target.
    let rest: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let which = match rest.as_slice() {
        ["pull", target, ..] => *target,
        ["pull"] => "all",
        [first, ..] => *first,
        [] => "all",
    };
    let embedding = crate::index::vector::model_spec();
    let attention = crate::attn::model_spec();
    let targets: Vec<&ModelSpec> = match which {
        "embedding" => vec![&embedding],
        "attention" => vec![&attention],
        "all" => vec![&embedding, &attention],
        "status" => {
            for spec in [&embedding, &attention] {
                let p = model_path(spec.file);
                let state = if p.exists() {
                    format!(
                        "present ({} MB)",
                        std::fs::metadata(&p).map(|m| m.len() >> 20).unwrap_or(0)
                    )
                } else {
                    "missing".into()
                };
                println!("{:<10} {:<40} {state}", spec.kind, spec.file);
            }
            return Ok(0);
        }
        other => {
            eprintln!("usage: mari model pull [embedding|attention|all] | status  (got {other})");
            return Ok(2);
        }
    };
    for spec in targets {
        // pull() honors auto_download intent; force it on for an explicit pull.
        let mut spec = ModelSpec {
            auto_download: true,
            ..spec_clone(spec)
        };
        spec.auto_download = true;
        pull(&spec)?;
    }
    Ok(0)
}

fn spec_clone(s: &ModelSpec) -> ModelSpec {
    ModelSpec {
        file: s.file,
        url: s.url,
        sha256: s.sha256,
        approx_mb: s.approx_mb,
        kind: s.kind,
        auto_download: s.auto_download,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn which_target<'a>(args: &[&'a str]) -> &'a str {
        match args {
            ["pull", target, ..] => target,
            ["pull"] => "all",
            [first, ..] => first,
            [] => "all",
        }
    }

    #[test]
    fn pull_verb_names_the_target() {
        assert_eq!(which_target(&["pull", "attention"]), "attention");
        assert_eq!(which_target(&["pull", "all"]), "all");
        assert_eq!(which_target(&["pull"]), "all");
        assert_eq!(which_target(&["status"]), "status");
        assert_eq!(which_target(&[]), "all");
    }

    #[test]
    fn checksum_rejects_mismatch() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"hello world").unwrap();
        // sha256("hello world")
        let ok = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        assert!(verify_sha256(tmp.path(), ok).is_ok());
        assert!(verify_sha256(tmp.path(), &"0".repeat(64)).is_err());
    }
}
