//! Minimal SARIF 2.1.0 output for static analysis consumers (security / CI gates).

use crate::runner::SuiteOutcome;
use serde::Serialize;

#[derive(Serialize)]
struct SarifRoot<'a> {
    #[serde(rename = "version")]
    version: &'static str,
    #[serde(rename = "$schema")]
    schema: &'static str,
    runs: Vec<SarifRun<'a>>,
}

#[derive(Serialize)]
struct SarifRun<'a> {
    tool: SarifTool,
    results: Vec<SarifResult<'a>>,
}

#[derive(Serialize)]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Serialize)]
struct SarifDriver {
    name: &'static str,
    #[serde(rename = "semanticVersion")]
    semantic_version: &'static str,
    information_uri: &'static str,
}

#[derive(Serialize)]
struct SarifResult<'a> {
    rule_id: &'static str,
    level: &'static str,
    message: SarifMessage<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    locations: Option<Vec<SarifLocation<'a>>>,
}

#[derive(Serialize)]
struct SarifMessage<'a> {
    text: &'a str,
}

#[derive(Serialize)]
struct SarifLocation<'a> {
    #[serde(rename = "physicalLocation")]
    physical_location: SarifPhysicalLocation<'a>,
}

#[derive(Serialize)]
struct SarifPhysicalLocation<'a> {
    #[serde(rename = "artifactLocation")]
    artifact_location: SarifArtifactLocation<'a>,
}

#[derive(Serialize)]
struct SarifArtifactLocation<'a> {
    uri: &'a str,
}

/// Emit SARIF JSON for failed (non-skipped) scenarios.
pub fn render_sarif(suite_name: &str, outcome: &SuiteOutcome) -> String {
    let mut results = Vec::new();
    for scenario in &outcome.scenarios {
        if scenario.skipped || scenario.passed {
            continue;
        }
        let msg = scenario
            .error
            .as_deref()
            .unwrap_or("scenario failed without message");
        results.push(SarifResult {
            rule_id: "mcp-test/scenario-failed",
            level: "error",
            message: SarifMessage { text: msg },
            locations: Some(vec![SarifLocation {
                physical_location: SarifPhysicalLocation {
                    artifact_location: SarifArtifactLocation { uri: suite_name },
                },
            }]),
        });
    }

    let root = SarifRoot {
        version: "2.1.0",
        schema: "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "mcp-test",
                    semantic_version: env!("CARGO_PKG_VERSION"),
                    information_uri: "https://modelcontextprotocol.io/",
                },
            },
            results,
        }],
    };

    serde_json::to_string_pretty(&root).unwrap_or_else(|_| "{}".into())
}
