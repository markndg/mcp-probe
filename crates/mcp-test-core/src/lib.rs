//! Core library for testing MCP servers (stdio and experimental HTTP).
//!
//! Supports JSON suite files, JSON Schema contracts, conformance packs, SARIF/JUnit
//! reporting helpers, session modes, and structured JSON-RPC error assertions.

mod conformance;
mod error;
mod expect;
mod junit;
mod packs;
mod protocol;
mod rpc;
mod runner;
mod sarif;
mod schema;
mod suite;
mod trace;
mod transport;

pub use conformance::{builtin_scenarios, suite_with_server};
pub use error::CoreError;
pub use expect::{strict_equal, subset_match, subset_match_with_options, MatchFailure};
pub use junit::render_junit;
pub use packs::{load_pack_scenarios, suite_from_pack};
pub use protocol::{ClientInfo, ConnectOptions};
pub use rpc::RpcError;
pub use runner::{
    run_scenario, run_scenario_on_session, run_suite, ScenarioOutcome, SuiteOutcome,
    SuiteResolution,
};
pub use sarif::render_sarif;
pub use suite::{
    ExpectRpcError, ExpectSpec, HttpEndpoint, MatchMode, Scenario, SendSpec, ServerSpec,
    SessionMode, Step, SuiteFile, ToolCallSpec,
};
pub use trace::{append_ndjson, redact_secrets, trace_event};
pub use transport::{McpHttpSession, McpSession, McpStdioSession};
