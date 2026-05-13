use serde::Serialize;

/// Information about this test client, sent during `initialize`.
#[derive(Debug, Clone, Serialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self {
            name: "mcp-test".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Tunables for connecting to an MCP server over stdio.
#[derive(Debug, Clone)]
pub struct ConnectOptions {
    /// MCP protocol version string negotiated with `initialize`.
    pub protocol_version: String,
    /// Wall-clock timeout waiting for each JSON-RPC response after a request.
    pub response_timeout: std::time::Duration,
    pub client_info: ClientInfo,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            protocol_version: "2024-11-05".to_string(),
            response_timeout: std::time::Duration::from_secs(5),
            client_info: ClientInfo::default(),
        }
    }
}
