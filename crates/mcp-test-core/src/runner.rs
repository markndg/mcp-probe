use crate::error::CoreError;
use crate::expect::subset_match;
use crate::protocol::ConnectOptions;
use crate::suite::{Scenario, SuiteFile};
use crate::transport::McpStdioSession;
use serde::Serialize;
use std::ffi::OsString;
use std::path::Path;

/// Outcome for a single scenario.
#[derive(Debug, Clone, Serialize)]
pub struct ScenarioOutcome {
    pub name: String,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Aggregate report for a suite run.
#[derive(Debug, Clone, Serialize)]
pub struct SuiteOutcome {
    pub passed: bool,
    pub scenarios: Vec<ScenarioOutcome>,
}

/// Runs every scenario in `suite` against a freshly spawned server process.
pub fn run_suite(suite: &SuiteFile, options: &ConnectOptions) -> Result<SuiteOutcome, CoreError> {
    let mut scenarios = Vec::with_capacity(suite.scenarios.len());
    let mut all_passed = true;
    for scenario in &suite.scenarios {
        let res = run_scenario(suite, scenario, options);
        match res {
            Ok(()) => scenarios.push(ScenarioOutcome {
                name: scenario.name.clone(),
                passed: true,
                error: None,
            }),
            Err(e) => {
                all_passed = false;
                scenarios.push(ScenarioOutcome {
                    name: scenario.name.clone(),
                    passed: false,
                    error: Some(e.to_string()),
                });
            }
        }
    }
    Ok(SuiteOutcome {
        passed: all_passed,
        scenarios,
    })
}

/// Runs one scenario with its own server subprocess and handshake.
pub fn run_scenario(
    suite: &SuiteFile,
    scenario: &Scenario,
    options: &ConnectOptions,
) -> Result<(), CoreError> {
    let cmd_os: OsString = OsString::from(suite.server.command.clone());
    let args_os: Vec<OsString> = suite
        .server
        .args
        .iter()
        .map(|a| OsString::from(a.clone()))
        .collect();
    let cwd = suite
        .server
        .cwd
        .as_ref()
        .map(|p| Path::new(p).to_path_buf());
    let env_pairs: Vec<(OsString, OsString)> = suite
        .server
        .env
        .iter()
        .map(|(k, v)| (OsString::from(k), OsString::from(v)))
        .collect();

    let mut session = McpStdioSession::spawn(
        cmd_os,
        &args_os,
        cwd.as_deref(),
        &env_pairs,
        options.clone(),
    )?;

    for (idx, step) in scenario.steps.iter().enumerate() {
        let result = session.call(&step.send.method, step.send.params.clone())?;
        subset_match(&step.expect.result, &result).map_err(|e| CoreError::StepExpectation {
            scenario: scenario.name.clone(),
            step: idx + 1,
            source: e,
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::suite::ServerSpec;
    use std::collections::HashMap;

    #[test]
    fn empty_suite_passes_without_spawning() {
        let suite = SuiteFile {
            version: 1,
            server: ServerSpec {
                command: "false".to_string(),
                args: Vec::new(),
                cwd: None,
                env: HashMap::new(),
            },
            scenarios: Vec::new(),
        };
        let outcome = run_suite(&suite, &ConnectOptions::default()).expect("suite");
        assert!(outcome.passed);
        assert!(outcome.scenarios.is_empty());
    }
}
