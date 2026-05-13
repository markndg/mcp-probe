use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// How scenarios share a server process.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    /// Fresh subprocess + handshake for every scenario (default, most deterministic).
    #[default]
    PerScenario,
    /// One subprocess for the whole suite; scenarios run in order on the same session.
    PerSuite,
}

/// HTTP(S) MCP endpoint (experimental: JSON-RPC POST with optional session header).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HttpEndpoint {
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

/// Child process specification for stdio MCP servers.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ServerSpec {
    #[serde(default)]
    pub http: Option<HttpEndpoint>,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Top-level suite document (JSON).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SuiteFile {
    pub version: u32,
    #[serde(default)]
    pub session: SessionMode,
    pub server: ServerSpec,
    pub scenarios: Vec<Scenario>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Scenario {
    pub name: String,
    /// If non-empty, the scenario is skipped unless the `initialize` result `capabilities`
    /// object contains **at least one** of these top-level keys (for example `"resources"`).
    #[serde(default)]
    pub skip_unless_any_capability: Vec<String>,
    #[serde(default)]
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Step {
    #[serde(default)]
    pub send: Option<SendSpec>,
    #[serde(default)]
    pub call_tool: Option<ToolCallSpec>,
    /// Present for successful JSON-RPC `result` expectations.
    #[serde(default)]
    pub expect: Option<ExpectSpec>,
    /// When present, the RPC call must fail with a JSON-RPC error matching this spec.
    #[serde(default)]
    pub expect_error: Option<ExpectRpcError>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToolCallSpec {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SendSpec {
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MatchMode {
    #[default]
    Subset,
    Strict,
}

/// Expectations are applied to the JSON-RPC **`result`** object only (unless `expect_error` is used on the step).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ExpectSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_schema: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_schema_path: Option<String>,
    #[serde(default)]
    pub match_mode: MatchMode,
    /// JSON Pointer ([RFC 6901](https://datatracker.ietf.org/doc/html/rfc6901)) into `result` before assertions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_pointer: Option<String>,
    #[serde(default)]
    pub ordered_arrays: bool,
}

impl ExpectSpec {
    pub fn validate_success_expect(&self) -> Result<(), String> {
        let has_subset = self.result.is_some();
        let has_inline = self.result_schema.is_some();
        let has_path = self
            .result_schema_path
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        if !has_subset && !has_inline && !has_path {
            return Err(
                "expect requires at least one of `result`, `result_schema`, or `result_schema_path`"
                    .into(),
            );
        }
        if has_inline && has_path {
            return Err("use only one of `result_schema` or `result_schema_path`".into());
        }
        Ok(())
    }
}

/// Match a JSON-RPC error response.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ExpectRpcError {
    #[serde(default)]
    pub code: Option<i64>,
    #[serde(default)]
    pub message_contains: Option<String>,
    /// Rust regex syntax matched against the error `message`.
    #[serde(default)]
    pub message_regex: Option<String>,
}

impl Step {
    /// Validates step shape (call-time).
    pub fn validate(&self) -> Result<(), String> {
        match (&self.send, &self.call_tool) {
            (Some(_), Some(_)) => Err("set only one of `send` or `call_tool`".to_string()),
            (None, None) => Err("step requires `send` or `call_tool`".to_string()),
            _ => Ok(()),
        }?;

        match (&self.expect_error, &self.expect) {
            (Some(_), _) => Ok(()),
            (None, Some(e)) => e.validate_success_expect(),
            (None, None) => Err("step requires `expect` or `expect_error`".to_string()),
        }
    }

    pub fn rpc_method_and_params(&self) -> Result<(String, Value), String> {
        if let Some(ct) = &self.call_tool {
            let params = serde_json::json!({
                "name": ct.name,
                "arguments": ct.arguments,
            });
            return Ok(("tools/call".to_string(), params));
        }
        if let Some(s) = &self.send {
            return Ok((s.method.clone(), s.params.clone()));
        }
        Err("step has no rpc target".into())
    }
}

impl SuiteFile {
    /// Parses a suite file from JSON bytes.
    pub fn from_json_slice(data: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_suite() {
        let raw = br#"{
            "version": 1,
            "server": { "command": "true" },
            "scenarios": []
        }"#;
        let suite = SuiteFile::from_json_slice(raw).unwrap();
        assert_eq!(suite.version, 1);
        assert!(suite.scenarios.is_empty());
        assert_eq!(suite.session, SessionMode::PerScenario);
    }

    #[test]
    fn parses_v1_expect_with_only_result() {
        let raw = br#"{
            "version": 1,
            "server": { "command": "true" },
            "scenarios": [{
                "name": "x",
                "steps": [{
                    "send": { "method": "tools/list", "params": {} },
                    "expect": { "result": { "tools": [] } }
                }]
            }]
        }"#;
        let suite = SuiteFile::from_json_slice(raw).unwrap();
        suite.scenarios[0].steps[0].validate().unwrap();
    }

    #[test]
    fn parses_v2_expect_schema_only() {
        let raw = br#"{
            "version": 2,
            "server": { "command": "true" },
            "scenarios": [{
                "name": "x",
                "steps": [{
                    "send": { "method": "tools/list", "params": {} },
                    "expect": { "result_schema": { "type": "object" } }
                }]
            }]
        }"#;
        let suite = SuiteFile::from_json_slice(raw).unwrap();
        suite.scenarios[0].steps[0].validate().unwrap();
    }

    #[test]
    fn call_tool_step_round_trips() {
        let raw = br#"{
            "version": 2,
            "server": { "command": "true" },
            "scenarios": [{
                "name": "t",
                "steps": [{
                    "call_tool": { "name": "demo", "arguments": { "x": 1 } },
                    "expect": { "result": { "content": [] } }
                }]
            }]
        }"#;
        let suite = SuiteFile::from_json_slice(raw).unwrap();
        let (m, p) = suite.scenarios[0].steps[0].rpc_method_and_params().unwrap();
        assert_eq!(m, "tools/call");
        assert_eq!(p["name"], "demo");
    }
}
