//! Catalog of Mari's prose-quality capabilities.

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
                cap("initialize repository config", "mari init [style|all]"),
                cap("show repository status", "mari status"),
                cap("inspect or set config", "mari config"),
                cap("capability catalog", "mari features [--json]"),
                cap("local web console", "mari console"),
            ],
        },
        Group {
            intent: "prose quality",
            capabilities: vec![
                cap("deterministic detector", "mari detect"),
                cap("human audit report", "mari audit <file>"),
                cap(
                    "narrative questionnaire",
                    "mari narrative <questions|score>",
                ),
                cap("style waivers", "mari ignores"),
                cap("zero-tolerance rules", "mari zero"),
                cap("grammar pass", "mari detect --grammar"),
                cap("glossary", "mari glossary"),
            ],
        },
        Group {
            intent: "maintenance",
            capabilities: vec![
                cap("hooks", "mari hooks"),
                cap("post-edit hook entry", "mari hook run"),
                cap("edit-notify rules", "mari rules"),
                cap("nudges", "mari nudge <list|add|remove|check>"),
                cap("localization checks", "mari i18n / mari localize"),
                cap("whole-project validation", "mari check"),
            ],
        },
        Group {
            intent: "documentation",
            capabilities: vec![
                cap("public surface extraction", "mari surface [dir]"),
                cap("document archetypes", "mari asset"),
                cap("doc platform detection", "mari platform"),
            ],
        },
    ]
}

fn cap(capability: &'static str, command: &'static str) -> Capability {
    Capability {
        capability,
        command,
        status: "implemented",
    }
}

#[cfg(test)]
mod tests {
    use super::catalog;

    #[test]
    fn catalog_matches_available_commands() {
        let commands = catalog()
            .iter()
            .flat_map(|group| &group.capabilities)
            .map(|cap| cap.command)
            .collect::<Vec<_>>();
        assert!(commands.contains(&"mari detect"));
        assert!(commands.contains(&"mari config"));
        assert!(commands.contains(&"mari console"));
    }
}
