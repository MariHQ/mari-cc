mod assets;
mod attn;
mod authcmd;
mod checkcmd;
mod cloud;
mod config;
mod configcmd;
mod connectors;
mod curation;
mod detector;
mod docsite;
mod factcheck;
mod features;
mod hook;
mod i18n;
mod index;
mod initcmd;
mod lineage;
mod narrative;
mod ocr;
mod office;
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
    about = "Local-first knowledge curation and prose quality for AI-assisted teams"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Interactive, assistant-guided setup (search | style | all)
    Init { which: Option<String> },
    /// Workspace, cloud, embedding, per-source, detector and tag status
    Status,
    /// Connect a provider credential
    Auth {
        provider: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        email: Option<String>,
        #[arg(long)]
        subdomain: Option<String>,
        #[arg(long)]
        key: Option<String>,
        #[arg(long)]
        secret: Option<String>,
        #[arg(long)]
        method: Option<String>,
    },
    /// Show or change per-source scope (global | local)
    Scope {
        source: Option<String>,
        scope: Option<String>,
    },
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
    /// Nudges: directed edit obligations
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
    /// Track/untrack refs for a source: mari track <source> add|remove|list [ref]
    Track {
        args: Vec<String>,
        #[arg(long = "list-key")]
        list_key: Option<String>,
    },
    /// Sync tracked sources into the index
    Sync {
        source: Option<String>,
        #[arg(long)]
        rebuild: bool,
        #[arg(long)]
        since: Option<i64>,
    },
    /// Hybrid search over the knowledge base
    Search {
        query: String,
        #[arg(long, num_args = 0..=1, default_missing_value = "4000")]
        full: Option<usize>,
        #[arg(long)]
        variant: Vec<String>,
        #[arg(long)]
        k: Option<usize>,
        #[arg(long)]
        source: Option<String>,
        #[arg(long)]
        doc: Option<String>,
        #[arg(long)]
        author: Option<String>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        before: Option<String>,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long = "no-tag")]
        no_tag: Option<String>,
        #[arg(long)]
        expand: Option<usize>,
        #[arg(long)]
        json: bool,
    },
    /// Skill-facing repository/knowledge explorer
    Explore {
        query_or_file: String,
        #[arg(long)]
        k: Option<usize>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        deep: bool,
        #[arg(long)]
        focus: bool,
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
    /// Most recently changed docs/messages
    Recent {
        #[arg(long)]
        source: Option<String>,
        #[arg(long)]
        doc: Option<String>,
        #[arg(long)]
        author: Option<String>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        before: Option<String>,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long = "no-tag")]
        no_tag: Option<String>,
        #[arg(long, default_value = "20")]
        limit: usize,
        #[arg(long, num_args = 0..=1, default_missing_value = "4000")]
        full: Option<usize>,
        #[arg(long)]
        json: bool,
    },
    /// Full document body for best id/title matches
    Doc {
        r#ref: String,
        #[arg(long)]
        source: Option<String>,
        #[arg(long, num_args = 0..=1, default_missing_value = "0")]
        full: Option<usize>,
    },
    /// Whole thread/conversation as one block
    Thread {
        r#ref: String,
        #[arg(long)]
        source: Option<String>,
        #[arg(long, num_args = 0..=1, default_missing_value = "0")]
        full: Option<usize>,
    },
    /// Chunks surrounding a chunk id in document order
    Neighbors {
        chunk_id: String,
        #[arg(long, default_value = "3")]
        radius: usize,
        #[arg(long, num_args = 0..=1, default_missing_value = "0")]
        full: Option<usize>,
    },
    /// Docs one hop away in the edge graph
    Related {
        r#ref: String,
        #[arg(long)]
        source: Option<String>,
        #[arg(long, default_value = "20")]
        limit: usize,
        #[arg(long, num_args = 0..=1, default_missing_value = "0")]
        full: Option<usize>,
    },
    /// Read-only SQL over the catalog
    Sql {
        query: Option<String>,
        #[arg(long)]
        global: bool,
    },
    /// Team sharing: init | connect | role
    Cloud {
        args: Vec<String>,
        #[arg(long)]
        backend: Option<String>,
        #[arg(long)]
        bucket: Option<String>,
        #[arg(long)]
        prefix: Option<String>,
        #[arg(long)]
        region: Option<String>,
        #[arg(long)]
        force: bool,
    },
    /// Fetch the latest cloud index into the replica
    Pull,
    /// Curation tags: <path-or-ref> <status> | list | remove
    Tag {
        args: Vec<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        status: Option<String>,
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
    /// Facts ledger: list | add
    Facts {
        args: Vec<String>,
        #[arg(long)]
        source: Option<String>,
    },
    /// Agent-assisted fact extraction candidates
    Extract {
        args: Vec<String>,
        #[arg(long)]
        source: Option<String>,
        #[arg(long)]
        doc: Option<String>,
        #[arg(long)]
        since: Option<i64>,
        #[arg(long)]
        json: bool,
    },
    /// Human-facing detector report; `audit kb` audits the knowledge base
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
        models: bool,
        #[arg(long = "slop-spans")]
        slop_spans: bool,
        #[arg(long)]
        grammar: bool,
        #[arg(long = "no-config")]
        no_config: bool,
    },
    /// Vendored humanizer skill management
    Humanize {
        action: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Check a file's claims against ground truth
    Factcheck {
        file: String,
        #[arg(long)]
        source: Option<String>,
        #[arg(long)]
        kb: bool,
        #[arg(long)]
        models: bool,
        #[arg(long)]
        decompose: bool,
        #[arg(long)]
        claims: Option<String>,
        #[arg(long = "emit-claim-targets")]
        emit_claim_targets: bool,
        #[arg(long)]
        deep: bool,
        #[arg(long)]
        ground: Option<String>,
        #[arg(long)]
        threshold: Option<f64>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        quiet: bool,
        #[arg(long)]
        lookback: Option<i64>,
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
        #[arg(long)]
        deep: bool,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        threshold: Option<f64>,
    },
    /// Localization: i18n <file> | conform | coverage
    I18n {
        args: Vec<String>,
        #[arg(long)]
        deep: bool,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        strict: bool,
    },
    /// Alias for i18n localization checks
    Localize {
        args: Vec<String>,
        #[arg(long)]
        deep: bool,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        strict: bool,
    },
    /// Lineage curation: list | add <src>[#sym] <dst>[#sym] | confirm <id> | reject <id>
    Lineage {
        args: Vec<String>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        by: Option<String>,
        #[arg(long)]
        note: Option<String>,
    },
    /// Post-edit hook entry point (called by the agent harness, never breaks the turn)
    Hook { args: Vec<String> },
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
        Cmd::Auth {
            provider,
            token,
            url,
            email,
            subdomain,
            key,
            secret,
            method,
        } => authcmd::run(
            &provider,
            authcmd::AuthFlags {
                token,
                url,
                email,
                subdomain,
                key,
                secret,
                method,
            },
        ),
        Cmd::Scope { source, scope } => authcmd::scope(source.as_deref(), scope.as_deref()),
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
        Cmd::Track { args, list_key } => connectors::track(&args, list_key.as_deref()),
        Cmd::Sync {
            source,
            rebuild,
            since,
        } => index::sync::run(source.as_deref(), rebuild, since),
        Cmd::Search {
            query,
            full,
            variant,
            k,
            source,
            doc,
            author,
            since,
            before,
            tag,
            no_tag,
            expand,
            json,
        } => index::search::run(index::search::SearchArgs {
            query,
            full,
            variants: variant,
            k,
            source,
            doc,
            author,
            since,
            before,
            tag,
            no_tag,
            expand,
            json,
        }),
        Cmd::Explore {
            query_or_file,
            k,
            json,
            deep,
            focus,
        } => surface::explore(&query_or_file, k, json, deep, focus),
        Cmd::Surface { dir, json } => surface::surface(dir.as_deref(), json),
        Cmd::Narrative { action, file, json } => {
            narrative::run(action.as_deref(), file.as_deref(), json)
        }
        Cmd::Recent {
            source,
            doc,
            author,
            since,
            before,
            tag,
            no_tag,
            limit,
            full,
            json,
        } => index::search::recent(
            source.as_deref(),
            doc.as_deref(),
            author.as_deref(),
            since.as_deref(),
            before.as_deref(),
            tag.as_deref(),
            no_tag.as_deref(),
            limit,
            full,
            json,
        ),
        Cmd::Doc {
            r#ref,
            source,
            full,
        } => index::search::doc(&r#ref, source.as_deref(), full),
        Cmd::Thread {
            r#ref,
            source,
            full,
        } => index::search::doc(&r#ref, source.as_deref(), full),
        Cmd::Neighbors {
            chunk_id,
            radius,
            full,
        } => index::search::neighbors(&chunk_id, radius, full),
        Cmd::Related {
            r#ref,
            source,
            limit,
            full,
        } => index::search::related(&r#ref, source.as_deref(), limit, full),
        Cmd::Sql { query, global } => index::sqlcmd(query.as_deref(), global),
        Cmd::Cloud {
            args,
            backend,
            bucket,
            prefix,
            region,
            force,
        } => cloud::run(
            &args,
            backend.as_deref(),
            bucket.as_deref(),
            prefix.as_deref(),
            region.as_deref(),
            force,
        ),
        Cmd::Pull => cloud::pull(),
        Cmd::Tag {
            args,
            note,
            status,
            json,
        } => curation::tag(&args, note.as_deref(), status.as_deref(), json),
        Cmd::Glossary { args, use_, not_ } => {
            curation::glossary(&args, use_.as_deref(), not_.as_deref())
        }
        Cmd::Facts { args, source } => curation::facts(&args, source.as_deref()),
        Cmd::Extract {
            args,
            source,
            doc,
            since,
            json,
        } => curation::extract(&args, source.as_deref(), doc.as_deref(), since, json),
        Cmd::Audit { args, json, strict } => {
            if args.first().map(|s| s.as_str()) == Some("kb") {
                curation::audit_kb(&args[1..], json, strict)
            } else {
                detector::runner::audit(&args, json)
            }
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
            models,
            slop_spans,
            grammar,
            no_config,
        } => detector::runner::cmd_detect(detector::runner::DetectArgs {
            paths,
            stdin,
            json,
            summary,
            score,
            strict,
            quiet,
            style,
            models,
            slop_spans,
            grammar,
            no_config,
        }),
        Cmd::Humanize { action, json } => curation::humanize(action.as_deref(), json),
        Cmd::Factcheck {
            file,
            source,
            kb,
            models,
            decompose,
            claims,
            emit_claim_targets,
            deep,
            ground,
            threshold,
            json,
            strict,
            quiet,
            lookback,
        } => factcheck::run(factcheck::FactcheckArgs {
            file,
            source,
            kb,
            models,
            decompose,
            claims,
            emit_claim_targets,
            deep,
            ground,
            threshold,
            json,
            strict,
            quiet,
            lookback,
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
            deep,
            limit,
            threshold,
        } => checkcmd::run(json, strict, deep, limit, threshold),
        Cmd::I18n {
            args,
            deep,
            limit,
            strict,
        } => i18n::run(&args, deep, limit, strict),
        Cmd::Localize {
            args,
            deep,
            limit,
            strict,
        } => i18n::run(&args, deep, limit, strict),
        Cmd::Lineage {
            args,
            json,
            by,
            note,
        } => lineage::run(&args, json, by.as_deref(), note.as_deref()),
        Cmd::Hook { args } => Ok(hook::run(&args)),
    }
}
