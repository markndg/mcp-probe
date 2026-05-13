use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use mcp_test_core::{run_suite, ConnectOptions, SuiteFile};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "mcp-test", version, about = "Test and validate MCP servers")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a JSON suite file against a server command.
    Run {
        /// Path to suite JSON (`version`, `server`, `scenarios`).
        #[arg(long)]
        config: PathBuf,
        /// Per-request timeout in milliseconds.
        #[arg(long, default_value_t = 5000_u64)]
        timeout_ms: u64,
        /// Optional path to write a JSON report.
        #[arg(long)]
        report: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run {
            config,
            timeout_ms,
            report,
        } => {
            let data = fs::read(&config)
                .with_context(|| format!("failed to read suite file {}", config.display()))?;
            let suite = SuiteFile::from_json_slice(&data).context("failed to parse suite JSON")?;
            if suite.version != 1 {
                anyhow::bail!("unsupported suite version: {}", suite.version);
            }
            let options = ConnectOptions {
                response_timeout: Duration::from_millis(timeout_ms),
                ..ConnectOptions::default()
            };
            let outcome = run_suite(&suite, &options).context("suite execution failed")?;
            let text =
                serde_json::to_string_pretty(&outcome).context("failed to serialize report")?;
            if let Some(path) = report {
                fs::write(&path, &text)
                    .with_context(|| format!("failed to write report {}", path.display()))?;
            }
            println!("{text}");
            if !outcome.passed {
                std::process::exit(1);
            }
        }
    }
    Ok(())
}
