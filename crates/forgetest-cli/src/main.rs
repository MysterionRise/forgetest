//! forgetest CLI — the user-facing command-line interface.

use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser)]
#[command(name = "forgetest", version, about = "LLM code-quality eval harness")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run evaluations
    Run {
        /// Path to .toml eval set or directory
        #[arg(long)]
        eval_set: PathBuf,

        /// Models to evaluate (e.g. "anthropic/claude-sonnet-4-20250514,openai/gpt-4.1")
        #[arg(long)]
        models: Option<String>,

        /// Pass@k values (comma-separated, default: "1")
        #[arg(long, default_value = "1")]
        pass_k: String,

        /// Max concurrent evals
        #[arg(long, default_value = "4")]
        parallelism: usize,

        /// Generation temperature
        #[arg(long, default_value = "0.0")]
        temperature: f64,

        /// Output directory
        #[arg(long, default_value = "./forgetest-results")]
        output: PathBuf,

        /// Output format: json, html, sarif, all
        #[arg(long, default_value = "json")]
        format: String,

        /// Filter by tags
        #[arg(long)]
        filter: Option<String>,

        /// Config file path
        #[arg(long)]
        config: Option<PathBuf>,
    },

    /// Compare two eval reports
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
    },

    /// Validate eval set TOML files
    Validate {
        /// Path to eval set file or directory
        #[arg(long)]
        eval_set: PathBuf,
    },

    /// List available models
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
            models,
            pass_k,
            parallelism,
            temperature,
            output,
            format,
            filter,
            config,
        } => {
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
            )
            .await
        }
        Commands::Compare {
            baseline,
            current,
            threshold,
            fail_on_regression,
            format,
        } => commands::compare::execute(baseline, current, threshold, fail_on_regression, format),
        Commands::Validate { eval_set } => commands::validate::execute(eval_set),
        Commands::ListModels { provider, config } => {
            commands::list_models::execute(provider, config)
        }
        Commands::Init => commands::init::execute(),
    };

    if let Err(e) = result {
        eprintln!("Error: {e:#}");
        process::exit(1);
    }
}
