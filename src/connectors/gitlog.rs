//! Git commit-history indexing (SPEC §6.4): one document per commit,
//! `doc_id = <repo>:<sha>`, URL derived from the origin remote when
//! GitHub/GitLab-shaped, chat-sized chunking (config `git.chunking`).
//! Incremental last-HEAD cursor reading `last..HEAD`; rebase/force-push
//! triggers a full scan and prune of vanished commits.

use super::cloud::{get_meta, ingest_remote_doc, RemoteDoc, SyncStats};
use crate::index::set_meta;
use anyhow::Result;
use duckdb::Connection;
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

pub fn sync(conn: &Connection, repos: &[std::path::PathBuf], rebuild: bool) -> Result<SyncStats> {
    let mut stats = SyncStats::default();
    for repo in repos {
        if !repo.join(".git").exists() {
            continue;
        }
        if let Err(e) = sync_repo(conn, repo, rebuild, &mut stats) {
            eprintln!("note: git history {}: {e}", repo.display());
        }
    }
    Ok(stats)
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    if !out.status.success() {
        return Err(anyhow::anyhow!(
            "git {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn repo_slug(repo: &Path) -> String {
    repo.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".into())
}

fn sync_repo(conn: &Connection, repo: &Path, rebuild: bool, stats: &mut SyncStats) -> Result<()> {
    let head = git(repo, &["rev-parse", "HEAD"])?.trim().to_string();
    if head.is_empty() {
        return Ok(());
    }
    let slug = repo_slug(repo);
    let cursor_key = format!("git.head.{}", repo.display());
    let cursor = if rebuild {
        None
    } else {
        get_meta(conn, &cursor_key)
    };

    // Cursor still an ancestor? Then read last..HEAD; otherwise the history
    // was rewritten — full scan + prune vanished commits (§6.4).
    let (range, full_scan) = match &cursor {
        Some(last) if last == &head => return Ok(()),
        Some(last)
            if Command::new("git")
                .arg("-C")
                .arg(repo)
                .args(["merge-base", "--is-ancestor", last, &head])
                .status()
                .map(|s| s.success())
                .unwrap_or(false) =>
        {
            (format!("{last}..HEAD"), false)
        }
        Some(_) => ("HEAD".to_string(), true),
        None => ("HEAD".to_string(), true),
    };

    let commit_url_base = commit_url_base(&remote_url(repo));
    // \x1e separates commits, \x1f separates header fields from the file list.
    let log = git(
        repo,
        &[
            "log",
            &range,
            "--name-only",
            "--pretty=format:%x1e%H%x1f%an%x1f%aI%x1f%s%x1f%b%x1f",
        ],
    )?;
    for record in log.split('\u{1e}').filter(|r| !r.trim().is_empty()) {
        let parts: Vec<&str> = record.splitn(6, '\u{1f}').collect();
        if parts.len() < 6 {
            continue;
        }
        let (sha, author, date, subject, body, files) = (
            parts[0].trim(),
            parts[1],
            parts[2],
            parts[3],
            parts[4],
            parts[5],
        );
        stats.seen += 1;
        let doc = commit_doc(
            &slug,
            commit_url_base.as_deref(),
            sha,
            author,
            date,
            subject,
            body,
            files,
        );
        match ingest_remote_doc(conn, "git", &doc) {
            Ok(Some(chunks)) => {
                stats.changed += 1;
                stats.chunks += chunks;
            }
            Ok(None) => {}
            Err(e) => eprintln!("note: git commit {sha} skipped: {e}"),
        }
    }

    if full_scan && cursor.is_some() {
        // Prune commit docs that vanished from the rewritten history.
        let all: BTreeSet<String> = git(repo, &["rev-list", "HEAD"])?
            .lines()
            .map(|sha| format!("{slug}:{sha}"))
            .collect();
        let mut stmt = conn.prepare(
            "SELECT doc_id, external_id FROM documents WHERE source_id = 'git' AND kind = 'commit' AND external_id LIKE ?1",
        )?;
        let rows = stmt.query_map([format!("{slug}:%")], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for (doc_id, ext) in rows.flatten() {
            if !all.contains(&ext) {
                crate::index::sync::delete_doc(conn, &doc_id)?;
                stats.deleted += 1;
            }
        }
    }
    set_meta(conn, &cursor_key, &head)?;
    Ok(())
}

fn remote_url(repo: &Path) -> String {
    git(repo, &["remote", "get-url", "origin"])
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// https://host/owner/repo/commit/ when the origin is GitHub/GitLab-shaped.
pub fn commit_url_base(remote: &str) -> Option<String> {
    if remote.is_empty() {
        return None;
    }
    let https = if let Some(rest) = remote.strip_prefix("git@") {
        // git@github.com:owner/repo.git
        let (host, path) = rest.split_once(':')?;
        format!("https://{host}/{path}")
    } else if remote.starts_with("https://") || remote.starts_with("http://") {
        remote.to_string()
    } else {
        return None;
    };
    let base = https.trim_end_matches('/').trim_end_matches(".git");
    let host_ok = base.contains("github.") || base.contains("gitlab.");
    host_ok.then(|| {
        let sep = if base.contains("gitlab.") {
            "/-/commit/"
        } else {
            "/commit/"
        };
        format!("{base}{sep}")
    })
}

#[allow(clippy::too_many_arguments)]
pub fn commit_doc(
    slug: &str,
    url_base: Option<&str>,
    sha: &str,
    author: &str,
    date: &str,
    subject: &str,
    body: &str,
    files: &str,
) -> RemoteDoc {
    let mut text = format!("{subject}\n");
    let body = body.trim();
    if !body.is_empty() {
        text.push_str(&format!("\n{body}\n"));
    }
    let file_list: Vec<&str> = files
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if !file_list.is_empty() {
        text.push_str("\nfiles:\n");
        for f in &file_list {
            text.push_str(&format!("- {f}\n"));
        }
    }
    RemoteDoc {
        external_id: format!("{slug}:{sha}"),
        canonical_ref: format!("git:{slug}:{sha}"),
        title: subject.to_string(),
        url: url_base.map(|b| format!("{b}{sha}")),
        author: Some(author.to_string()),
        created_at: Some(date.to_string()),
        updated_at: Some(date.to_string()),
        mime: "text/plain",
        kind: "commit",
        container: Some((slug.to_string(), "in_repo")),
        body: text,
        revision: sha.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_urls_derive_from_shaped_remotes_only() {
        assert_eq!(
            commit_url_base("git@github.com:acme/mari.git").as_deref(),
            Some("https://github.com/acme/mari/commit/")
        );
        assert_eq!(
            commit_url_base("https://gitlab.com/acme/mari.git").as_deref(),
            Some("https://gitlab.com/acme/mari/-/commit/")
        );
        assert_eq!(commit_url_base("https://example.com/x.git"), None);
        assert_eq!(commit_url_base(""), None);
    }

    #[test]
    fn commit_doc_includes_message_and_files() {
        let doc = commit_doc(
            "mari",
            Some("https://github.com/a/m/commit/"),
            "abc123",
            "Ana",
            "2026-01-01T00:00:00Z",
            "Fix pricing",
            "Move to $12",
            "docs/pricing.md\nsrc/main.rs\n",
        );
        assert_eq!(doc.external_id, "mari:abc123");
        assert_eq!(doc.kind, "commit");
        assert!(doc.body.contains("Move to $12"));
        assert!(doc.body.contains("- docs/pricing.md"));
        assert_eq!(
            doc.url.as_deref(),
            Some("https://github.com/a/m/commit/abc123")
        );
    }

    #[test]
    fn end_to_end_commit_history_sync_and_cursor() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        let sh = |args: &[&str]| {
            assert!(std::process::Command::new("git")
                .arg("-C")
                .arg(repo)
                .args(args)
                .env("GIT_AUTHOR_NAME", "T")
                .env("GIT_AUTHOR_EMAIL", "t@x")
                .env("GIT_COMMITTER_NAME", "T")
                .env("GIT_COMMITTER_EMAIL", "t@x")
                .status()
                .unwrap()
                .success());
        };
        sh(&["init", "-q"]);
        std::fs::write(repo.join("a.md"), "# a\n").unwrap();
        sh(&["add", "."]);
        sh(&["commit", "-qm", "first commit"]);

        let conn = duckdb::Connection::open_in_memory().unwrap();
        crate::index::ensure_schema(&conn).unwrap();
        let mut stats = SyncStats::default();
        sync_repo(&conn, repo, false, &mut stats).unwrap();
        assert_eq!(stats.changed, 1);
        let kind: String = conn
            .query_row(
                "SELECT kind FROM documents WHERE source_id = 'git'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kind, "commit");

        // Second run: cursor short-circuits, nothing changes.
        let mut stats2 = SyncStats::default();
        sync_repo(&conn, repo, false, &mut stats2).unwrap();
        assert_eq!(stats2.changed, 0);

        // New commit → only the delta is read.
        std::fs::write(repo.join("b.md"), "# b\n").unwrap();
        sh(&["add", "."]);
        sh(&["commit", "-qm", "second commit"]);
        let mut stats3 = SyncStats::default();
        sync_repo(&conn, repo, false, &mut stats3).unwrap();
        assert_eq!(stats3.changed, 1);
    }
}
