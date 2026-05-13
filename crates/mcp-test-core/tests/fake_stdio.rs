use mcp_test_core::{run_suite, ConnectOptions, SuiteFile};
use std::time::Duration;

#[test]
fn fake_server_tools_list_passes() {
    let exe = std::env::var_os("CARGO_BIN_EXE_mcp-test-fake-server")
        .expect("cargo should set CARGO_BIN_EXE_mcp-test-fake-server for integration tests");
    let suite_json = serde_json::json!({
        "version": 1,
        "server": {
            "command": exe.to_string_lossy().to_string(),
            "args": []
        },
        "scenarios": [{
            "name": "lists tools",
            "steps": [{
                "send": { "method": "tools/list", "params": {} },
                "expect": { "result": { "tools": [{ "name": "demo_tool" }] } }
            }]
        }]
    });
    let suite: SuiteFile = serde_json::from_value(suite_json).expect("suite json");
    let options = ConnectOptions {
        response_timeout: Duration::from_secs(2),
        ..ConnectOptions::default()
    };
    let outcome = run_suite(&suite, &options).expect("run suite");
    assert!(outcome.passed, "{outcome:?}");
}

#[test]
fn fake_server_subset_mismatch_fails() {
    let exe = std::env::var_os("CARGO_BIN_EXE_mcp-test-fake-server")
        .expect("cargo should set CARGO_BIN_EXE_mcp-test-fake-server for integration tests");
    let suite_json = serde_json::json!({
        "version": 1,
        "server": {
            "command": exe.to_string_lossy().to_string(),
            "args": []
        },
        "scenarios": [{
            "name": "wrong expectation",
            "steps": [{
                "send": { "method": "tools/list", "params": {} },
                "expect": { "result": { "tools": [{ "name": "missing_tool" }] } }
            }]
        }]
    });
    let suite: SuiteFile = serde_json::from_value(suite_json).expect("suite json");
    let options = ConnectOptions {
        response_timeout: Duration::from_secs(2),
        ..ConnectOptions::default()
    };
    let outcome = run_suite(&suite, &options).expect("run suite");
    assert!(!outcome.passed);
    let err = outcome.scenarios[0].error.as_ref().expect("error");
    assert!(
        err.contains("expectation failed"),
        "unexpected error text: {err}"
    );
}
