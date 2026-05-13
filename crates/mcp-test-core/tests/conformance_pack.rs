use mcp_test_core::{run_suite, suite_with_server, ConnectOptions, ServerSpec, SuiteResolution};
use std::time::Duration;

#[test]
fn conformance_pack_passes_on_fake_stdio_server() {
    let exe = std::env::var_os("CARGO_BIN_EXE_mcp-test-fake-server")
        .expect("cargo should set CARGO_BIN_EXE_mcp-test-fake-server for integration tests");
    let server = ServerSpec {
        http: None,
        command: exe.to_string_lossy().into_owned(),
        args: Vec::new(),
        cwd: None,
        env: std::collections::HashMap::new(),
    };
    let suite = suite_with_server(server);
    let options = ConnectOptions {
        response_timeout: Duration::from_secs(2),
        ..ConnectOptions::default()
    };
    let outcome = run_suite(&suite, &options, &SuiteResolution::default()).expect("run suite");
    assert!(
        !outcome.scenarios.iter().any(|s| s.skipped),
        "builtin conformance scenarios must run (not capability-skip); got: {outcome:?}"
    );
    assert!(outcome.passed, "{outcome:?}");
}
