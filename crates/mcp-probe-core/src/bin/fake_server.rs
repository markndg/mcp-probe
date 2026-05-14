//! Minimal MCP stdio server used by integration tests and local experiments.
//!
//! Supports `initialize`, `notifications/initialized`, `tools/list`,
//! `resources/list`, and `prompts/list`.

use serde_json::{json, Value};
use std::io::{BufRead, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    let reader = std::io::BufReader::new(stdin.lock());

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let msg: Value = match serde_json::from_str(trimmed) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let Some(method) = msg.get("method").and_then(|m| m.as_str()) else {
            continue;
        };

        if method.starts_with("notifications/") {
            continue;
        }

        let Some(id) = msg.get("id") else {
            continue;
        };

        match method {
            "initialize" => {
                let negotiated = msg
                    .get("params")
                    .and_then(|p| p.get("protocolVersion"))
                    .cloned()
                    .unwrap_or_else(|| json!("2024-11-05"));
                let result = json!({
                    "protocolVersion": negotiated,
                    "capabilities": {
                        "tools": {},
                        "resources": {},
                        "prompts": {}
                    },
                    "serverInfo": {
                        "name": "mcp_probe_fake_server",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                });
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": result
                });
                writeln!(out, "{}", serde_json::to_string(&resp)?)?;
                out.flush()?;
            }
            "tools/list" => {
                let result = json!({
                    "tools": [{
                        "name": "demo_tool",
                        "description": "A fake tool for tests.",
                        "inputSchema": { "type": "object", "properties": {} }
                    }]
                });
                let resp = json!({ "jsonrpc": "2.0", "id": id, "result": result });
                writeln!(out, "{}", serde_json::to_string(&resp)?)?;
                out.flush()?;
            }
            "resources/list" => {
                let result = json!({ "resources": [] });
                let resp = json!({ "jsonrpc": "2.0", "id": id, "result": result });
                writeln!(out, "{}", serde_json::to_string(&resp)?)?;
                out.flush()?;
            }
            "prompts/list" => {
                let result = json!({ "prompts": [] });
                let resp = json!({ "jsonrpc": "2.0", "id": id, "result": result });
                writeln!(out, "{}", serde_json::to_string(&resp)?)?;
                out.flush()?;
            }
            _ => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": format!("method not found: {method}")
                    }
                });
                writeln!(out, "{}", serde_json::to_string(&resp)?)?;
                out.flush()?;
            }
        }
    }

    Ok(())
}
