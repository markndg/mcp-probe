use crate::error::CoreError;
use crate::protocol::ConnectOptions;
use crate::rpc::RpcError;
use crate::suite::ServerSpec;
use crate::trace;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

enum Incoming {
    Response {
        id: Value,
        payload: Result<Value, RpcError>,
    },
    ServerRequest(Value),
    Other(Value),
}

/// Live MCP session over newline-delimited JSON-RPC on a child process stdio pair.
pub struct McpStdioSession {
    child: Child,
    stdin: Arc<Mutex<BufWriter<ChildStdin>>>,
    incoming: Receiver<Incoming>,
    reader_join: Option<thread::JoinHandle<Result<(), CoreError>>>,
    next_id: u64,
    response_timeout: Duration,
    trace_file: Option<Arc<Mutex<File>>>,
    /// `capabilities` object from the last successful `initialize` result.
    pub capabilities: Value,
}

impl McpStdioSession {
    /// Spawns the server process and performs the MCP initialize handshake.
    pub fn spawn(server: &ServerSpec, options: ConnectOptions) -> Result<Self, CoreError> {
        if server.http.is_some() {
            return Err(CoreError::Handshake(
                "stdio transport requested but `server.http` is set".into(),
            ));
        }
        if server.command.trim().is_empty() {
            return Err(CoreError::Handshake(
                "stdio transport requires a non-empty `server.command`".into(),
            ));
        }

        let mut cmd = Command::new(&server.command);
        cmd.args(&server.args);
        if let Some(cwd) = &server.cwd {
            cmd.current_dir(Path::new(cwd));
        }
        for (k, v) in &server.env {
            cmd.env(k, v);
        }
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| CoreError::Handshake("failed to open child stdin pipe".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CoreError::Handshake("failed to open child stdout pipe".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| CoreError::Handshake("failed to open child stderr pipe".into()))?;

        let (tx, rx) = mpsc::channel::<Incoming>();

        let _ = thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                let _ = line;
            }
        });

        let stdin_shared = Arc::new(Mutex::new(BufWriter::new(stdin)));
        let handlers = Arc::new(options.client_jsonrpc_results.clone());
        let stdin_for_reader = Arc::clone(&stdin_shared);
        let reader_join =
            thread::spawn(move || read_stdout_loop(stdout, tx, stdin_for_reader, handlers));

        let trace_file = options
            .trace_ndjson_path
            .as_ref()
            .map(|p| {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(p)
                    .map(|f| Arc::new(Mutex::new(f)))
            })
            .transpose()?;

        let mut session = Self {
            child,
            stdin: stdin_shared,
            incoming: rx,
            reader_join: Some(reader_join),
            next_id: 2,
            response_timeout: options.response_timeout,
            trace_file,
            capabilities: json!({}),
        };

        session.handshake(&options)?;
        Ok(session)
    }

    fn trace_line(&self, record: &Value) {
        if let Some(f) = &self.trace_file {
            if let Ok(mut g) = f.lock() {
                let _ = writeln!(
                    g,
                    "{}",
                    serde_json::to_string(&trace::redact_secrets(record)).unwrap_or_default()
                );
            }
        }
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
        let init_result = match self.wait_for_response_payload(&init_id)? {
            Ok(v) => v,
            Err(e) => return Err(CoreError::Handshake(format!("initialize failed: {e}"))),
        };
        validate_initialize_result(&init_result)?;
        self.capabilities = init_result
            .get("capabilities")
            .cloned()
            .unwrap_or_else(|| json!({}));

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
        let mut guard = self.stdin.lock().unwrap_or_else(|e| e.into_inner());
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

    /// Sends a JSON-RPC request; transport failures are `Err`, application-level RPC errors are `Ok(Err(..))`.
    pub fn call_outcome(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<std::result::Result<Value, RpcError>, CoreError> {
        let id = Value::from(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.trace_line(&trace::trace_event("request", &req));
        self.write_request(&id, method, params)?;
        let out = self.wait_for_response_payload(&id)?;
        let trace_payload = match &out {
            Ok(v) => json!({"ok": true, "result": v}),
            Err(e) => json!({"ok": false, "error": e}),
        };
        self.trace_line(&trace::trace_event("response", &trace_payload));
        Ok(out)
    }

    /// Sends a JSON-RPC request and returns the `result` object on success.
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, CoreError> {
        match self.call_outcome(method, params)? {
            Ok(v) => Ok(v),
            Err(e) => Err(CoreError::JsonRpc(e)),
        }
    }

    fn wait_for_response_payload(
        &mut self,
        id: &Value,
    ) -> Result<std::result::Result<Value, RpcError>, CoreError> {
        let deadline = Instant::now() + self.response_timeout;
        loop {
            self.check_child_alive()?;
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(CoreError::Timeout(self.response_timeout));
            }
            match self.incoming.recv_timeout(remaining) {
                Ok(Incoming::Response { id: rid, payload }) => {
                    if &rid == id {
                        return Ok(payload);
                    }
                }
                Ok(Incoming::ServerRequest(v)) => {
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
    stdin: Arc<Mutex<BufWriter<ChildStdin>>>,
    handlers: Arc<HashMap<String, Value>>,
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
            continue;
        }

        if value.get("method").is_some() && value.get("id").is_some() {
            if let Some(method) = value.get("method").and_then(|m| m.as_str()) {
                if let Some(id) = value.get("id") {
                    if let Some(result) = handlers.get(method) {
                        let resp = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": result
                        });
                        let mut out = serde_json::to_string(&resp)?;
                        out.push('\n');
                        let mut w = stdin.lock().unwrap_or_else(|e| e.into_inner());
                        if w.write_all(out.as_bytes()).is_ok() {
                            let _ = w.flush();
                        }
                        continue;
                    }
                }
            }
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
                    payload: Err(RpcError {
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
        .ok_or_else(|| CoreError::Handshake("initialize result missing protocolVersion".into()))?;
    if protocol_version.is_empty() {
        return Err(CoreError::Handshake(
            "initialize result contained empty protocolVersion".into(),
        ));
    }
    Ok(())
}

fn build_http_headers(map: &HashMap<String, String>) -> Result<HeaderMap, CoreError> {
    let mut headers = HeaderMap::new();
    for (k, v) in map {
        let name = HeaderName::from_bytes(k.as_bytes())
            .map_err(|_| CoreError::Http(format!("invalid header name: {k}")))?;
        let val = HeaderValue::from_str(v)
            .map_err(|_| CoreError::Http(format!("invalid header value for {k}")))?;
        headers.insert(name, val);
    }
    Ok(headers)
}

/// Experimental MCP-over-HTTP client (single JSON response per POST).
pub struct McpHttpSession {
    client: reqwest::blocking::Client,
    url: String,
    headers: HeaderMap,
    session_id: Option<String>,
    next_id: u64,
    trace_file: Option<Arc<Mutex<File>>>,
    /// `capabilities` object from the last successful `initialize` result.
    pub capabilities: Value,
}

impl McpHttpSession {
    pub fn connect(server: &ServerSpec, options: ConnectOptions) -> Result<Self, CoreError> {
        let http = server.http.as_ref().ok_or_else(|| {
            CoreError::Handshake("http transport requires `server.http.url`".into())
        })?;
        let client = reqwest::blocking::Client::builder()
            .timeout(options.response_timeout)
            .build()
            .map_err(|e| CoreError::Http(e.to_string()))?;
        let headers = build_http_headers(&http.headers)?;
        let trace_file = options
            .trace_ndjson_path
            .as_ref()
            .map(|p| {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(p)
                    .map(|f| Arc::new(Mutex::new(f)))
            })
            .transpose()?;

        let mut session = Self {
            client,
            url: http.url.clone(),
            headers,
            session_id: None,
            next_id: 2,
            trace_file,
            capabilities: json!({}),
        };
        session.handshake_http(&options)?;
        Ok(session)
    }

    fn trace_line(&self, record: &Value) {
        if let Some(f) = &self.trace_file {
            if let Ok(mut g) = f.lock() {
                let _ = writeln!(
                    g,
                    "{}",
                    serde_json::to_string(&trace::redact_secrets(record)).unwrap_or_default()
                );
            }
        }
    }

    fn parse_session_id(resp: &reqwest::blocking::Response) -> Option<String> {
        for (k, v) in resp.headers() {
            if k.as_str().eq_ignore_ascii_case("mcp-session-id") {
                return v.to_str().ok().map(|s| s.to_string());
            }
        }
        None
    }

    fn post_json(
        &mut self,
        body: &Value,
    ) -> Result<std::result::Result<Value, RpcError>, CoreError> {
        self.trace_line(&trace::trace_event("http_request", body));
        let mut req = self
            .client
            .post(&self.url)
            .headers(self.headers.clone())
            .json(body);
        if let Some(sid) = &self.session_id {
            if let Ok(hv) = HeaderValue::from_str(sid) {
                req = req.header("Mcp-Session-Id", hv);
            }
        }
        let resp = req.send().map_err(|e| CoreError::Http(e.to_string()))?;
        if let Some(sid) = Self::parse_session_id(&resp) {
            self.session_id = Some(sid);
        }
        let status = resp.status();
        let text = resp.text().map_err(|e| CoreError::Http(e.to_string()))?;
        if !status.is_success() {
            return Err(CoreError::Http(format!("HTTP {status}: {text}")));
        }
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(Ok(Value::Null));
        }
        let value: Value = serde_json::from_str(trimmed).map_err(CoreError::Json)?;
        self.trace_line(&trace::trace_event("http_response", &value));
        Self::parse_single_response(value)
    }

    fn parse_single_response(
        value: Value,
    ) -> Result<std::result::Result<Value, RpcError>, CoreError> {
        if let Some(err) = value.get("error") {
            let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
            let message = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            let data = err.get("data").cloned();
            return Ok(Err(RpcError {
                code,
                message,
                data,
            }));
        }
        if let Some(result) = value.get("result") {
            return Ok(Ok(result.clone()));
        }
        Err(CoreError::UnexpectedMessage(format!(
            "unrecognized HTTP JSON body: {value}"
        )))
    }

    fn handshake_http(&mut self, options: &ConnectOptions) -> Result<(), CoreError> {
        let init_id = Value::from(1_u64);
        let params = json!({
            "protocolVersion": options.protocol_version,
            "capabilities": {},
            "clientInfo": {
                "name": options.client_info.name,
                "version": options.client_info.version,
            }
        });
        let body = json!({
            "jsonrpc": "2.0",
            "id": init_id,
            "method": "initialize",
            "params": params
        });
        let init_result = match self.post_json(&body)? {
            Ok(v) => v,
            Err(e) => return Err(CoreError::Handshake(format!("initialize failed: {e}"))),
        };
        validate_initialize_result(&init_result)?;
        self.capabilities = init_result
            .get("capabilities")
            .cloned()
            .unwrap_or_else(|| json!({}));

        let initialized = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        let _ = self.post_json(&initialized)?;
        Ok(())
    }

    pub fn call_outcome(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<std::result::Result<Value, RpcError>, CoreError> {
        let id = Value::from(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        let body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        self.post_json(&body)
    }

    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, CoreError> {
        match self.call_outcome(method, params)? {
            Ok(v) => Ok(v),
            Err(e) => Err(CoreError::JsonRpc(e)),
        }
    }
}

/// Unified MCP client session (stdio subprocess or HTTP POST).
pub enum McpSession {
    Stdio(McpStdioSession),
    Http(McpHttpSession),
}

impl McpSession {
    pub fn connect(server: &ServerSpec, options: ConnectOptions) -> Result<Self, CoreError> {
        if server.http.is_some() {
            Ok(Self::Http(McpHttpSession::connect(server, options)?))
        } else {
            Ok(Self::Stdio(McpStdioSession::spawn(server, options)?))
        }
    }

    pub fn capabilities(&self) -> &Value {
        match self {
            Self::Stdio(s) => &s.capabilities,
            Self::Http(h) => &h.capabilities,
        }
    }

    pub fn call_outcome(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<std::result::Result<Value, RpcError>, CoreError> {
        match self {
            Self::Stdio(s) => s.call_outcome(method, params),
            Self::Http(h) => h.call_outcome(method, params),
        }
    }

    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, CoreError> {
        match self {
            Self::Stdio(s) => s.call(method, params),
            Self::Http(h) => h.call(method, params),
        }
    }
}
