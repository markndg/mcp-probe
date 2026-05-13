use crate::error::CoreError;
use crate::protocol::ConnectOptions;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

enum Incoming {
    Response {
        id: Value,
        payload: Result<Value, RpcErrorPayload>,
    },
    /// Server-initiated JSON-RPC request toward the client (ignored in v1).
    ServerRequest(Value),
    /// Non-request traffic such as stray logs parsed as JSON (treated as ignorable when possible).
    Other(Value),
}

#[derive(Debug, Clone)]
struct RpcErrorPayload {
    code: i64,
    message: String,
    data: Option<Value>,
}

/// Live MCP session over newline-delimited JSON-RPC on a child process stdio pair.
pub struct McpStdioSession {
    child: Child,
    stdin: Arc<Mutex<BufWriter<ChildStdin>>>,
    incoming: Receiver<Incoming>,
    reader_join: Option<thread::JoinHandle<Result<(), CoreError>>>,
    next_id: u64,
    response_timeout: Duration,
}

impl McpStdioSession {
    /// Spawns `command` with `args` and performs the MCP initialize handshake.
    pub fn spawn(
        command: impl AsRef<std::ffi::OsStr>,
        args: &[std::ffi::OsString],
        cwd: Option<&std::path::Path>,
        env: &[(std::ffi::OsString, std::ffi::OsString)],
        options: ConnectOptions,
    ) -> Result<Self, CoreError> {
        let mut cmd = Command::new(command.as_ref());
        cmd.args(args);
        if let Some(cwd) = cwd {
            cmd.current_dir(cwd);
        }
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| CoreError::Handshake("failed to open child stdin pipe".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CoreError::Handshake("failed to open child stdout pipe".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| CoreError::Handshake("failed to open child stderr pipe".to_string()))?;

        let (tx, rx) = mpsc::channel::<Incoming>();

        let _ = thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                // Intentionally discard stderr by default to avoid leaking secrets.
                let _ = line;
            }
        });

        let reader_join = thread::spawn(move || read_stdout_loop(stdout, tx));

        let stdin = Arc::new(Mutex::new(BufWriter::new(stdin)));
        let mut session = Self {
            child,
            stdin,
            incoming: rx,
            reader_join: Some(reader_join),
            next_id: 2,
            response_timeout: options.response_timeout,
        };

        session.handshake(&options)?;
        Ok(session)
    }

    fn handshake(&mut self, options: &ConnectOptions) -> Result<(), CoreError> {
        let init_id = Value::from(1_u64);
        let params = json!({
            "protocolVersion": options.protocol_version,
            "capabilities": {},
            "clientInfo": {
                "name": options.client_info.name,
                "version": options.client_info.version,
            }
        });
        self.write_request(&init_id, "initialize", params)?;
        let init_result = self.wait_for_response(&init_id)?;
        validate_initialize_result(&init_result)?;

        let initialized = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        self.write_raw_value(&initialized)?;
        Ok(())
    }

    fn write_raw_value(&mut self, value: &Value) -> Result<(), CoreError> {
        let mut line = serde_json::to_string(value)?;
        line.push('\n');
        let mut guard = self
            .stdin
            .lock()
            .map_err(|e| CoreError::Handshake(format!("failed to lock stdin writer: {e}")))?;
        guard.write_all(line.as_bytes())?;
        guard.flush()?;
        Ok(())
    }

    fn write_request(&mut self, id: &Value, method: &str, params: Value) -> Result<(), CoreError> {
        let envelope = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.write_raw_value(&envelope)
    }

    /// Sends a JSON-RPC request and returns the `result` object on success.
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, CoreError> {
        let id = Value::from(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.write_request(&id, method, params)?;
        self.wait_for_response(&id)
    }

    fn wait_for_response(&mut self, id: &Value) -> Result<Value, CoreError> {
        let deadline = Instant::now() + self.response_timeout;
        loop {
            self.check_child_alive()?;
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(CoreError::Timeout(self.response_timeout));
            }
            match self.incoming.recv_timeout(remaining) {
                Ok(Incoming::Response {
                    id: rid,
                    payload: Ok(result),
                }) => {
                    if &rid == id {
                        return Ok(result);
                    }
                }
                Ok(Incoming::Response {
                    id: rid,
                    payload: Err(err),
                }) => {
                    if &rid == id {
                        return Err(CoreError::Rpc(format!(
                            "code={}, message={}, data={}",
                            err.code,
                            err.message,
                            err.data
                                .as_ref()
                                .map(|d| d.to_string())
                                .unwrap_or_else(|| "null".to_string())
                        )));
                    }
                }
                Ok(Incoming::ServerRequest(v)) => {
                    // v1: ignore server->client requests; real servers rarely send before client calls.
                    let _ = v;
                }
                Ok(Incoming::Other(v)) => {
                    let _ = v;
                }
                Err(RecvTimeoutError::Timeout) => {
                    return Err(CoreError::Timeout(self.response_timeout));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(self.map_reader_disconnect_to_child_exit());
                }
            }
        }
    }

    fn map_reader_disconnect_to_child_exit(&mut self) -> CoreError {
        match self.child.try_wait() {
            Ok(Some(status)) => CoreError::ChildExited(Some(status)),
            Ok(None) => CoreError::UnexpectedMessage(
                "stdout reader disconnected while child still running".to_string(),
            ),
            Err(e) => CoreError::Io(e),
        }
    }

    fn check_child_alive(&mut self) -> Result<(), CoreError> {
        match self.child.try_wait()? {
            Some(status) => Err(CoreError::ChildExited(Some(status))),
            None => Ok(()),
        }
    }
}

impl Drop for McpStdioSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(h) = self.reader_join.take() {
            let _ = h.join();
        }
    }
}

fn read_stdout_loop(
    stdout: std::process::ChildStdout,
    tx: mpsc::Sender<Incoming>,
) -> Result<(), CoreError> {
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            return Ok(());
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if value.get("method").is_some() && value.get("id").is_none() {
            // JSON-RPC notification from server (not a response to our requests).
            continue;
        }

        if value.get("method").is_some() && value.get("id").is_some() {
            let _ = tx.send(Incoming::ServerRequest(value));
            continue;
        }

        if let Some(id) = value.get("id").cloned() {
            if let Some(err) = value.get("error") {
                let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
                let message = err
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("")
                    .to_string();
                let data = err.get("data").cloned();
                let _ = tx.send(Incoming::Response {
                    id,
                    payload: Err(RpcErrorPayload {
                        code,
                        message,
                        data,
                    }),
                });
                continue;
            }
            if let Some(result) = value.get("result").cloned() {
                let _ = tx.send(Incoming::Response {
                    id,
                    payload: Ok(result),
                });
                continue;
            }
        }

        let _ = tx.send(Incoming::Other(value));
    }
}

fn validate_initialize_result(result: &Value) -> Result<(), CoreError> {
    let protocol_version = result
        .get("protocolVersion")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            CoreError::Handshake("initialize result missing protocolVersion".to_string())
        })?;
    if protocol_version.is_empty() {
        return Err(CoreError::Handshake(
            "initialize result contained empty protocolVersion".to_string(),
        ));
    }
    Ok(())
}
