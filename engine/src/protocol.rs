//! Engine protocol: newline-delimited JSON over stdin (commands) and stdout (replies and events).
//! See ARCHITECTURE.md. Logs never go to stdout.

use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Write;

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
pub struct Command {
    pub id: u64,
    pub cmd: String,
    #[serde(default)]
    pub args: Value,
}

pub fn parse_command(line: &str) -> Result<Command, String> {
    serde_json::from_str(line).map_err(|e| format!("Could not read command: {e}"))
}

pub fn reply_ok(id: u64, result: Value) -> Value {
    json!({ "id": id, "ok": true, "result": result })
}

pub fn reply_err(id: u64, message: &str) -> Value {
    json!({ "id": id, "ok": false, "error": message })
}

pub fn event(name: &str, data: Value) -> Value {
    json!({ "event": name, "data": data })
}

/// Writes one message as a single line to stdout. Stdout is locked per message so
/// lines from different threads never interleave.
pub fn send(msg: &Value) {
    let line = msg.to_string();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let _ = out.write_all(line.as_bytes());
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

pub fn send_event(name: &str, data: Value) {
    send(&event(name, data));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_command_with_and_without_args() {
        let c = parse_command(r#"{"id":3,"cmd":"get_status"}"#).unwrap();
        assert_eq!(c.id, 3);
        assert_eq!(c.cmd, "get_status");
        assert!(c.args.is_null());
        let c = parse_command(r#"{"id":4,"cmd":"start_recording","args":{"source":"desktop"}}"#).unwrap();
        assert_eq!(c.args["source"], "desktop");
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_command("not json").is_err());
        assert!(parse_command(r#"{"cmd":"x"}"#).is_err());
    }

    #[test]
    fn reply_shapes() {
        assert_eq!(reply_ok(1, json!({"a":1}))["ok"], true);
        let e = reply_err(2, "nope");
        assert_eq!(e["ok"], false);
        assert_eq!(e["error"], "nope");
        assert_eq!(event("hello", json!({}))["event"], "hello");
    }
}
