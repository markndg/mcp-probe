use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// JSON-RPC error object returned by the MCP server for a request.
#[derive(Debug, Clone, Serialize, Deserialize, Error)]
#[error("json-rpc error code={code} message={message}")]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}
