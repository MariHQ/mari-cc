#![allow(clippy::too_many_arguments)]
mod assets;
mod checkcmd;
mod config;
mod configcmd;
mod console;
mod curation;
mod detector;
mod docsite;
mod features;
mod hook;
mod i18n;
mod initcmd;
mod narrative;
mod platform;
mod rulescmd;
mod statuscmd;
mod surface;
mod workspace;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "mari",
    version,
    about = "Deterministic prose quality for AI-assisted teams"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Interactive, assistant-guided editorial setup
    Init { which: Option<String> },
    /// Workspace, detector, and hook status
    Status,
    /// get PATH | set PATH VALUE | list
    Config {
        action: Option<String>,
        path: Option<String>,
        value: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Self-description capability catalog
    Features {
        #[arg(long)]
        json: bool,
    },
    /// Hook management + hook-scoped waivers
    Hooks {
        args: Vec<String>,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Detector waivers (committed .mari/config.json)
    Ignores {
        args: Vec<String>,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Zero-tolerance rule list
    Zero { args: Vec<String> },
    /// Edit-notify rules: list | discover | add | remove
    Rules {
        args: Vec<String>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        write: bool,
        #[arg(long)]
        paths: Option<String>,
        #[arg(long)]
        notify: Option<String>,
        #[arg(long)]
        exclude: Option<String>,
    },
    /// Nudges: list | add | remove | check
    Nudge {
        args: Vec<String>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        when: Option<String>,
        #[arg(long = "edit")]
        edit: Vec<String>,
        #[arg(long)]
        message: Option<String>,
        #[arg(long)]
        exclude: Option<String>,
    },
    /// Extract public API/docs/config surface
    Surface {
        dir: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Whole-document narrative questionnaire and score
    Narrative {
        action: Option<String>,
        file: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Glossary: harvest | list | add
    Glossary {
        args: Vec<String>,
        #[arg(long = "use")]
        use_: Option<String>,
        #[arg(long = "not")]
        not_: Option<String>,
    },
    /// Human-facing detector report
    Audit {
        args: Vec<String>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        strict: bool,
    },
    /// The deterministic detector
    Detect {
        paths: Vec<String>,
        #[arg(long)]
        stdin: bool,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        summary: bool,
        #[arg(long)]
        score: bool,
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        quiet: bool,
        #[arg(long)]
        style: Option<String>,
        #[arg(long)]
        grammar: bool,
        #[arg(long = "no-config")]
        no_config: bool,
        /// Extract and lint user-facing copy from code under <dir> (JSX/TSX text, string literals)
        #[arg(long)]
        strings: Option<String>,
        /// Treat each input line as its own unit (nav titles, menu labels, stdin label lists)
        #[arg(long)]
        labels: bool,
    },
    /// Document archetypes: detect | check | scaffold
    Asset {
        args: Vec<String>,
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        force: bool,
    },
    /// Doc-platform detection and scaffolding
    Platform {
        args: Vec<String>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        force: bool,
    },
    /// Docsite flow plan and readiness status
    Docsite {
        args: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// Whole-project docs validation
    Check {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        strict: bool,
        /// Validate in-page #anchor→id links in HTML/JSX (code-based sites)
        #[arg(long)]
        anchors: bool,
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Localization: i18n <file> | conform
    I18n {
        args: Vec<String>,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        strict: bool,
    },
    /// Alias for i18n localization checks
    Localize {
        args: Vec<String>,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        strict: bool,
    },
    /// Report which optional external tools are available
    Doctor,
    /// Post-edit hook entry point (called by the agent harness, never breaks the turn)
    Hook { args: Vec<String> },
    /// Launch the local prose-quality console
    Console {
        #[arg(long)]
        port: Option<u16>,
        /// Open the console in your default browser
        #[arg(long)]
        open: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    let code = run(cli).unwrap_or_else(|e| {
        eprintln!("✗ {e:#}");
        1
    });
    std::process::exit(code);
}

fn run(cli: Cli) -> anyhow::Result<i32> {
    match cli.cmd {
        Cmd::Init { which } => initcmd::run(which.as_deref()),
        Cmd::Status => statuscmd::run(),
        Cmd::Config {
            action,
            path,
            value,
            json,
        } => configcmd::run(action.as_deref(), path.as_deref(), value.as_deref(), json),
        Cmd::Features { json } => features::run(json),
        Cmd::Hooks { args, reason } => rulescmd::hooks(&args, reason.as_deref()),
        Cmd::Ignores { args, reason } => rulescmd::ignores(&args, reason.as_deref()),
        Cmd::Zero { args } => rulescmd::zero(&args),
        Cmd::Rules {
            args,
            json,
            write,
            paths,
            notify,
            exclude,
        } => rulescmd::rules(
            &args,
            json,
            write,
            paths.as_deref(),
            notify.as_deref(),
            exclude.as_deref(),
        ),
        Cmd::Nudge {
            args,
            json,
            when,
            edit,
            message,
            exclude,
        } => rulescmd::nudge(
            &args,
            json,
            when.as_deref(),
            &edit,
            message.as_deref(),
            exclude.as_deref(),
        ),
        Cmd::Surface { dir, json } => surface::surface(dir.as_deref(), json),
        Cmd::Narrative { action, file, json } => {
            narrative::run(action.as_deref(), file.as_deref(), json)
        }
        Cmd::Glossary { args, use_, not_ } => {
            curation::glossary(&args, use_.as_deref(), not_.as_deref())
        }
        Cmd::Audit { args, json, strict } => {
            let _ = strict;
            detector::runner::audit(&args, json)
        }
        Cmd::Detect {
            paths,
            stdin,
            json,
            summary,
            score,
            strict,
            quiet,
            style,
            grammar,
            no_config,
            strings,
            labels,
        } => detector::runner::cmd_detect(detector::runner::DetectArgs {
            paths,
            stdin,
            json,
            summary,
            score,
            strict,
            quiet,
            style,
            grammar,
            no_config,
            strings,
            labels,
        }),
        Cmd::Asset {
            args,
            strict,
            force,
        } => assets::run(&args, strict, force),
        Cmd::Platform {
            args,
            json,
            name,
            force,
        } => platform::run(&args, json, name.as_deref(), force),
        Cmd::Docsite { args, json } => docsite::run(&args, json),
        Cmd::Check {
            json,
            strict,
            anchors,
            limit,
        } => checkcmd::run(json, strict, anchors, limit),
        Cmd::I18n {
            args,
            limit,
            strict,
        } => i18n::run(&args, limit, strict),
        Cmd::Localize {
            args,
            limit,
            strict,
        } => i18n::run(&args, limit, strict),
        Cmd::Doctor => statuscmd::doctor(),
        Cmd::Hook { args } => Ok(hook::run(&args)),
        Cmd::Console { port, open } => console::run(port, open),
    }
}
