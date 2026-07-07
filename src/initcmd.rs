//! `mari init [search|style|all]` per SPEC §5.1 — assistant-guided setup.
//! The CLI prints everything the assistant (or user) needs; it never blocks
//! on input when stdout is not a TTY.

use crate::{authcmd, config, workspace};
use anyhow::Result;
use serde_json::Value;

struct SourceInfo {
    key: &'static str,
    label: &'static str,
    auth: Option<&'static str>,
    list_keys: &'static [&'static str],
    auto_index: &'static str,
    lookback_key: Option<&'static str>,
    cred_fields: &'static str,
}

const SOURCES: &[SourceInfo] = &[
    SourceInfo {
        key: "slack",
        label: "Slack",
        auth: Some("slack"),
        list_keys: &["slack.channels"],
        auto_index: "all channels the token is a member of",
        lookback_key: Some("slack.lookback_days"),
        cred_fields: "token (xoxp-/xoxb-)",
    },
    SourceInfo {
        key: "gdocs",
        label: "Google Drive",
        auth: Some("google"),
        list_keys: &["google.docs", "google.folders"],
        auto_index: "docs+PDFs the user owns",
        lookback_key: Some("gdocs.lookback_days"),
        cred_fields: "gcloud browser sign-in",
    },
    SourceInfo {
        key: "github",
        label: "GitHub",
        auth: Some("github"),
        list_keys: &["github.repos"],
        auto_index: "none — track ≥1 repo",
        lookback_key: None,
        cred_fields: "token (github_pat_/ghp_)",
    },
    SourceInfo {
        key: "confluence",
        label: "Confluence",
        auth: Some("confluence"),
        list_keys: &["confluence.spaces", "confluence.pages"],
        auto_index: "none — track ≥1 space/page",
        lookback_key: None,
        cred_fields: "url + email + token (Cloud) or url + PAT (Server/DC)",
    },
    SourceInfo {
        key: "jira",
        label: "Jira",
        auth: Some("jira"),
        list_keys: &["jira.projects"],
        auto_index: "none — track ≥1 project",
        lookback_key: None,
        cred_fields: "url + email + token (Cloud) or url + PAT (DC)",
    },
    SourceInfo {
        key: "zendesk",
        label: "Zendesk",
        auth: Some("zendesk"),
        list_keys: &["zendesk.include"],
        auto_index: "tickets + articles once connected",
        lookback_key: None,
        cred_fields: "subdomain + email + token",
    },
    SourceInfo {
        key: "salesforce",
        label: "Salesforce",
        auth: Some("salesforce"),
        list_keys: &["salesforce.objects"],
        auto_index: "Knowledge articles + Cases once connected",
        lookback_key: None,
        cred_fields: "OAuth token + instance url",
    },
    SourceInfo {
        key: "hubspot",
        label: "HubSpot",
        auth: Some("hubspot"),
        list_keys: &["hubspot.include"],
        auto_index: "tickets + notes + KB once connected",
        lookback_key: None,
        cred_fields: "private-app token (pat-…)",
    },
    SourceInfo {
        key: "microsoft",
        label: "Microsoft 365",
        auth: Some("microsoft"),
        list_keys: &["microsoft.drives", "microsoft.mail", "microsoft.teams"],
        auto_index: "none — track ≥1 drive/mail/teams ref",
        lookback_key: None,
        cred_fields: "device-code sign-in",
    },
    SourceInfo {
        key: "discord",
        label: "Discord",
        auth: Some("discord"),
        list_keys: &["discord.channels", "discord.guilds"],
        auto_index: "none — track ≥1 channel/guild",
        lookback_key: Some("discord.lookback_days"),
        cred_fields: "bot token (Message Content intent)",
    },
    SourceInfo {
        key: "linear",
        label: "Linear",
        auth: Some("linear"),
        list_keys: &["linear.teams", "linear.projects"],
        auto_index: "none — track ≥1 team",
        lookback_key: None,
        cred_fields: "personal API key",
    },
    SourceInfo {
        key: "git",
        label: "Git history",
        auth: None,
        list_keys: &["git.repos"],
        auto_index: "the cwd repo",
        lookback_key: None,
        cred_fields: "none",
    },
    SourceInfo {
        key: "localfiles",
        label: "Local files",
        auth: None,
        list_keys: &["localfiles.paths"],
        auto_index: "none — track ≥1 path",
        lookback_key: None,
        cred_fields: "none",
    },
];

pub fn run(which: Option<&str>) -> Result<i32> {
    match which.unwrap_or("all") {
        "search" => init_search(),
        "style" => init_style(),
        "all" => {
            init_search()?;
            println!();
            init_style()
        }
        other => {
            eprintln!("unknown init target: {other} (search | style | all)");
            Ok(2)
        }
    }
}

fn tracked_count(cfg: &Value, keys: &[&str]) -> usize {
    keys.iter()
        .filter_map(|k| config::get_path(cfg, k))
        .filter_map(|v| v.as_array())
        .map(|a| a.len())
        .sum()
}

fn init_search() -> Result<i32> {
    let root = workspace::work_root();
    let cfg = config::resolve(Some(&root));
    println!("Mari knowledge sources — connection status\n");
    for s in SOURCES {
        let scope = workspace::source_scope(s.key);
        let connected = match s.auth {
            None => true,
            Some(p) => authcmd::credential(p).is_some(),
        };
        let mark = if connected { "[x]" } else { "[ ]" };
        println!("{mark} {}  (key: {}, scope: {scope})", s.label, s.key);
        match s.auth {
            None => println!("      auth: none needed"),
            Some(p) if connected => println!(
                "      connected (credential: {})",
                authcmd::credential_path(p).display()
            ),
            Some(p) => {
                println!("      connect with: {}", auth_command_template(p));
                println!(
                    "      credential file: {}",
                    authcmd::credential_path(p).display()
                );
            }
        }
        println!("      required fields: {}", s.cred_fields);
        println!(
            "      config: {} — list keys: {} (tracked: {})",
            config::repo_config_path(&root).display(),
            s.list_keys.join(", "),
            tracked_count(&cfg, s.list_keys)
        );
        println!("      auto-index: {}", s.auto_index);
        if let Some(lb) = s.lookback_key {
            if let Some(v) = config::get_path(&cfg, lb) {
                println!("      lookback: {v} days ({lb})");
            }
        }
    }
    println!("\nScopes: `global` shares one index across all repos (~/.mari/_global); `local` is per-repo.");
    println!("Change with: mari scope <source> <global|local>\n");
    println!("Credential handling — three paths:");
    println!("  1. The assistant runs `mari auth <provider> --token …` for you.");
    println!("  2. You run the same command yourself (type `! mari auth …`).");
    println!("  3. You write the credential file directly at the path shown above (mode 0600).");
    Ok(0)
}

fn auth_command_template(provider: &str) -> String {
    match provider {
        "slack" => "mari auth slack --token <xoxp-or-xoxb-token>".into(),
        "google" => "mari auth google".into(),
        "github" => "mari auth github --token <github_pat-or-ghp-token>".into(),
        "confluence" => {
            "mari auth confluence --url <url> --email <email> --token <api-token>".into()
        }
        "jira" => "mari auth jira --url <url> --email <email> --token <api-token>".into(),
        "zendesk" => {
            "mari auth zendesk --subdomain <subdomain> --email <email> --token <api-token>".into()
        }
        "salesforce" => "mari auth salesforce --url <instance-url> --token <oauth-token>".into(),
        "hubspot" => "mari auth hubspot --token <pat-token>".into(),
        "microsoft" => "mari auth microsoft".into(),
        "discord" => "mari auth discord --token <bot-token>".into(),
        "linear" => "mari auth linear --token <personal-api-key>".into(),
        other => format!("mari auth {other}"),
    }
}

fn init_style() -> Result<i32> {
    let root = workspace::work_root();
    let product = root.join("PRODUCT.md");
    let style = root.join("STYLE.md");
    println!("Mari editorial setup\n");
    if product.exists() {
        println!("[x] PRODUCT.md exists — editorial context is configured.");
    } else {
        println!("[ ] PRODUCT.md missing. This one-time setup is assistant-guided:");
        println!("    1. Ask the team's register (docs | marketing | editorial | microcopy) and base style guide");
        println!("       (microsoft | google | ap | chicago | plain — sets detector.styleGuide).");
        println!("    2. Sample existing writing for voice (read 2–3 representative files).");
        println!("    3. Write PRODUCT.md: audience, register, voice, banned words, reading-grade target.");
    }
    if style.exists() {
        println!("[x] STYLE.md exists.");
    } else {
        println!("[ ] STYLE.md missing — offer to create it: base guide, Terminology table (Use/Not), formatting rules, forbidden phrasings.");
    }
    let hook_installed = root.join(".claude").join("settings.json").exists()
        && std::fs::read_to_string(root.join(".claude").join("settings.json"))
            .map(|s| s.contains("mari hook"))
            .unwrap_or(false);
    if hook_installed {
        println!("[x] post-edit hook installed.");
    } else {
        println!("[ ] post-edit hook not installed — offer `mari hooks on`.");
    }
    println!("[ ] offer `mari rules discover` to propose code↔docs edit-notify rules.");
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::{auth_command_template, SOURCES};

    #[test]
    fn auth_command_templates_are_concrete() {
        assert_eq!(
            auth_command_template("github"),
            "mari auth github --token <github_pat-or-ghp-token>"
        );
        assert_eq!(
            auth_command_template("zendesk"),
            "mari auth zendesk --subdomain <subdomain> --email <email> --token <api-token>"
        );
        assert_eq!(auth_command_template("google"), "mari auth google");
        assert_eq!(auth_command_template("microsoft"), "mari auth microsoft");
    }

    #[test]
    fn source_display_order_puts_git_after_cloud_sources_and_localfiles_last() {
        let keys = SOURCES.iter().map(|s| s.key).collect::<Vec<_>>();

        assert_eq!(
            keys,
            vec![
                "slack",
                "gdocs",
                "github",
                "confluence",
                "jira",
                "zendesk",
                "salesforce",
                "hubspot",
                "microsoft",
                "discord",
                "linear",
                "git",
                "localfiles"
            ]
        );
    }
}
