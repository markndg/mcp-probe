use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Information about this test client, sent during `initialize`.
#[derive(Debug, Clone, Serialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self {
            name: "mcp-probe".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Tunables for connecting to an MCP server (stdio or HTTP).
#[derive(Debug, Clone)]
pub struct ConnectOptions {
    /// MCP protocol version string negotiated with `initialize`.
    pub protocol_version: String,
    /// Wall-clock timeout waiting for each JSON-RPC response after a request.
    pub response_timeout: Duration,
    pub client_info: ClientInfo,
    /// Automatic JSON-RPC **results** returned for server→client requests with matching `method` keys.
    pub client_jsonrpc_results: HashMap<String, Value>,
    /// When set, append one NDJSON object per RPC exchange (redacted) to this file.
    pub trace_ndjson_path: Option<PathBuf>,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            protocol_version: "2024-11-05".to_string(),
            response_timeout: Duration::from_secs(5),
            client_info: ClientInfo::default(),
            client_jsonrpc_results: HashMap::new(),
            trace_ndjson_path: None,
        }
    }
}
