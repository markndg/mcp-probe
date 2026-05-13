use anyhow::{Context, Result};
use mcp_test_core::{ConnectOptions, McpSession, SuiteFile};
use rand::Rng;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

fn fuzz_method(base: &str, seed: u64) -> String {
    match seed % 8 {
        0 => base.to_string(),
        1 => format!("{base}/"),
        2 if !base.is_empty() => base[..base.len().saturating_sub(1)].to_string(),
        3 => "__not_an_mcp_method__".to_string(),
        4 => String::new(),
        5 => format!("NOT_{}", base.to_ascii_uppercase()),
        6 => format!("{base}::{seed}"),
        _ => base.chars().rev().collect(),
    }
}

fn mutate_params(base: &Value, seed: u64) -> Value {
    let mut rng = rand::thread_rng();
    let mode = seed % 10;
    match mode {
        0 | 1 => match base {
            Value::Object(m) if !m.is_empty() => {
                let mut out = m.clone();
                out.insert(format!("_fuzz_{seed}"), json!(rng.gen::<u32>()));
                Value::Object(out)
            }
            Value::Object(m) => {
                let mut out = m.clone();
                out.insert("_fuzz_empty".to_string(), json!(true));
                Value::Object(out)
            }
            _ => json!({ "_fuzz": rng.gen::<u32>() }),
        },
        2 => Value::Null,
        3 => json!(rng.gen::<i64>()),
        4 => json!("wrong_param_type"),
        5 => match base {
            Value::Object(m) if !m.is_empty() => {
                let keys: Vec<_> = m.keys().cloned().collect();
                let drop = keys[(seed as usize) % keys.len()].clone();
                let mut out = m.clone();
                out.remove(&drop);
                Value::Object(out)
            }
            _ => base.clone(),
        },
        6 => match base {
            Value::Object(m) if !m.is_empty() => {
                let keys: Vec<_> = m.keys().cloned().collect();
                let k = keys[(seed as usize) % keys.len()].clone();
                let mut out = m.clone();
                out.insert(k, json!({ "nested": [1, 2, { "x": null }] }));
                Value::Object(out)
            }
            _ => json!({ "nested": { "a": 1 } }),
        },
        7 => json!([]),
        8 => json!({ "cursor": seed, "extra": { "b": false } }),
        _ => base.clone(),
    }
}

/// Best-effort fuzz runner: repeatedly opens sessions and issues the first scenario’s first RPC
/// with randomized parameters and sometimes a malformed method string to catch crashes/hangs
/// (not a replacement for protocol-aware framing fuzzing).
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
    let (base_method, base_params) = step.rpc_method_and_params().map_err(anyhow::Error::msg)?;

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
        let method = fuzz_method(&base_method, i);
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
