mod fuzz;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use mcp_test_core::{
    render_junit, render_sarif, run_suite, suite_from_pack, suite_with_server, ConnectOptions,
    ServerSpec, SuiteFile, SuiteOutcome, SuiteResolution,
};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Parser)]
#[command(name = "mcp-test", version, about = "Test and validate MCP servers")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a JSON suite file against a server command or HTTP endpoint.
    Run {
        #[arg(long)]
        config: PathBuf,
        #[arg(long, default_value_t = 5000_u64)]
        timeout_ms: u64,
        #[arg(long)]
        report: Option<PathBuf>,
        #[arg(long)]
        junit: Option<PathBuf>,
        #[arg(long)]
        sarif: Option<PathBuf>,
        #[arg(long, default_value = "2024-11-05")]
        protocol_version: String,
        /// Append redacted NDJSON trace of RPC exchanges to this file.
        #[arg(long)]
        trace_file: Option<PathBuf>,
        /// JSON object mapping server→client JSON-RPC `method` strings to literal `result` payloads.
        #[arg(long)]
        client_reply_file: Option<PathBuf>,
    },
    /// Run the built-in conformance pack (or `--pack default`) against a server.
    Conformance {
        #[arg(long)]
        command: String,
        #[arg(long = "server-arg")]
        server_arg: Vec<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long, default_value_t = 5000_u64)]
        timeout_ms: u64,
        #[arg(long)]
        report: Option<PathBuf>,
        #[arg(long)]
        junit: Option<PathBuf>,
        #[arg(long)]
        sarif: Option<PathBuf>,
        #[arg(long, default_value = "2024-11-05")]
        protocol_version: String,
        #[arg(long)]
        trace_file: Option<PathBuf>,
        #[arg(long)]
        client_reply_file: Option<PathBuf>,
        /// Conformance pack name (`builtin` for compiled checks, `default` for JSON pack).
        #[arg(long, default_value = "builtin")]
        pack: String,
    },
    /// Alias for `run` that requires `--trace-file` (record a redacted NDJSON transcript).
    Record {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        trace_file: PathBuf,
        #[arg(long, default_value_t = 5000_u64)]
        timeout_ms: u64,
        #[arg(long)]
        report: Option<PathBuf>,
        #[arg(long)]
        junit: Option<PathBuf>,
        #[arg(long)]
        sarif: Option<PathBuf>,
        #[arg(long, default_value = "2024-11-05")]
        protocol_version: String,
        #[arg(long)]
        client_reply_file: Option<PathBuf>,
    },
    /// Lightweight parameter mutation fuzzing against the first step of the first scenario.
    Fuzz {
        #[arg(long)]
        config: PathBuf,
        #[arg(long, default_value_t = 50_u64)]
        iterations: u64,
        #[arg(long, default_value_t = 2000_u64)]
        timeout_ms: u64,
        #[arg(long, default_value = "2024-11-05")]
        protocol_version: String,
    },
}

fn connect_options(
    timeout_ms: u64,
    protocol_version: String,
    trace_file: Option<PathBuf>,
    client_reply_file: Option<PathBuf>,
) -> Result<ConnectOptions> {
    let mut opts = ConnectOptions {
        protocol_version,
        response_timeout: Duration::from_millis(timeout_ms),
        trace_ndjson_path: trace_file,
        ..Default::default()
    };
    if let Some(path) = client_reply_file {
        let raw = fs::read(&path)
            .with_context(|| format!("read client reply file {}", path.display()))?;
        let map: HashMap<String, Value> =
            serde_json::from_slice(&raw).context("parse client reply JSON object")?;
        opts.client_jsonrpc_results = map;
    }
    Ok(opts)
}

fn write_artifacts(
    outcome: &SuiteOutcome,
    report: Option<&Path>,
    junit: Option<&Path>,
    junit_suite_name: &str,
    sarif: Option<&Path>,
    sarif_suite_uri: &str,
) -> Result<()> {
    if let Some(path) = report {
        let text =
            serde_json::to_string_pretty(outcome).context("failed to serialize JSON report")?;
        fs::write(path, text)
            .with_context(|| format!("failed to write JSON report {}", path.display()))?;
    }
    if let Some(path) = junit {
        let xml = render_junit(junit_suite_name, outcome);
        fs::write(path, xml)
            .with_context(|| format!("failed to write JUnit report {}", path.display()))?;
    }
    if let Some(path) = sarif {
        let doc = render_sarif(sarif_suite_uri, outcome);
        fs::write(path, doc)
            .with_context(|| format!("failed to write SARIF report {}", path.display()))?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Run {
            config,
            timeout_ms,
            report,
            junit,
            sarif,
            protocol_version,
            trace_file,
            client_reply_file,
        } => {
            let data = fs::read(&config)
                .with_context(|| format!("failed to read suite file {}", config.display()))?;
            let suite = SuiteFile::from_json_slice(&data).context("failed to parse suite JSON")?;
            if suite.version < 1 || suite.version > 3 {
                bail!("unsupported suite version: {}", suite.version);
            }
            let resolution = SuiteResolution::from_suite_path(&config);
            let options =
                connect_options(timeout_ms, protocol_version, trace_file, client_reply_file)?;
            let outcome =
                run_suite(&suite, &options, &resolution).context("suite execution failed")?;
            let text =
                serde_json::to_string_pretty(&outcome).context("failed to serialize report")?;
            write_artifacts(
                &outcome,
                report.as_deref(),
                junit.as_deref(),
                "mcp-test",
                sarif.as_deref(),
                config.to_string_lossy().as_ref(),
            )?;
            println!("{text}");
            if !outcome.passed {
                std::process::exit(1);
            }
        }
        Commands::Conformance {
            command,
            server_arg,
            cwd,
            timeout_ms,
            report,
            junit,
            sarif,
            protocol_version,
            trace_file,
            client_reply_file,
            pack,
        } => {
            let server = ServerSpec {
                http: None,
                command,
                args: server_arg,
                cwd: cwd.as_ref().map(|p| p.to_string_lossy().into_owned()),
                env: std::collections::HashMap::new(),
            };
            let suite = match pack.as_str() {
                "builtin" => suite_with_server(server),
                other => suite_from_pack(server, other).context("load conformance pack")?,
            };
            let resolution = SuiteResolution::default();
            let options =
                connect_options(timeout_ms, protocol_version, trace_file, client_reply_file)?;
            let outcome =
                run_suite(&suite, &options, &resolution).context("conformance run failed")?;
            let text =
                serde_json::to_string_pretty(&outcome).context("failed to serialize report")?;
            write_artifacts(
                &outcome,
                report.as_deref(),
                junit.as_deref(),
                "mcp-test-conformance",
                sarif.as_deref(),
                "conformance",
            )?;
            println!("{text}");
            if !outcome.passed {
                std::process::exit(1);
            }
        }
        Commands::Record {
            config,
            trace_file,
            timeout_ms,
            report,
            junit,
            sarif,
            protocol_version,
            client_reply_file,
        } => {
            let data = fs::read(&config)
                .with_context(|| format!("failed to read suite file {}", config.display()))?;
            let suite = SuiteFile::from_json_slice(&data).context("failed to parse suite JSON")?;
            if suite.version < 1 || suite.version > 3 {
                bail!("unsupported suite version: {}", suite.version);
            }
            let resolution = SuiteResolution::from_suite_path(&config);
            let options = connect_options(
                timeout_ms,
                protocol_version,
                Some(trace_file),
                client_reply_file,
            )?;
            let outcome = run_suite(&suite, &options, &resolution).context("record run failed")?;
            let text =
                serde_json::to_string_pretty(&outcome).context("failed to serialize report")?;
            write_artifacts(
                &outcome,
                report.as_deref(),
                junit.as_deref(),
                "mcp-test-record",
                sarif.as_deref(),
                config.to_string_lossy().as_ref(),
            )?;
            println!("{text}");
            if !outcome.passed {
                std::process::exit(1);
            }
        }
        Commands::Fuzz {
            config,
            iterations,
            timeout_ms,
            protocol_version,
        } => {
            fuzz::run_fuzz(&config, iterations, timeout_ms, protocol_version)?;
        }
    }
    Ok(())
}
