//! End-to-end checks for the repository-local prose workflow.

use std::path::Path;
use std::process::Command;

fn mari(cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mari"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run mari")
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn primary_journey_end_to_end() {
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path();
    assert!(Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["init", "-q"])
        .status()
        .unwrap()
        .success());

    std::fs::write(
        root.join("slop.md"),
        "In today's fast-paced world, it's important to note that we leverage synergy.\n",
    )
    .unwrap();

    let output = mari(root, &["detect", "slop.md", "--quiet"]);
    let detected = stdout(&output);
    assert!(
        detected.contains("cliche-opener")
            || detected.contains("filler-phrase")
            || detected.contains("marketing-buzzword"),
        "detect output: {detected}"
    );

    let output = mari(root, &["config", "set", "detector.grammar", "false"]);
    assert!(output.status.success());
    assert!(root.join(".mari/config.json").is_file());
    let output = mari(root, &["config", "get", "detector.grammar"]);
    assert_eq!(stdout(&output).trim(), "false");

    let output = mari(root, &["status"]);
    assert!(stdout(&output).contains("word lists"));
}

#[test]
fn hook_reports_findings_and_never_breaks_the_turn() {
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path();
    assert!(Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["init", "-q"])
        .status()
        .unwrap()
        .success());
    std::fs::write(
        root.join("doc.md"),
        "In today's fast-paced world we leverage synergy.\n",
    )
    .unwrap();

    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_mari"))
        .args(["hook", "run"])
        .current_dir(root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let payload = format!(
        r#"{{"tool_input":{{"file_path":"{}/doc.md"}}}}"#,
        root.display()
    );
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(!stdout(&output).trim().is_empty());
}

#[test]
fn detector_handles_reference_docs() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = mari(repo, &["detect", "skills/mari/references", "--json"]);
    let value: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(value["summary"]["errors"].as_u64().unwrap_or(0), 0);
}
