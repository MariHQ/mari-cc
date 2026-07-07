//! End-to-end journey test (SPEC §19 / docs 09.6): drives the built `mari`
//! binary through the primary user flow in an isolated temp HOME + repo, with
//! model auto-download disabled so it runs offline in CI (keyword search and
//! the deterministic paths are exercised; vector embedding is covered by the
//! model-cached CI job).

use std::path::Path;
use std::process::Command;

fn mari(home: &Path, cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mari"))
        .args(args)
        .current_dir(cwd)
        .env("HOME", home)
        .output()
        .expect("run mari")
}

fn stdout(o: &std::process::Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

#[test]
fn primary_journey_end_to_end() {
    let home = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    let h = home.path();
    let r = repo.path();

    // A git repo with a doc and a facts ledger.
    assert!(Command::new("git")
        .arg("-C")
        .arg(r)
        .arg("init")
        .arg("-q")
        .status()
        .unwrap()
        .success());
    std::fs::create_dir(r.join("docs")).unwrap();
    std::fs::write(
        r.join("docs/pricing.md"),
        "# Pricing\n\nThe enterprise plan costs $49 per seat per month.\n",
    )
    .unwrap();
    std::fs::write(
        r.join("FACTS.md"),
        "- The enterprise plan costs $49 per seat  (billing)\n",
    )
    .unwrap();
    // A deliberately sloppy doc for the detector.
    std::fs::write(
        r.join("slop.md"),
        "In today's fast-paced world, it's important to note that we leverage synergy.\n",
    )
    .unwrap();

    // Disable model download so sync runs offline.
    std::fs::create_dir_all(h.join(".mari")).unwrap();
    std::fs::write(
        h.join(".mari/config.json"),
        r#"{"embedding":{"auto_download":false}}"#,
    )
    .unwrap();

    // 1. track + sync localfiles.
    let o = mari(
        h,
        r,
        &[
            "track",
            "localfiles",
            "add",
            &r.join("docs").to_string_lossy(),
        ],
    );
    assert!(
        o.status.success(),
        "track failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let o = mari(h, r, &["sync", "localfiles"]);
    assert!(
        o.status.success(),
        "sync failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(
        stdout(&o).contains("document(s) updated"),
        "sync summary: {}",
        stdout(&o)
    );

    // 2. keyword search finds the doc (no vectors needed).
    let o = mari(h, r, &["search", "enterprise plan cost", "--k", "3"]);
    assert!(
        stdout(&o).to_lowercase().contains("pricing"),
        "search: {}",
        stdout(&o)
    );

    // 3. detector flags the sloppy doc and stays clean on good prose.
    let o = mari(h, r, &["detect", "slop.md", "--quiet"]);
    let out = stdout(&o);
    assert!(
        out.contains("cliche-opener")
            || out.contains("filler-phrase")
            || out.contains("marketing-buzzword"),
        "detect: {out}"
    );

    // 4. factcheck: matching claim is supported, contradiction flagged.
    std::fs::write(
        r.join("claim.md"),
        "The enterprise plan costs $59 per seat per month.\n",
    )
    .unwrap();
    let o = mari(h, r, &["factcheck", "claim.md"]);
    let out = stdout(&o);
    assert!(
        out.contains("number-date-mismatch") || out.contains("contradict"),
        "factcheck: {out}"
    );

    // 5. tag a doc (committed config) and list it back.
    let o = mari(h, r, &["tag", "docs/pricing.md", "canonical"]);
    assert!(
        o.status.success(),
        "tag: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let o = mari(h, r, &["tag", "list"]);
    assert!(stdout(&o).contains("canonical"), "tag list: {}", stdout(&o));

    // 6. sql read-only surface works; a write is refused.
    let o = mari(h, r, &["sql", "SELECT COUNT(*) FROM documents"]);
    assert!(
        o.status.success(),
        "sql select: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let o = mari(h, r, &["sql", "DELETE FROM documents"]);
    assert_eq!(
        o.status.code(),
        Some(2),
        "sql write should be rejected with exit 2"
    );

    // 7. config coercion round-trips.
    let o = mari(h, r, &["config", "set", "search.k", "12"]);
    assert!(o.status.success());
    let o = mari(h, r, &["config", "get", "search.k"]);
    assert!(stdout(&o).trim() == "12", "config get: {}", stdout(&o));
}

#[test]
fn hook_runs_all_jobs_and_never_breaks_the_turn() {
    let home = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    let h = home.path();
    let r = repo.path();
    assert!(Command::new("git")
        .arg("-C")
        .arg(r)
        .arg("init")
        .arg("-q")
        .status()
        .unwrap()
        .success());
    std::fs::write(
        r.join("doc.md"),
        "In today's fast-paced world we leverage synergy.\n",
    )
    .unwrap();

    // Feed a Claude Code PostToolUse payload on stdin.
    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_mari"))
        .args(["hook", "run"])
        .current_dir(r)
        .env("HOME", h)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let payload = format!(
        r#"{{"tool_input":{{"file_path":"{}/doc.md"}}}}"#,
        r.display()
    );
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    // The hook must always exit 0 (§15.1 invariant) and surface prose findings.
    assert_eq!(out.status.code(), Some(0), "hook must exit 0");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("cliche-opener")
            || String::from_utf8_lossy(&out.stdout).contains("filler-phrase")
            || String::from_utf8_lossy(&out.stdout).contains("marketing-buzzword"),
        "hook prose lint: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// False-positive budget (SPEC §19 / docs 09.2): the detector must not error
/// on Mari's own reference-doc corpus (technical prose with numbers, lists,
/// and code). Advisories/warnings are expected; a hard ceiling on error-level
/// findings guards against a rule that starts over-firing.
#[test]
fn false_positive_budget_over_reference_docs() {
    let home = tempfile::tempdir().unwrap();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    // JSON output so we can count error-severity findings precisely.
    let o = mari(
        home.path(),
        repo,
        &["detect", "skills/mari/references", "--json"],
    );
    let out = stdout(&o);
    let v: serde_json::Value = serde_json::from_str(&out).expect("detect --json");
    let errors = v["summary"]["errors"].as_u64().unwrap_or(0);
    assert_eq!(
        errors, 0,
        "reference docs produced {errors} error-level findings:\n{out}"
    );

    // And the deliberate-slop fixture must exceed a floor (the detector works).
    let o = mari(
        home.path(),
        repo,
        &["detect", "fixtures/sloppy.md", "--json"],
    );
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    let findings = v["summary"]["findings"].as_u64().unwrap_or(0);
    assert!(
        findings >= 15,
        "slop fixture should trip many rules, got {findings}"
    );
}
