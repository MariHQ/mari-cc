//! PDF text extraction and OCR (SPEC §4.3 / §8.6), backed exclusively by
//! `baidu/Unlimited-OCR` — no fallback engines. Backends:
//!   `text`      — embedded text only (PyMuPDF)
//!   `auto`      — embedded text; OCR only pages with <16 extractable chars
//!   `ocr-model` — every page through the model
//! The toolchain (a Python venv with PyMuPDF, plus torch/transformers for
//! the model tiers) is auto-provisioned into `~/.mari/ocr` on first use
//! unless `ocr.auto_install=false`. Any failure is loud: the file errors,
//! nothing is silently substituted.

use crate::{config, workspace};
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const DEFAULT_MODEL: &str = "baidu/Unlimited-OCR";

pub struct OcrConfig {
    pub backend: String,
    pub model: String,
    pub dpi: i64,
    pub auto_install: bool,
}

pub fn ocr_config(cfg: &Value) -> OcrConfig {
    let o = &cfg["ocr"];
    let model = o["model"].as_str().unwrap_or("").trim().to_string();
    OcrConfig {
        backend: o["backend"].as_str().unwrap_or("auto").to_string(),
        model: if model.is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            model
        },
        dpi: o["dpi"].as_i64().unwrap_or(200),
        auto_install: o["auto_install"].as_bool().unwrap_or(true),
    }
}

fn ocr_home() -> PathBuf {
    config::mari_home().join("ocr")
}

fn venv_python() -> PathBuf {
    ocr_home().join("venv").join("bin").join("python")
}

/// Whether this backend needs the model stack (torch/transformers), not
/// just PyMuPDF.
pub fn needs_model(backend: &str) -> bool {
    backend != "text"
}

const BASE_PKGS: &[&str] = &["pymupdf==1.27.2.2"];
// Per the Unlimited-OCR README (transformers inference requirements).
const MODEL_PKGS: &[&str] = &[
    "torch",
    "torchvision",
    "transformers==4.57.1",
    "Pillow",
    "einops",
    "addict",
    "easydict",
    "psutil",
];

/// Provision the toolchain on first use (§4.3). Idempotent via marker files.
pub fn ensure_toolchain(cfg: &OcrConfig) -> Result<PathBuf> {
    let home = ocr_home();
    let python = venv_python();
    let base_ok = home.join(".base-ok");
    let model_ok = home.join(".model-ok");
    let need_model = needs_model(&cfg.backend);

    let provisioned = python.exists() && base_ok.exists() && (!need_model || model_ok.exists());
    if !provisioned {
        if !cfg.auto_install {
            return Err(anyhow!(
                "OCR toolchain is not provisioned and ocr.auto_install=false — \
                 run `mari config set ocr.auto_install true` or provision {} yourself",
                home.display()
            ));
        }
        workspace::ensure_dir(&home)?;
        if !python.exists() {
            eprintln!("provisioning OCR toolchain at {} …", home.display());
            let status = Command::new("python3")
                .args(["-m", "venv"])
                .arg(home.join("venv"))
                .status()
                .map_err(|_| anyhow!("python3 not found — the OCR toolchain needs Python 3.12+"))?;
            if !status.success() {
                return Err(anyhow!("failed to create the OCR venv"));
            }
        }
        if !base_ok.exists() {
            pip_install(&python, BASE_PKGS)?;
            std::fs::write(&base_ok, "")?;
        }
        if need_model && !model_ok.exists() {
            eprintln!(
                "installing the {} inference stack (torch + transformers — this is large) …",
                DEFAULT_MODEL
            );
            pip_install(&python, MODEL_PKGS)?;
            std::fs::write(&model_ok, "")?;
        }
    }
    // Keep the runner current with this build.
    let runner = home.join("run_ocr.py");
    std::fs::write(&runner, RUNNER_PY)?;
    Ok(python)
}

fn pip_install(python: &Path, pkgs: &[&str]) -> Result<()> {
    let out = Command::new(python)
        .args(["-m", "pip", "install", "--quiet"])
        .args(pkgs)
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "pip install failed: {}",
            String::from_utf8_lossy(&out.stderr)
                .trim()
                .chars()
                .take(400)
                .collect::<String>()
        ));
    }
    Ok(())
}

/// Extract a PDF's text per the configured backend. No fallbacks: any
/// failure is an error for this file.
pub fn extract_pdf(path: &Path) -> Result<String> {
    let cfg = ocr_config(&config::resolve(Some(&workspace::work_root())));
    extract_pdf_with(path, &cfg)
}

pub fn extract_pdf_with(path: &Path, cfg: &OcrConfig) -> Result<String> {
    if !matches!(cfg.backend.as_str(), "text" | "auto" | "ocr-model") {
        return Err(anyhow!(
            "unknown ocr.backend `{}` — use text | auto | ocr-model",
            cfg.backend
        ));
    }
    let python = ensure_toolchain(cfg)?;
    let out = Command::new(&python)
        .arg(ocr_home().join("run_ocr.py"))
        .arg("--pdf")
        .arg(path)
        .args(["--backend", &cfg.backend])
        .args(["--dpi", &cfg.dpi.to_string()])
        .args(["--model", &cfg.model])
        .output()
        .map_err(|e| anyhow!("failed to run the OCR toolchain: {e}"))?;
    if !out.status.success() {
        return Err(anyhow!(
            "OCR failed for {}: {}",
            path.display(),
            String::from_utf8_lossy(&out.stderr)
                .trim()
                .chars()
                .take(400)
                .collect::<String>()
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    if text.trim().is_empty() {
        return Err(anyhow!("OCR produced no text for {}", path.display()));
    }
    Ok(text)
}

/// The toolchain runner. Implements all three §8.6 backends against
/// baidu/Unlimited-OCR via HuggingFace transformers (per the project README);
/// device is CUDA when available, else CPU. There is no other engine.
const RUNNER_PY: &str = r#"# Mari OCR runner: baidu/Unlimited-OCR, no fallbacks.
import argparse
import glob
import os
import sys
import tempfile

SPARSE_CHARS = 16  # SPEC §8.6: auto OCRs pages with <16 extractable chars


def pdf_pages(pdf_path):
    import fitz
    return fitz.open(pdf_path)


def render_page(doc, index, dpi, out_dir):
    import fitz
    mat = fitz.Matrix(dpi / 72, dpi / 72)
    out = os.path.join(out_dir, "page_%04d.png" % (index + 1))
    doc[index].get_pixmap(matrix=mat).save(out)
    return out


_MODEL = None
_TOKENIZER = None


def load_model(model_id):
    global _MODEL, _TOKENIZER
    if _MODEL is not None:
        return _MODEL, _TOKENIZER
    import torch
    from transformers import AutoModel, AutoTokenizer

    _TOKENIZER = AutoTokenizer.from_pretrained(model_id, trust_remote_code=True)
    cuda = torch.cuda.is_available()
    dtype = torch.bfloat16 if cuda else torch.float32
    model = AutoModel.from_pretrained(
        model_id,
        trust_remote_code=True,
        use_safetensors=True,
        torch_dtype=dtype,
    )
    model = model.eval()
    if cuda:
        model = model.cuda()
    _MODEL = model
    return _MODEL, _TOKENIZER


def read_saved_result(out_dir):
    for pattern in ("*.mmd", "*.md", "*.txt"):
        hits = sorted(glob.glob(os.path.join(out_dir, pattern)))
        if hits:
            with open(hits[-1], "r", encoding="utf-8") as f:
                return f.read()
    return ""


def ocr_pages(model_id, image_files):
    model, tokenizer = load_model(model_id)
    out_dir = tempfile.mkdtemp(prefix="mari_ocr_out_")
    if len(image_files) == 1:
        result = model.infer(
            tokenizer,
            prompt="<image>document parsing.",
            image_file=image_files[0],
            output_path=out_dir,
            base_size=1024, image_size=1024, crop_mode=False,
            max_length=32768,
            no_repeat_ngram_size=35, ngram_window=1024,
            save_results=True,
        )
    else:
        result = model.infer_multi(
            tokenizer,
            prompt="<image>Multi page parsing.",
            image_files=image_files,
            output_path=out_dir,
            image_size=1024,
            max_length=32768,
            no_repeat_ngram_size=35, ngram_window=1024,
            save_results=True,
        )
    if isinstance(result, str) and result.strip():
        return result
    if isinstance(result, (list, tuple)):
        joined = "\n\n".join(str(r) for r in result if str(r).strip())
        if joined.strip():
            return joined
    return read_saved_result(out_dir)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pdf", required=True)
    ap.add_argument("--backend", choices=["text", "auto", "ocr-model"], required=True)
    ap.add_argument("--dpi", type=int, default=200)
    ap.add_argument("--model", default="baidu/Unlimited-OCR")
    args = ap.parse_args()

    doc = pdf_pages(args.pdf)
    n = doc.page_count

    if args.backend == "ocr-model":
        tmp = tempfile.mkdtemp(prefix="mari_ocr_")
        images = [render_page(doc, i, args.dpi, tmp) for i in range(n)]
        sys.stdout.write(ocr_pages(args.model, images))
        return

    page_text = [doc[i].get_text() for i in range(n)]
    if args.backend == "text":
        sys.stdout.write("\n\n".join(t for t in page_text if t.strip()))
        return

    # auto: embedded text; OCR only sparse pages (<16 extractable chars).
    sparse = [i for i, t in enumerate(page_text) if len(t.strip()) < SPARSE_CHARS]
    if sparse:
        tmp = tempfile.mkdtemp(prefix="mari_ocr_")
        for i in sparse:
            image = render_page(doc, i, args.dpi, tmp)
            page_text[i] = ocr_pages(args.model, [image])
    sys.stdout.write("\n\n".join(t for t in page_text if t.strip()))


if __name__ == "__main__":
    main()
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn config_defaults_to_unlimited_ocr() {
        let cfg = ocr_config(
            &json!({"ocr": {"backend": "auto", "model": "", "dpi": 200, "auto_install": true}}),
        );
        assert_eq!(cfg.model, "baidu/Unlimited-OCR");
        assert_eq!(cfg.backend, "auto");
        assert_eq!(cfg.dpi, 200);
        assert!(needs_model("auto"));
        assert!(needs_model("ocr-model"));
        assert!(!needs_model("text"));
    }

    #[test]
    fn unknown_backend_and_disabled_install_fail_loudly() {
        let bad = OcrConfig {
            backend: "tesseract".into(),
            model: DEFAULT_MODEL.into(),
            dpi: 200,
            auto_install: true,
        };
        let err = extract_pdf_with(Path::new("/nonexistent.pdf"), &bad).unwrap_err();
        assert!(err.to_string().contains("unknown ocr.backend"));

        let off = OcrConfig {
            backend: "text".into(),
            model: DEFAULT_MODEL.into(),
            dpi: 200,
            auto_install: false,
        };
        // With auto_install off and (presumably) no provisioned toolchain,
        // this must error rather than fall back. If a toolchain exists on
        // this machine the extract still errors on the missing file.
        let err = extract_pdf_with(Path::new("/nonexistent-mari-test.pdf"), &off).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("auto_install") || msg.contains("OCR failed") || msg.contains("no text"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn runner_embeds_spec_constants() {
        assert!(RUNNER_PY.contains("SPARSE_CHARS = 16"));
        assert!(RUNNER_PY.contains("baidu/Unlimited-OCR"));
        assert!(RUNNER_PY.contains("Multi page parsing."));
        assert!(RUNNER_PY.contains("trust_remote_code=True"));
        // No fallback engines anywhere in the runner.
        assert!(!RUNNER_PY.to_lowercase().contains("tesseract"));
    }
}
