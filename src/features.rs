//! Capability catalog (SPEC §5.1).

use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
struct Group {
    intent: &'static str,
    capabilities: Vec<Capability>,
}

#[derive(Serialize)]
struct Capability {
    capability: &'static str,
    command: &'static str,
    status: &'static str,
}

pub fn run(json: bool) -> Result<i32> {
    let groups = catalog();
    if json {
        println!("{}", serde_json::to_string_pretty(&groups)?);
        return Ok(0);
    }

    for group in groups {
        println!("{}", group.intent);
        for cap in group.capabilities {
            println!(
                "  {:<28} {:<32} {}",
                cap.capability, cap.command, cap.status
            );
        }
    }
    Ok(0)
}

fn catalog() -> Vec<Group> {
    vec![
        Group {
            intent: "setup",
            capabilities: vec![
                cap(
                    "initialize workspace config",
                    "mari init [search|style|all]",
                    "implemented",
                ),
                cap("show workspace status", "mari status", "implemented"),
                cap("inspect/set config", "mari config", "implemented"),
                cap(
                    "provider credentials",
                    "mari auth <provider>",
                    "implemented",
                ),
                cap(
                    "source scope",
                    "mari scope [source] [global|local]",
                    "implemented",
                ),
                cap(
                    "capability catalog",
                    "mari features [--json]",
                    "implemented",
                ),
            ],
        },
        Group {
            intent: "knowledge",
            capabilities: vec![
                cap(
                    "track source refs",
                    "mari track <source> add|remove|list [--list-key]",
                    "implemented",
                ),
                cap("sync catalog sources", "mari sync [source]", "implemented"),
                cap(
                    "hybrid keyword search",
                    "mari search <query>",
                    "implemented",
                ),
                cap(
                    "repository explorer",
                    "mari explore <question-or-file>",
                    "deterministic surface",
                ),
                cap(
                    "public surface extraction",
                    "mari surface [dir]",
                    "implemented",
                ),
                cap("recent documents", "mari recent", "implemented"),
                cap("document lookup", "mari doc <ref>", "implemented"),
                cap("thread lookup", "mari thread <ref>", "implemented"),
                cap(
                    "neighbor chunks",
                    "mari neighbors <chunk-id>",
                    "implemented",
                ),
                cap("related documents", "mari related <ref>", "implemented"),
                cap(
                    "read-only catalog SQL",
                    "mari sql \"SELECT ...\"",
                    "implemented",
                ),
            ],
        },
        Group {
            intent: "curation",
            capabilities: vec![
                cap("tags", "mari tag", "implemented"),
                cap("glossary", "mari glossary", "implemented"),
                cap("facts ledger", "mari facts", "implemented"),
                cap("fact extraction candidates", "mari extract", "implemented"),
                cap("knowledge-base audit", "mari audit kb", "implemented"),
                cap(
                    "fact checking",
                    "mari factcheck <file>",
                    "deterministic surface",
                ),
                cap("lineage curation", "mari lineage <list|add|confirm|reject>", "implemented"),
            ],
        },
        Group {
            intent: "prose quality",
            capabilities: vec![
                cap("deterministic detector", "mari detect", "implemented"),
                cap("human audit report", "mari audit <file>", "implemented"),
                cap(
                    "narrative questionnaire",
                    "mari narrative <questions|score>",
                    "implemented",
                ),
                cap("style waivers", "mari ignores", "implemented"),
                cap("zero-tolerance rules", "mari zero", "implemented"),
                cap("grammar pass", "mari detect --grammar", "feature-gated"),
                cap("ML/deep tiers", "mari detect --models", "not in this build"),
            ],
        },
        Group {
            intent: "maintenance",
            capabilities: vec![
                cap("hooks", "mari hooks", "implemented"),
                cap("post-edit hook entry", "mari hook run", "implemented"),
                cap("edit-notify rules", "mari rules", "implemented"),
                cap("nudges", "mari nudge", "implemented"),
                cap(
                    "i18n structure checks",
                    "mari i18n / mari localize",
                    "implemented",
                ),
                cap(
                    "whole-project validation",
                    "mari check",
                    "deterministic surface",
                ),
            ],
        },
        Group {
            intent: "publishing",
            capabilities: vec![
                cap("asset archetypes", "mari asset", "deterministic surface"),
                cap(
                    "doc platform detection",
                    "mari platform",
                    "deterministic surface",
                ),
                cap(
                    "docsite flow plan/status",
                    "mari docsite <plan|status>",
                    "deterministic surface",
                ),
                cap(
                    "git-backed cloud sharing",
                    "mari cloud init|connect --backend git [--force]",
                    "implemented",
                ),
                cap(
                    "s3-backed cloud sharing",
                    "mari cloud init|connect --bucket B [--force]",
                    "implemented",
                ),
                cap("cloud pull", "mari pull", "implemented"),
                cap("humanizer vendoring", "mari humanize update", "implemented"),
            ],
        },
    ]
}

fn cap(capability: &'static str, command: &'static str, status: &'static str) -> Capability {
    Capability {
        capability,
        command,
        status,
    }
}

#[cfg(test)]
mod tests {
    use super::catalog;

    #[test]
    fn sync_capability_is_not_limited_to_local_sources() {
        let groups = catalog();
        let sync = groups
            .iter()
            .flat_map(|group| &group.capabilities)
            .find(|cap| cap.capability == "sync catalog sources")
            .expect("sync capability");

        assert_eq!(sync.command, "mari sync [source]");
        assert_eq!(sync.status, "implemented");
    }

    #[test]
    fn read_capabilities_include_thread_alias() {
        let groups = catalog();
        let thread = groups
            .iter()
            .flat_map(|group| &group.capabilities)
            .find(|cap| cap.capability == "thread lookup")
            .expect("thread capability");

        assert_eq!(thread.command, "mari thread <ref>");
        assert_eq!(thread.status, "implemented");
    }

    #[test]
    fn cloud_init_capabilities_show_force_safety_flag() {
        let groups = catalog();
        let commands = groups
            .iter()
            .flat_map(|group| &group.capabilities)
            .filter(|cap| cap.capability.contains("cloud sharing"))
            .map(|cap| cap.command)
            .collect::<Vec<_>>();

        assert!(commands.iter().all(|command| command.contains("[--force]")));
    }
}
