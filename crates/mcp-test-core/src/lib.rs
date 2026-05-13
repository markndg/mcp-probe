//! Core library for testing MCP servers over the stdio transport.
//!
//! v1 scope: spawn a server subprocess, perform the MCP initialize handshake,
//! send JSON-RPC requests, and assert structured expectations against responses.

mod error;
mod expect;
mod protocol;
mod runner;
mod suite;
mod transport;

pub use error::CoreError;
pub use expect::{subset_match, MatchFailure};
pub use protocol::{ClientInfo, ConnectOptions};
pub use runner::{run_scenario, run_suite, ScenarioOutcome, SuiteOutcome};
pub use suite::{Scenario, SendSpec, ServerSpec, Step, SuiteFile};
pub use transport::McpStdioSession;
