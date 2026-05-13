use anyhow::{Context, Result};
use mcp_test_core::{ConnectOptions, McpSession, SuiteFile};
use rand::Rng;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

fn mutate_params(base: &Value, seed: u64) -> Value {
    let mut rng = rand::thread_rng();
    match base {
        Value::Object(m) => {
            let mut out = m.clone();
            out.insert(format!("_fuzz_{seed}"), json!(rng.gen::<u32>()));
            Value::Object(out)
        }
        _ => json!({ "_fuzz": rng.gen::<u32>() }),
    }
}

/// Best-effort fuzz runner: repeatedly opens sessions and issues the first scenario’s first RPC
/// with randomized parameters to catch crashes/hangs (not a replacement for protocol-aware fuzzing).
pub fn run_fuzz(
    config: &Path,
    iterations: u64,
    timeout_ms: u64,
    protocol_version: String,
) -> Result<()> {
    let data = std::fs::read(config).with_context(|| format!("read {}", config.display()))?;
    let suite: SuiteFile = SuiteFile::from_json_slice(&data).context("parse suite JSON")?;
    let scenario = suite
        .scenarios
        .first()
        .context("suite needs at least one scenario")?;
    let step = scenario
        .steps
        .first()
        .context("first scenario needs at least one step")?;
    step.validate().map_err(anyhow::Error::msg)?;
    let (method, base_params) = step.rpc_method_and_params().map_err(anyhow::Error::msg)?;

    let options = ConnectOptions {
        protocol_version,
        response_timeout: Duration::from_millis(timeout_ms),
        ..Default::default()
    };

    let mut transport_failures = 0u64;
    let mut rpc_failures = 0u64;
    let mut successes = 0u64;

    for i in 0..iterations {
        let mut session = match McpSession::connect(&suite.server, options.clone()) {
            Ok(s) => s,
            Err(_) => {
                transport_failures += 1;
                continue;
            }
        };
        let params = mutate_params(&base_params, i);
        match session.call_outcome(&method, params) {
            Ok(Ok(_)) => successes += 1,
            Ok(Err(_)) => rpc_failures += 1,
            Err(_) => {
                transport_failures += 1;
            }
        }
    }

    eprintln!(
        "fuzz complete: iterations={iterations} successes={successes} rpc_errors={rpc_failures} transport_errors={transport_failures}"
    );
    Ok(())
}
