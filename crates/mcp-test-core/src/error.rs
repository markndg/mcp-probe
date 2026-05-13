use crate::expect::MatchFailure;
use crate::rpc::RpcError;
use serde_json::Error as JsonError;
use std::io;
use thiserror::Error;

/// Errors surfaced by the MCP test harness library.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("i/o error: {0}")]
    Io(#[from] io::Error),

    #[error("json error: {0}")]
    Json(#[from] JsonError),

    #[error("json-rpc error: {0}")]
    JsonRpc(#[from] RpcError),

    #[error("handshake failed: {0}")]
    Handshake(String),

    #[error("timed out after {0:?} waiting for a response")]
    Timeout(std::time::Duration),

    #[error("scenario `{scenario}` step {step}: expectation failed: {source}")]
    StepExpectation {
        scenario: String,
        step: usize,
        #[source]
        source: MatchFailure,
    },

    #[error("scenario `{scenario}` step {step}: invalid expect configuration: {detail}")]
    InvalidExpectConfig {
        scenario: String,
        step: usize,
        detail: String,
    },

    #[error("scenario `{scenario}` step {step}: result JSON Schema mismatch: {message}")]
    ResultSchemaMismatch {
        scenario: String,
        step: usize,
        message: String,
    },

    #[error("scenario `{scenario}` step {step}: rpc error expectation failed: {detail}")]
    RpcExpectationMismatch {
        scenario: String,
        step: usize,
        detail: String,
    },

    #[error("scenario `{scenario}` step {step}: expected JSON-RPC error but call succeeded")]
    UnexpectedRpcSuccess { scenario: String, step: usize },

    #[error("http transport error: {0}")]
    Http(String),

    #[error("blocked potentially unsafe path: {0}")]
    PathTraversal(String),

    #[error("invalid step: {0}")]
    InvalidStep(String),

    #[error("server subprocess exited unexpectedly: {0:?}")]
    ChildExited(Option<std::process::ExitStatus>),

    #[error("received an unexpected message from the server: {0}")]
    UnexpectedMessage(String),
}
