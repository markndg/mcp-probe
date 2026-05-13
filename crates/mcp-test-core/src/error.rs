use crate::expect::MatchFailure;
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
    Rpc(String),

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

    #[error("server subprocess exited unexpectedly: {0:?}")]
    ChildExited(Option<std::process::ExitStatus>),

    #[error("received an unexpected message from the server: {0}")]
    UnexpectedMessage(String),
}
