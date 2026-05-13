//! Minimal TCP HTTP/1.1 fixture for [`McpHttpSession`] (handshake, session header, one RPC).

use mcp_check_core::{ConnectOptions, HttpEndpoint, McpHttpSession, ServerSpec};
use serde_json::json;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn find_headers_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn parse_content_length(headers: &[u8]) -> std::io::Result<usize> {
    let text = String::from_utf8_lossy(headers);
    for line in text.split("\r\n") {
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            return rest.trim().parse().map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "bad content-length")
            });
        }
    }
    Ok(0)
}

fn read_one_http_request(stream: &mut impl Read) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 2048];
    loop {
        if let Some(end) = find_headers_end(&buf) {
            let header_end = end + 4;
            let cl = parse_content_length(&buf[..end])?;
            let total = header_end + cl;
            while buf.len() < total {
                let n = stream.read(&mut tmp)?;
                if n == 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "eof while reading body",
                    ));
                }
                buf.extend_from_slice(&tmp[..n]);
            }
            return Ok(buf[..total].to_vec());
        }
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            if buf.is_empty() {
                return Ok(Vec::new());
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "eof in headers",
            ));
        }
        buf.extend_from_slice(&tmp[..n]);
    }
}

fn json_body(req: &[u8]) -> &str {
    let end = find_headers_end(req).expect("headers");
    let start = end + 4;
    std::str::from_utf8(&req[start..]).expect("utf8 body")
}

fn write_http_json_response(
    stream: &mut impl Write,
    body: &str,
    session_id: Option<&str>,
) -> std::io::Result<()> {
    use std::fmt::Write as _;
    let mut head = String::new();
    writeln!(head, "HTTP/1.1 200 OK").unwrap();
    writeln!(head, "Content-Length: {}", body.len()).unwrap();
    writeln!(head, "Content-Type: application/json").unwrap();
    writeln!(head, "Connection: keep-alive").unwrap();
    if let Some(sid) = session_id {
        writeln!(head, "Mcp-Session-Id: {sid}").unwrap();
    }
    writeln!(head).unwrap();
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()
}

#[test]
fn http_session_handshake_propagates_session_id_and_call_succeeds() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}/", addr);
    let with_session_header = Arc::new(AtomicUsize::new(0));
    let with_session_header_srv = with_session_header.clone();

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
        for _ in 0..32 {
            let req = match read_one_http_request(&mut stream) {
                Ok(b) if b.is_empty() => break,
                Ok(b) => b,
                Err(_) => break,
            };
            if String::from_utf8_lossy(&req)
                .to_ascii_lowercase()
                .contains("mcp-session-id:")
            {
                with_session_header_srv.fetch_add(1, Ordering::SeqCst);
            }
            let body = json_body(&req);
            let msg: serde_json::Value = match serde_json::from_str(body.trim()) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
            let id = msg.get("id").cloned().unwrap_or(json!(null));
            match method {
                "initialize" => {
                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "protocolVersion": "2024-11-05",
                            "capabilities": { "tools": {} },
                            "serverInfo": { "name": "fixture", "version": "0" }
                        }
                    });
                    write_http_json_response(
                        &mut stream,
                        &resp.to_string(),
                        Some("fixture-session"),
                    )
                    .unwrap();
                }
                "notifications/initialized" => {
                    write_http_json_response(&mut stream, "", None).unwrap();
                }
                "tools/list" => {
                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": { "tools": [] }
                    });
                    write_http_json_response(&mut stream, &resp.to_string(), None).unwrap();
                }
                _ => {
                    let err = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32601, "message": method }
                    });
                    write_http_json_response(&mut stream, &err.to_string(), None).unwrap();
                }
            }
        }
    });

    let spec = ServerSpec {
        http: Some(HttpEndpoint {
            url,
            headers: HashMap::new(),
        }),
        command: String::new(),
        args: Vec::new(),
        cwd: None,
        env: HashMap::new(),
    };

    let opts = ConnectOptions {
        response_timeout: Duration::from_secs(5),
        ..ConnectOptions::default()
    };

    let mut session = McpHttpSession::connect(&spec, opts).expect("connect");
    let result = session.call("tools/list", json!({})).expect("tools/list");
    assert_eq!(result["tools"], json!([]));

    drop(session);
    server.join().expect("server join");

    assert!(
        with_session_header.load(Ordering::SeqCst) >= 2,
        "follow-up POSTs after initialize should include Mcp-Session-Id"
    );
}
