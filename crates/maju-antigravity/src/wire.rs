use serde_json::{json, Value};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32000;

#[derive(Debug)]
pub enum Inbound {
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    Notification {
        method: String,
        params: Value,
    },
    Ignored,
    Invalid {
        id: Value,
        message: String,
    },
}

pub type Sender = mpsc::Sender<Value>;

pub fn classify(message: &Value) -> Inbound {
    if !message.is_object() || message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Inbound::Invalid {
            id: message.get("id").cloned().unwrap_or(Value::Null),
            message: "missing or invalid JSON-RPC version".to_string(),
        };
    }
    let method = message
        .get("method")
        .and_then(Value::as_str)
        .map(str::to_string);
    let id = message.get("id").cloned();
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    match (method, id) {
        (Some(method), Some(id)) => Inbound::Request { id, method, params },
        (Some(method), None) => Inbound::Notification { method, params },
        (None, Some(_)) => Inbound::Ignored,
        (None, None) => Inbound::Invalid {
            id: Value::Null,
            message: "missing JSON-RPC method and id".to_string(),
        },
    }
}

pub fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

pub fn error(id: Value, code: i32, message: impl AsRef<str>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message.as_ref() }
    })
}

pub fn session_update(session_id: &str, update: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": { "sessionId": session_id, "update": update }
    })
}

pub fn usage_update(session_id: &str, update: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "_goose/unstable/session/update",
        "params": { "sessionId": session_id, "update": update }
    })
}

pub async fn send(sender: &Sender, message: Value) {
    let _ = sender.send(message).await;
}

pub async fn read_line<R: AsyncBufRead + Unpin>(reader: &mut R) -> std::io::Result<Option<String>> {
    const MAX_LINE_BYTES: usize = 8 * 1024 * 1024;
    let mut bytes = Vec::new();
    loop {
        let chunk = reader.fill_buf().await?;
        if chunk.is_empty() {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                String::from_utf8(bytes)
                    .map(Some)
                    .map_err(|_| std::io::Error::other("ACP frame is not UTF-8"))
            };
        }
        let take = chunk
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(chunk.len(), |index| index + 1);
        if bytes.len().saturating_add(take) > MAX_LINE_BYTES {
            return Err(std::io::Error::other("ACP frame exceeds 8 MiB"));
        }
        bytes.extend_from_slice(&chunk[..take]);
        reader.consume(take);
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
            if bytes.last() == Some(&b'\r') {
                bytes.pop();
            }
            return String::from_utf8(bytes)
                .map(Some)
                .map_err(|_| std::io::Error::other("ACP frame is not UTF-8"));
        }
    }
}

pub async fn writer(mut receiver: mpsc::Receiver<Value>) {
    let mut stdout = tokio::io::stdout();
    while let Some(message) = receiver.recv().await {
        let Ok(mut line) = serde_json::to_vec(&message) else {
            continue;
        };
        line.push(b'\n');
        if stdout.write_all(&line).await.is_err() {
            return;
        }
        if stdout.flush().await.is_err() {
            return;
        }
    }
}
