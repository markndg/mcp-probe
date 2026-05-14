use crate::error::CoreError;
use crate::expect::{strict_equal, subset_match_with_options};
use crate::protocol::ConnectOptions;
use crate::rpc::RpcError;
use crate::schema::validate_json_schema;
use crate::suite::{ExpectRpcError, ExpectSpec, MatchMode, Scenario, SessionMode, SuiteFile};
use crate::transport::McpSession;
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

/// How on-disk suite-relative paths (such as `result_schema_path`) are resolved.
#[derive(Debug, Clone, Default)]
pub struct SuiteResolution {
    /// Directory containing the suite file (used for `result_schema_path`).
    pub suite_directory: Option<PathBuf>,
}

impl SuiteResolution {
    /// Best-effort resolution from an on-disk suite path.
    pub fn from_suite_path(path: &Path) -> Self {
        Self {
            suite_directory: std::fs::canonicalize(path)
                .ok()
                .and_then(|abs| abs.parent().map(|p| p.to_path_buf())),
        }
    }
}

/// Outcome for a single scenario.
#[derive(Debug, Clone, Serialize)]
pub struct ScenarioOutcome {
    pub name: String,
    pub passed: bool,
    #[serde(default)]
    pub skipped: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Aggregate report for a suite run.
#[derive(Debug, Clone, Serialize)]
pub struct SuiteOutcome {
    pub passed: bool,
    pub scenarios: Vec<ScenarioOutcome>,
}

fn secure_schema_path(base: &Path, rel: &str) -> Result<PathBuf, CoreError> {
    let trimmed = rel.trim();
    if trimmed.contains("..") {
        return Err(CoreError::PathTraversal(format!(
            "result_schema_path contains '..': {trimmed}"
        )));
    }
    let joined = base.join(trimmed);
    let canon = std::fs::canonicalize(&joined).map_err(CoreError::Io)?;
    let base_canon = std::fs::canonicalize(base).map_err(CoreError::Io)?;
    if !canon.starts_with(&base_canon) {
        return Err(CoreError::PathTraversal(format!(
            "result_schema_path escapes suite directory: {trimmed}"
        )));
    }
    Ok(canon)
}

fn should_skip_for_capabilities(server_caps: &Value, keys: &[String]) -> bool {
    if keys.is_empty() {
        return false;
    }
    let Some(obj) = server_caps.as_object() else {
        return true;
    };
    !keys.iter().any(|k| obj.contains_key(k))
}

/// Runs every scenario in `suite` against server transport(s) according to [`SessionMode`].
pub fn run_suite(
    suite: &SuiteFile,
    options: &ConnectOptions,
    resolution: &SuiteResolution,
) -> Result<SuiteOutcome, CoreError> {
    let mut scenarios_out = Vec::with_capacity(suite.scenarios.len());
    let mut all_passed = true;

    match suite.session {
        SessionMode::PerScenario => {
            for scenario in &suite.scenarios {
                let mut session = match McpSession::connect(&suite.server, options.clone()) {
                    Ok(s) => s,
                    Err(e) => {
                        all_passed = false;
                        scenarios_out.push(ScenarioOutcome {
                            name: scenario.name.clone(),
                            passed: false,
                            skipped: false,
                            error: Some(e.to_string()),
                        });
                        continue;
                    }
                };
                let caps = session.capabilities().clone();
                if should_skip_for_capabilities(&caps, &scenario.skip_unless_any_capability) {
                    scenarios_out.push(ScenarioOutcome {
                        name: scenario.name.clone(),
                        passed: true,
                        skipped: true,
                        error: None,
                    });
                    continue;
                }
                let res = run_scenario_on_session(&mut session, scenario, resolution);
                push_scenario_outcome(&mut scenarios_out, &mut all_passed, scenario, res);
            }
        }
        SessionMode::PerSuite => {
            let mut session = McpSession::connect(&suite.server, options.clone())?;
            let caps = session.capabilities().clone();
            for scenario in &suite.scenarios {
                if should_skip_for_capabilities(&caps, &scenario.skip_unless_any_capability) {
                    scenarios_out.push(ScenarioOutcome {
                        name: scenario.name.clone(),
                        passed: true,
                        skipped: true,
                        error: None,
                    });
                    continue;
                }
                let res = run_scenario_on_session(&mut session, scenario, resolution);
                push_scenario_outcome(&mut scenarios_out, &mut all_passed, scenario, res);
            }
        }
    }

    Ok(SuiteOutcome {
        passed: all_passed,
        scenarios: scenarios_out,
    })
}

fn push_scenario_outcome(
    scenarios_out: &mut Vec<ScenarioOutcome>,
    all_passed: &mut bool,
    scenario: &Scenario,
    res: Result<(), CoreError>,
) {
    match res {
        Ok(()) => scenarios_out.push(ScenarioOutcome {
            name: scenario.name.clone(),
            passed: true,
            skipped: false,
            error: None,
        }),
        Err(e) => {
            *all_passed = false;
            scenarios_out.push(ScenarioOutcome {
                name: scenario.name.clone(),
                passed: false,
                skipped: false,
                error: Some(e.to_string()),
            });
        }
    }
}

/// Runs one scenario on an existing session.
pub fn run_scenario_on_session(
    session: &mut McpSession,
    scenario: &Scenario,
    resolution: &SuiteResolution,
) -> Result<(), CoreError> {
    for (idx, step) in scenario.steps.iter().enumerate() {
        step.validate().map_err(|detail| {
            CoreError::InvalidStep(format!("{} step {}: {detail}", scenario.name, idx + 1))
        })?;
        let (method, params) = step.rpc_method_and_params().map_err(|detail| {
            CoreError::InvalidStep(format!("{} step {}: {detail}", scenario.name, idx + 1))
        })?;

        let outcome = session.call_outcome(&method, params)?;

        match (&step.expect_error, outcome) {
            (Some(_), Ok(_)) => {
                return Err(CoreError::UnexpectedRpcSuccess {
                    scenario: scenario.name.clone(),
                    step: idx + 1,
                });
            }
            (Some(spec), Err(rpc)) => {
                check_rpc_expectation(spec, &rpc).map_err(|detail| {
                    CoreError::RpcExpectationMismatch {
                        scenario: scenario.name.clone(),
                        step: idx + 1,
                        detail,
                    }
                })?;
            }
            (None, Err(rpc)) => {
                return Err(CoreError::JsonRpc(rpc));
            }
            (None, Ok(value)) => {
                let expect = step.expect.as_ref().ok_or_else(|| {
                    CoreError::InvalidStep(format!(
                        "{} step {}: missing `expect` for success path",
                        scenario.name,
                        idx + 1
                    ))
                })?;
                apply_expect(expect, &value, resolution, &scenario.name, idx + 1)?;
            }
        }
    }
    Ok(())
}

/// Spawns (or connects) a fresh session and runs one scenario (convenience wrapper).
pub fn run_scenario(
    suite: &SuiteFile,
    scenario: &Scenario,
    options: &ConnectOptions,
    resolution: &SuiteResolution,
) -> Result<(), CoreError> {
    let mut session = McpSession::connect(&suite.server, options.clone())?;
    run_scenario_on_session(&mut session, scenario, resolution)
}

fn check_rpc_expectation(spec: &ExpectRpcError, err: &RpcError) -> Result<(), String> {
    if let Some(expected_code) = spec.code {
        if expected_code != err.code {
            return Err(format!("expected code {expected_code}, got {}", err.code));
        }
    }
    if let Some(sub) = &spec.message_contains {
        if !err.message.contains(sub) {
            return Err(format!(
                "expected message to contain `{sub}`, got `{}`",
                err.message
            ));
        }
    }
    if let Some(re) = &spec.message_regex {
        let r = Regex::new(re).map_err(|e| format!("invalid message_regex: {e}"))?;
        if !r.is_match(&err.message) {
            return Err(format!(
                "expected message to match regex `{re}`, got `{}`",
                err.message
            ));
        }
    }
    Ok(())
}

fn apply_expect(
    expect: &ExpectSpec,
    result: &Value,
    resolution: &SuiteResolution,
    scenario: &str,
    step: usize,
) -> Result<(), CoreError> {
    let has_subset = expect.result.is_some();
    let has_inline = expect.result_schema.is_some();
    let has_path = expect
        .result_schema_path
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    if !has_subset && !has_inline && !has_path {
        return Err(CoreError::InvalidExpectConfig {
            scenario: scenario.to_string(),
            step,
            detail: "set at least one of `result`, `result_schema`, or `result_schema_path`"
                .to_string(),
        });
    }

    if has_inline && has_path {
        return Err(CoreError::InvalidExpectConfig {
            scenario: scenario.to_string(),
            step,
            detail: "use only one of `result_schema` or `result_schema_path`".to_string(),
        });
    }

    let effective: Cow<'_, Value> = if let Some(ptr) = &expect.result_pointer {
        let trimmed = ptr.trim();
        if trimmed.is_empty() {
            Cow::Borrowed(result)
        } else {
            let inner = result
                .pointer(trimmed)
                .ok_or_else(|| CoreError::InvalidExpectConfig {
                    scenario: scenario.to_string(),
                    step,
                    detail: format!("result_pointer `{ptr}` not found in response"),
                })?;
            Cow::Borrowed(inner)
        }
    } else {
        Cow::Borrowed(result)
    };
    let effective = effective.as_ref();

    if let Some(template) = &expect.result {
        match expect.match_mode {
            MatchMode::Strict => {
                strict_equal(template, effective).map_err(|e| CoreError::StepExpectation {
                    scenario: scenario.to_string(),
                    step,
                    source: e,
                })?;
            }
            MatchMode::Subset => {
                subset_match_with_options(template, effective, expect.ordered_arrays).map_err(
                    |e| CoreError::StepExpectation {
                        scenario: scenario.to_string(),
                        step,
                        source: e,
                    },
                )?;
            }
        }
    }

    if let Some(schema) = &expect.result_schema {
        validate_json_schema(effective, schema).map_err(|message| {
            CoreError::ResultSchemaMismatch {
                scenario: scenario.to_string(),
                step,
                message,
            }
        })?;
    }

    if let Some(rel) = &expect.result_schema_path {
        let trimmed = rel.trim();
        if trimmed.is_empty() {
            return Err(CoreError::InvalidExpectConfig {
                scenario: scenario.to_string(),
                step,
                detail: "`result_schema_path` is empty".to_string(),
            });
        }

        let base = resolution.suite_directory.as_ref().ok_or_else(|| {
            CoreError::InvalidExpectConfig {
                scenario: scenario.to_string(),
                step,
                detail: "`result_schema_path` requires the suite to be loaded from disk so its directory can be resolved"
                    .to_string(),
            }
        })?;

        let full = secure_schema_path(base, trimmed)?;
        let bytes = std::fs::read(&full)?;
        let schema: Value = serde_json::from_slice(&bytes)?;
        validate_json_schema(effective, &schema).map_err(|message| {
            CoreError::ResultSchemaMismatch {
                scenario: scenario.to_string(),
                step,
                message,
            }
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
            session: SessionMode::default(),
            server: ServerSpec {
                http: None,
                command: "false".to_string(),
                args: Vec::new(),
                cwd: None,
                env: HashMap::new(),
            },
            scenarios: Vec::new(),
        };
        let resolution = SuiteResolution::default();
        let outcome = run_suite(&suite, &ConnectOptions::default(), &resolution).expect("suite");
        assert!(outcome.passed);
        assert!(outcome.scenarios.is_empty());
    }
}
