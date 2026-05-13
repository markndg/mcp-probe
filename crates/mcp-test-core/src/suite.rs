use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Top-level suite document (JSON).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SuiteFile {
    pub version: u32,
    pub server: ServerSpec,
    pub scenarios: Vec<Scenario>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerSpec {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Scenario {
    pub name: String,
    #[serde(default)]
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Step {
    pub send: SendSpec,
    pub expect: ExpectSpec,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SendSpec {
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// Expectations are applied to the JSON-RPC **`result`** object only.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExpectSpec {
    /// Subset structure that must match the `result` field of a successful JSON-RPC response.
    pub result: Value,
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
    }
}
