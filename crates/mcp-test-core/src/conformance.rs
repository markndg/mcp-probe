//! Built-in MCP surface checks intended as a starter conformance pack (extensible in later releases).

use crate::suite::{ExpectSpec, Scenario, SendSpec, ServerSpec, Step, SuiteFile};
use serde_json::json;

fn tools_list_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "required": ["tools"],
        "properties": {
            "tools": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["name", "inputSchema"],
                    "properties": {
                        "name": { "type": "string", "minLength": 1 },
                        "description": { "type": "string" },
                        "inputSchema": { "type": "object" }
                    }
                }
            }
        }
    })
}

fn resources_list_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "required": ["resources"],
        "properties": {
            "resources": { "type": "array" }
        }
    })
}

fn prompts_list_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "required": ["prompts"],
        "properties": {
            "prompts": { "type": "array" }
        }
    })
}

/// Scenarios that should pass against any spec-shaped MCP server exposing these methods.
pub fn builtin_scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "conformance:mcp.tools_list".to_string(),
            skip_unless_any_capability: Vec::new(),
            steps: vec![Step {
                send: Some(SendSpec {
                    method: "tools/list".to_string(),
                    params: json!({}),
                }),
                call_tool: None,
                expect: Some(ExpectSpec {
                    result: None,
                    result_schema: Some(tools_list_schema()),
                    result_schema_path: None,
                    ..Default::default()
                }),
                expect_error: None,
            }],
        },
        Scenario {
            name: "conformance:mcp.resources_list".to_string(),
            skip_unless_any_capability: Vec::new(),
            steps: vec![Step {
                send: Some(SendSpec {
                    method: "resources/list".to_string(),
                    params: json!({}),
                }),
                call_tool: None,
                expect: Some(ExpectSpec {
                    result: None,
                    result_schema: Some(resources_list_schema()),
                    result_schema_path: None,
                    ..Default::default()
                }),
                expect_error: None,
            }],
        },
        Scenario {
            name: "conformance:mcp.prompts_list".to_string(),
            skip_unless_any_capability: Vec::new(),
            steps: vec![Step {
                send: Some(SendSpec {
                    method: "prompts/list".to_string(),
                    params: json!({}),
                }),
                call_tool: None,
                expect: Some(ExpectSpec {
                    result: None,
                    result_schema: Some(prompts_list_schema()),
                    result_schema_path: None,
                    ..Default::default()
                }),
                expect_error: None,
            }],
        },
    ]
}

/// A v2 suite with the built-in scenarios and the provided server command.
pub fn suite_with_server(server: ServerSpec) -> SuiteFile {
    SuiteFile {
        version: 2,
        session: crate::suite::SessionMode::default(),
        server,
        scenarios: builtin_scenarios(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conformance_suite_has_expected_shape() {
        let suite = suite_with_server(ServerSpec {
            command: "true".to_string(),
            args: Vec::new(),
            cwd: None,
            env: Default::default(),
            http: None,
        });
        assert_eq!(suite.version, 2);
        assert_eq!(suite.scenarios.len(), 3);
    }
}
