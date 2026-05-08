//! Component CLI command
//!

mod compose;
mod init;
mod install;
mod local;
mod publish;
mod registry;
mod run;
mod self_;
mod util;

use clap::{ColorChoice, CommandFactory, Parser};
use clap_verbosity_flag::Verbosity;
use miette::{Context, IntoDiagnostic};
use util::into_miette;

#[derive(Parser)]
#[command(author, version, about, long_about = None, term_width = 80)]
#[command(propagate_version = true)]
pub(crate) struct Cli {
    /// When to use colored output.
    #[arg(
        long,
        value_name = "WHEN",
        default_value = "auto",
        global = true,
        help_heading = "Global Options"
    )]
    color: ColorChoice,

    /// Run in offline mode.
    #[arg(long, global = true, help_heading = "Global Options")]
    offline: bool,

    /// Controls logging verbosity via `-v`/`--verbose` and `-q`/`--quiet`
    /// flags.
    #[command(flatten, next_help_heading = "Global Options")]
    verbosity: Verbosity,

    #[command(subcommand)]
    command: Option<Command>,
}

impl Cli {
    async fn run(self) -> miette::Result<()> {
        match self.command {
            Some(Command::Run(opts)) => opts.run(self.offline).await?,
            Some(Command::Local(opts)) => {
                opts.run();
            }
            Some(Command::Registry(opts)) => opts.run(self.offline).await.map_err(into_miette)?,
            Some(Command::Compose(opts)) => opts.run().map_err(into_miette)?,
            Some(Command::Init(opts)) => opts.run().await?,
            Some(Command::Install(opts)) => opts.run(self.offline).await?,
            Some(Command::Publish(opts)) => opts.run(self.offline).await.map_err(into_miette)?,
            Some(Command::Self_(opts)) => opts.run().await.map_err(into_miette)?,
            None => {
                // Apply the parsed color choice when printing help
                Cli::command()
                    .color(self.color)
                    .print_help()
                    .into_diagnostic()?;
            }
        }
        Ok(())
    }
}

#[derive(clap::Parser)]
enum Command {
    /// Execute a Wasm Component
    Run(run::Opts),
    /// Create a new wasm component in an existing directory
    Init(init::Opts),
    /// Install a dependency from an OCI registry
    Install(install::Opts),
    /// Publish a component or WIT interface to an OCI registry
    Publish(publish::Opts),
    /// Compose Wasm components from WAC scripts
    Compose(compose::Opts),
    /// Detect and manage local WASM files
    #[command(subcommand)]
    Local(local::Opts),
    /// Manage Wasm Components and WIT interfaces in OCI registries
    #[command(subcommand)]
    Registry(registry::Opts),
    /// Configure the `component(1)` tool, generate completions, & manage state
    #[clap(name = "self")]
    #[command(subcommand)]
    Self_(self_::Opts),
}

/// Compute the log directory for the application.
///
/// Uses the XDG state directory (`$XDG_STATE_HOME/wasm/logs`) on Linux,
/// and falls back to the local data directory on other systems.
pub(crate) fn log_dir() -> std::path::PathBuf {
    component_package_manager::storage::StateInfo::default_log_dir()
}

/// Initialize the tracing subscriber with a file appender and a stderr layer.
/// Logs are stored in an XDG-compliant directory.
///
/// The `level` parameter controls the verbosity of both the file and stderr
/// log layers, and is typically derived from `--verbose` / `--quiet` CLI flags.
///
/// The returned `WorkerGuard` must be kept alive for the duration of the
/// program to ensure all buffered log records are flushed.
fn init_tracing(
    level: tracing::level_filters::LevelFilter,
) -> miette::Result<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::Layer;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let log_dir = log_dir();
    std::fs::create_dir_all(&log_dir)
        .into_diagnostic()
        .wrap_err("failed to create log directory")?;

    let file_appender = tracing_appender::rolling::never(&log_dir, "component.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_filter(level);

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(level);

    tracing_subscriber::registry()
        .with(file_layer)
        .with(stderr_layer)
        .init();

    Ok(guard)
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    // Load .env file if present; variables already set in the environment
    // take precedence (system environment is not overridden).
    dotenvy::dotenv().ok();

    // Pre-process argv for `run`: tokens after the first positional
    // are quarantined behind `--` so clap doesn't try to interpret
    // them as host flags. This makes `component run X --help` route
    // `--help` into the dynamic sub-CLI built from X's WIT, while
    // `component run --help` (before any positional) still triggers
    // host help.
    // r[impl run.library-help.dynamic]
    // r[impl run.host-flags-before-input]
    let argv = quarantine_run_trailing_args(std::env::args().collect());
    let cli = Cli::parse_from(argv);
    let _tracing_guard = init_tracing(cli.verbosity.tracing_level_filter())?;
    cli.run().await?;
    Ok(())
}

/// Insert `--` after the first positional argument of the `run`
/// subcommand so that everything after it is treated as guest args
/// by clap (`trailing_var_arg = true`).
///
/// Identifies host flags as `--inherit-env`, `--inherit-network`,
/// `--no-stdio`, `--global`/`-g`, plus value-taking flags `--env`,
/// `--no-stdio`, `--global`/`-g`, plus value-taking flags `--env`,
/// `--dir`, `--listen` (each followed by its value). Global flags
/// (`-v`/`--verbose`, `-q`/`--quiet`, `--offline`, `--color`) that
/// clap allows before subcommand positionals are also recognized.
/// A leading `-h`/`--help` before any positional triggers host help
/// via clap in the usual way.
///
/// If an explicit `--` separator appears before the positional
/// `<INPUT>`, the user has already quarantined the trailing args
/// themselves, so the remainder is passed through unchanged. If an
/// unrecognized dash-prefixed token appears before the positional,
/// the original args are returned verbatim so clap can produce a
/// proper "unknown option" error rather than mistakenly treating
/// the flag as `<INPUT>`.
fn quarantine_run_trailing_args(args: Vec<String>) -> Vec<String> {
    /// Host flags that take no value.
    const VALUELESS: &[&str] = &[
        // `run`-specific flags.
        "--inherit-env",
        "--inherit-network",
        "--no-stdio",
        "--global",
        "-g",
        "-h",
        "--help",
        // Global flags clap allows after the `run` subcommand token.
        "-v",
        "--verbose",
        "-q",
        "--quiet",
        "--offline",
    ];
    /// Host flags that consume the next argument as a value.
    const VALUED: &[&str] = &[
        // `run`-specific flags.
        "--env", "--dir", "--listen",
        // Global flags clap allows after the `run` subcommand token.
        "--color",
    ];

    let Some(run_idx) = args.iter().position(|a| a == "run") else {
        return args;
    };

    let mut out = Vec::with_capacity(args.len() + 1);
    out.extend(args.iter().take(run_idx + 1).cloned());

    let mut i = run_idx + 1;
    while let Some(token) = args.get(i) {
        // Explicit `--` separator: the user has already quarantined the
        // trailing args themselves. Pass everything through unchanged so
        // clap can apply its `trailing_var_arg` semantics.
        if token == "--" {
            if let Some(rest) = args.get(i..) {
                out.extend(rest.iter().cloned());
            }
            return out;
        }
        if VALUELESS.iter().any(|f| f == token) {
            out.push(token.clone());
            i += 1;
            continue;
        }
        if VALUED.iter().any(|f| token == f) {
            out.push(token.clone());
            i += 1;
            if let Some(value) = args.get(i) {
                out.push(value.clone());
                i += 1;
            }
            continue;
        }
        if let Some(rest) = token.strip_prefix("--")
            && VALUED.iter().any(|f| {
                rest.split_once('=')
                    .is_some_and(|(name, _)| format!("--{name}") == **f)
            })
        {
            // `--env=KEY=VAL` form: still a host flag, no quarantine yet.
            out.push(token.clone());
            i += 1;
            continue;
        }
        // Unknown dash-prefixed token before any positional: bail out and
        // let clap produce a proper "unknown option" error rather than
        // mistakenly treating it as the `<INPUT>` positional.
        if token.starts_with('-') {
            return args;
        }
        // First non-host token: this is the positional INPUT.
        out.push(token.clone());
        i += 1;
        // Quarantine the rest after `--` so clap forwards it through
        // `trailing_var_arg`.
        if i < args.len() {
            out.push("--".to_string());
            if let Some(rest) = args.get(i..) {
                out.extend(rest.iter().cloned());
            }
        }
        return out;
    }
    out
}
