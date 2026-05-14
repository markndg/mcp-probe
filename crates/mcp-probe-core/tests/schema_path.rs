use mcp_probe_core::{run_suite, ConnectOptions, SuiteFile, SuiteResolution};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn result_schema_path_resolves_relative_to_suite_directory() {
    let exe = std::env::var_os("CARGO_BIN_EXE_mcp_probe_fake_server")
        .expect("cargo should set CARGO_BIN_EXE_mcp_probe_fake_server for integration tests");

    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schema_path");
    let mut suite_value: serde_json::Value =
        serde_json::from_slice(&fs::read(base.join("suite.json")).expect("read suite.json"))
            .expect("parse suite.json");
    *suite_value
        .pointer_mut("/server/command")
        .expect("command field") = json!(exe.to_string_lossy().to_string());

    let suite: SuiteFile = serde_json::from_value(suite_value).expect("suite model");
    let options = ConnectOptions {
        response_timeout: Duration::from_secs(2),
        ..ConnectOptions::default()
    };
    let resolution = SuiteResolution {
        suite_directory: Some(base),
    };
    let outcome = run_suite(&suite, &options, &resolution).expect("run suite");
    assert!(outcome.passed, "{outcome:?}");
}
