use serde_json::{json, Map, Value};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

/// Best-effort redaction for trace output (keys only; does not understand nested secrets in opaque strings).
pub fn redact_secrets(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                let lk = k.to_ascii_lowercase();
                if lk.contains("password")
                    || lk.contains("token")
                    || lk.contains("secret")
                    || lk.contains("authorization")
                {
                    out.insert(k.clone(), Value::String("***".to_string()));
                } else {
                    out.insert(k.clone(), redact_secrets(v));
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact_secrets).collect()),
        other => other.clone(),
    }
}

/// Append a single NDJSON trace record (creates parent dirs if possible; ignores some errors).
pub fn append_ndjson(path: &Path, record: &Value) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    let redacted = redact_secrets(record);
    writeln!(f, "{}", serde_json::to_string(&redacted)?)?;
    Ok(())
}

pub fn trace_event(kind: &str, payload: &Value) -> Value {
    json!({ "kind": kind, "payload": redact_secrets(payload) })
}
