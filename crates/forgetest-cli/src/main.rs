//! forgetest CLI — the user-facing command-line interface.

use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser)]
#[command(
    name = "forgetest",
    version,
    about = "Execution-backed Rust coding-agent evaluation"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a snippet eval set or repository-agent suite
    Run {
        /// Path to .toml eval set or directory
        #[arg(long, conflicts_with = "suite", required_unless_present = "suite")]
        eval_set: Option<PathBuf>,

        /// Path to a repository suite.toml
        #[arg(
            long,
            conflicts_with = "eval_set",
            required_unless_present = "eval_set"
        )]
        suite: Option<PathBuf>,

        /// Exact provider/model IDs to evaluate (e.g. "anthropic/MODEL,openai/MODEL")
        #[arg(long)]
        models: Option<String>,

        /// Coding agents for repository suites (e.g. "codex/model,claude/model")
        #[arg(long)]
        agents: Option<String>,

        /// Independent trials per repository task and agent
        #[arg(long, default_value = "3")]
        trials: u32,

        /// Execution profile: development or benchmark
        #[arg(long, default_value = "development")]
        profile: String,

        /// Immutable benchmark lock file
        #[arg(long)]
        benchmark_lock: Option<PathBuf>,

        /// Maximum agent output retained per trial
        #[arg(long, default_value = "4194304")]
        max_agent_output_bytes: usize,

        /// Total wall-clock budget for agent attempts in one trial
        #[arg(long)]
        agent_timeout_secs: Option<u64>,

        /// Maximum agent-reported input plus output tokens per trial
        #[arg(long)]
        max_agent_tokens: Option<u64>,

        /// Maximum agent-reported cost in USD per trial
        #[arg(long)]
        max_agent_cost_usd: Option<f64>,

        /// Retries after an agent process error or non-zero exit
        #[arg(long, default_value = "0")]
        agent_retries: u32,

        /// Pass@k values (comma-separated, default: "1")
        #[arg(long, default_value = "1")]
        pass_k: String,

        /// Max concurrent evals
        #[arg(long)]
        parallelism: Option<usize>,

        /// Generation temperature
        #[arg(long)]
        temperature: Option<f64>,

        /// Output directory
        #[arg(long)]
        output: Option<PathBuf>,

        /// Output format: json, html, sarif, all
        #[arg(long, default_value = "json")]
        format: String,

        /// Filter by tags
        #[arg(long)]
        filter: Option<String>,

        /// Config file path
        #[arg(long)]
        config: Option<PathBuf>,

        /// Runner to use: local or docker
        #[arg(long)]
        runner: Option<String>,
    },

    /// Compare compatible snippet or repository reports
    Compare {
        /// Baseline report JSON
        #[arg(long)]
        baseline: PathBuf,

        /// Current report JSON
        #[arg(long)]
        current: PathBuf,

        /// Regression threshold
        #[arg(long, default_value = "0.05")]
        threshold: f64,

        /// Exit code 1 if regressions found
        #[arg(long)]
        fail_on_regression: bool,

        /// Output format: text, json, markdown
        #[arg(long, default_value = "text")]
        format: String,

        /// Compare mismatched v2 evidence as explicitly non-gating
        #[arg(long)]
        allow_incomparable: bool,
    },

    /// Validate a snippet eval set or repository suite
    Validate {
        /// Path to eval set file or directory
        #[arg(long, conflicts_with = "suite", required_unless_present = "suite")]
        eval_set: Option<PathBuf>,

        /// Path to a repository suite.toml
        #[arg(
            long,
            conflicts_with = "eval_set",
            required_unless_present = "eval_set"
        )]
        suite: Option<PathBuf>,

        /// Run trusted local null- and reference-patch controls
        #[arg(long, requires = "suite")]
        calibrate: bool,
    },

    /// List live Ollama models or informational provider catalog entries
    ListModels {
        /// Filter to specific provider
        #[arg(long)]
        provider: Option<String>,

        /// Config file path
        #[arg(long)]
        config: Option<PathBuf>,
    },

    /// Create starter config and example eval set
    Init,

    /// Run a deterministic offline demo without API keys
    Demo {
        /// Output directory
        #[arg(long, default_value = "./forgetest-results")]
        output: PathBuf,

        /// Output format: json, html, sarif, all
        #[arg(long, default_value = "all")]
        format: String,

        /// Runner to use: local or docker
        #[arg(long, default_value = "local")]
        runner: String,

        /// Demo mode: snippet or repository
        #[arg(long, default_value = "snippet")]
        mode: String,
    },

    /// Create publication-safe artifacts from a private v2 report
    Redact {
        /// Private repository report JSON
        #[arg(long)]
        input: PathBuf,

        /// Output directory for sanitized artifacts
        #[arg(long)]
        output: PathBuf,

        /// Output format: json, html, sarif, all
        #[arg(long, default_value = "all")]
        format: String,
    },

    /// Inspect external coding-agent installations
    Agents {
        #[command(subcommand)]
        command: AgentCommands,
    },

    /// Import or export the constrained Harbor Rust-task subset
    Harbor {
        #[command(subcommand)]
        command: HarborCommands,
    },
}

#[derive(Subcommand)]
enum AgentCommands {
    /// Check executable, version, binary hash, and credential presence
    Doctor {
        /// Agents and exact models (e.g. codex/model,claude/model)
        #[arg(long)]
        agents: Option<String>,

        /// Also verify all immutable images in a benchmark lock
        #[arg(long)]
        benchmark_lock: Option<PathBuf>,
    },

    /// Inspect immutable images and write a benchmark lock
    Lock {
        /// Repository suite manifest
        #[arg(long)]
        suite: PathBuf,

        /// Agent as NAME/MODEL=IMAGE@sha256:DIGEST (repeatable)
        #[arg(long = "agent", required = true)]
        agents: Vec<String>,

        /// Optional effort as NAME=VALUE (repeatable)
        #[arg(long = "effort")]
        efforts: Vec<String>,

        /// Immutable verifier image
        #[arg(long)]
        verifier_image: String,

        /// Lock file to create
        #[arg(long, default_value = "benchmark.lock.toml")]
        output: PathBuf,

        /// Replace an existing lock file
        #[arg(long)]
        force: bool,

        /// Trials per task and agent
        #[arg(long, default_value = "3")]
        trials: u32,

        /// Maximum concurrent trials
        #[arg(long)]
        parallelism: Option<usize>,

        /// Total wall-clock budget for agent attempts in one trial
        #[arg(long)]
        agent_timeout_secs: Option<u64>,

        /// Maximum agent output retained per trial
        #[arg(long, default_value = "4194304")]
        max_agent_output_bytes: usize,

        /// Maximum agent-reported input plus output tokens per trial
        #[arg(long)]
        max_agent_tokens: Option<u64>,

        /// Maximum agent-reported cost in USD per trial
        #[arg(long)]
        max_agent_cost_usd: Option<f64>,

        /// Retries after an agent process error or non-zero exit
        #[arg(long, default_value = "0")]
        agent_retries: u32,

        /// Trusted config file path
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum HarborCommands {
    /// Export a repository suite to Harbor task directories
    Export {
        #[arg(long)]
        suite: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        base_image: String,
    },
    /// Import a forgetest-marked Harbor task
    Import {
        #[arg(long)]
        task: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        suite_id: String,
        #[arg(long)]
        suite_name: String,
        #[arg(long)]
        source_url: String,
        #[arg(long)]
        source_revision: String,
        #[arg(long)]
        license: String,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("forgetest=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Run {
            eval_set,
            suite,
            models,
            agents,
            trials,
            profile,
            benchmark_lock,
            max_agent_output_bytes,
            agent_timeout_secs,
            max_agent_tokens,
            max_agent_cost_usd,
            agent_retries,
            pass_k,
            parallelism,
            temperature,
            output,
            format,
            filter,
            config,
            runner,
        } => {
            if let Some(eval_set) = eval_set {
                commands::run::execute(
                    eval_set,
                    models,
                    pass_k,
                    parallelism,
                    temperature,
                    output,
                    format,
                    filter,
                    config,
                    runner,
                )
                .await
            } else {
                commands::repository_run::execute(commands::repository_run::RepositoryRunOptions {
                    suite_path: suite.expect("clap requires --suite or --eval-set"),
                    agents,
                    trials,
                    profile,
                    benchmark_lock_path: benchmark_lock,
                    max_agent_output_bytes,
                    agent_timeout_secs,
                    max_agent_tokens,
                    max_agent_cost_usd,
                    agent_retries,
                    parallelism,
                    output,
                    format,
                    config_path: config,
                    runner_override: runner,
                })
                .await
            }
        }
        Commands::Compare {
            baseline,
            current,
            threshold,
            fail_on_regression,
            format,
            allow_incomparable,
        } => commands::compare::execute(
            baseline,
            current,
            threshold,
            fail_on_regression,
            format,
            allow_incomparable,
        ),
        Commands::Validate {
            eval_set,
            suite,
            calibrate,
        } => commands::validate::execute(eval_set, suite, calibrate).await,
        Commands::ListModels { provider, config } => {
            commands::list_models::execute(provider, config).await
        }
        Commands::Init => commands::init::execute(),
        Commands::Demo {
            output,
            format,
            runner,
            mode,
        } => commands::demo::execute(output, format, runner, mode).await,
        Commands::Redact {
            input,
            output,
            format,
        } => commands::redact::execute(input, output, format),
        Commands::Agents { command } => match command {
            AgentCommands::Doctor {
                agents,
                benchmark_lock,
            } => commands::agents::doctor(agents, benchmark_lock).await,
            AgentCommands::Lock {
                suite,
                agents,
                efforts,
                verifier_image,
                output,
                force,
                trials,
                parallelism,
                agent_timeout_secs,
                max_agent_output_bytes,
                max_agent_tokens,
                max_agent_cost_usd,
                agent_retries,
                config,
            } => {
                commands::agents::lock(
                    suite,
                    agents,
                    efforts,
                    verifier_image,
                    output,
                    force,
                    trials,
                    parallelism,
                    agent_timeout_secs,
                    max_agent_output_bytes,
                    max_agent_tokens,
                    max_agent_cost_usd,
                    agent_retries,
                    config,
                )
                .await
            }
        },
        Commands::Harbor { command } => match command {
            HarborCommands::Export {
                suite,
                output,
                base_image,
            } => commands::harbor::export(suite, output, base_image),
            HarborCommands::Import {
                task,
                output,
                suite_id,
                suite_name,
                source_url,
                source_revision,
                license,
            } => commands::harbor::import(
                task,
                output,
                suite_id,
                suite_name,
                source_url,
                source_revision,
                license,
            ),
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {e:#}");
        process::exit(1);
    }
}
