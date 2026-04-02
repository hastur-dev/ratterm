//! DAP (Debug Adapter Protocol) client.
//!
//! Spawns a debug adapter process and communicates via stdin/stdout using
//! DAP JSON-RPC framing with Content-Length headers.

use std::collections::HashMap;
use std::io::{BufRead, Write as IoWrite};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;

use serde_json::{Value, json};

use super::DebugEvent;
use super::callstack::StackFrame;
use super::launch::{AdapterKind, LaunchConfig};
use super::variables::Variable;

/// Error type for DAP client operations.
#[derive(Debug, thiserror::Error)]
pub enum DapError {
    /// Failed to spawn the adapter process.
    #[error("Failed to spawn adapter: {0}")]
    SpawnFailed(String),
    /// Failed to send a message to the adapter.
    #[error("Send failed: {0}")]
    SendFailed(String),
    /// Failed to read a response from the adapter.
    #[error("Read failed: {0}")]
    ReadFailed(String),
    /// The adapter returned an error response.
    #[error("Adapter error: {0}")]
    AdapterError(String),
    /// Protocol-level error.
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    /// The adapter is not connected.
    #[error("Not connected")]
    NotConnected,
}

/// DAP client that communicates with a debug adapter process.
pub struct DapClient {
    /// The adapter child process.
    child: Option<Child>,
    /// Channel to send events back to the app.
    event_tx: mpsc::Sender<DebugEvent>,
    /// Next sequence number.
    seq: i64,
    /// Pending requests awaiting responses.
    pending: HashMap<i64, String>,
}

impl DapClient {
    /// Creates a new DAP client with an event sender channel.
    #[must_use]
    pub fn new(event_tx: mpsc::Sender<DebugEvent>) -> Self {
        Self {
            child: None,
            event_tx,
            seq: 1,
            pending: HashMap::new(),
        }
    }

    /// Spawns the debug adapter process.
    pub fn spawn(&mut self, adapter: &AdapterKind) -> Result<(), DapError> {
        let executable = adapter.executable();
        let child = Command::new(executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| DapError::SpawnFailed(format!("{}: {}", executable, e)))?;

        self.child = Some(child);
        Ok(())
    }

    /// Sends an initialize request to the adapter.
    pub fn initialize(&mut self) -> Result<i64, DapError> {
        let args = json!({
            "clientID": "ratterm",
            "clientName": "Ratterm",
            "adapterID": "ratterm",
            "linesStartAt1": true,
            "columnsStartAt1": true,
            "pathFormat": "path",
            "supportsVariableType": true,
            "supportsVariablePaging": false,
            "supportsRunInTerminalRequest": false,
        });
        self.send_request("initialize", args)
    }

    /// Sends a launch request.
    pub fn launch(&mut self, config: &LaunchConfig) -> Result<i64, DapError> {
        let mut args = json!({
            "program": config.program,
            "args": config.args,
            "stopOnEntry": config.stop_on_entry,
        });

        if let Some(ref cwd) = config.cwd {
            args["cwd"] = json!(cwd);
        }

        if !config.env.is_empty() {
            args["env"] = json!(config.env);
        }

        self.send_request("launch", args)
    }

    /// Sets breakpoints for a file.
    pub fn set_breakpoints(&mut self, file: &str, lines: &[u32]) -> Result<i64, DapError> {
        let breakpoints: Vec<Value> = lines.iter().map(|&line| json!({"line": line})).collect();

        let args = json!({
            "source": {"path": file},
            "breakpoints": breakpoints,
        });

        self.send_request("setBreakpoints", args)
    }

    /// Sends a continue request.
    pub fn continue_execution(&mut self, thread_id: i64) -> Result<i64, DapError> {
        self.send_request("continue", json!({"threadId": thread_id}))
    }

    /// Sends a step over (next) request.
    pub fn step_over(&mut self, thread_id: i64) -> Result<i64, DapError> {
        self.send_request("next", json!({"threadId": thread_id}))
    }

    /// Sends a step in request.
    pub fn step_in(&mut self, thread_id: i64) -> Result<i64, DapError> {
        self.send_request("stepIn", json!({"threadId": thread_id}))
    }

    /// Sends a step out request.
    pub fn step_out(&mut self, thread_id: i64) -> Result<i64, DapError> {
        self.send_request("stepOut", json!({"threadId": thread_id}))
    }

    /// Sends a pause request.
    pub fn pause(&mut self, thread_id: i64) -> Result<i64, DapError> {
        self.send_request("pause", json!({"threadId": thread_id}))
    }

    /// Sends a terminate request.
    pub fn terminate(&mut self) -> Result<i64, DapError> {
        self.send_request("disconnect", json!({"terminateDebuggee": true}))
    }

    /// Requests a stack trace.
    pub fn stack_trace(&mut self, thread_id: i64) -> Result<i64, DapError> {
        self.send_request("stackTrace", json!({"threadId": thread_id}))
    }

    /// Requests scopes for a stack frame.
    pub fn scopes(&mut self, frame_id: i64) -> Result<i64, DapError> {
        self.send_request("scopes", json!({"frameId": frame_id}))
    }

    /// Requests variables for a variable reference.
    pub fn variables(&mut self, variables_reference: i64) -> Result<i64, DapError> {
        self.send_request(
            "variables",
            json!({"variablesReference": variables_reference}),
        )
    }

    /// Evaluates an expression in a given frame context.
    pub fn evaluate(&mut self, expr: &str, frame_id: Option<i64>) -> Result<i64, DapError> {
        let mut args = json!({
            "expression": expr,
            "context": "repl",
        });
        if let Some(fid) = frame_id {
            args["frameId"] = json!(fid);
        }
        self.send_request("evaluate", args)
    }

    /// Sends a DAP request and returns the sequence number.
    fn send_request(&mut self, command: &str, arguments: Value) -> Result<i64, DapError> {
        let seq = self.seq;
        self.seq += 1;

        let message = json!({
            "seq": seq,
            "type": "request",
            "command": command,
            "arguments": arguments,
        });

        self.pending.insert(seq, command.to_string());
        self.send_message(&message)?;
        Ok(seq)
    }

    /// Writes a DAP message with Content-Length header to the adapter's stdin.
    fn send_message(&mut self, message: &Value) -> Result<(), DapError> {
        let child = self.child.as_mut().ok_or(DapError::NotConnected)?;
        let stdin = child.stdin.as_mut().ok_or(DapError::NotConnected)?;

        let body =
            serde_json::to_string(message).map_err(|e| DapError::SendFailed(e.to_string()))?;

        let header = format!("Content-Length: {}\r\n\r\n", body.len());

        stdin
            .write_all(header.as_bytes())
            .map_err(|e| DapError::SendFailed(e.to_string()))?;
        stdin
            .write_all(body.as_bytes())
            .map_err(|e| DapError::SendFailed(e.to_string()))?;
        stdin
            .flush()
            .map_err(|e| DapError::SendFailed(e.to_string()))?;

        Ok(())
    }

    /// Returns whether the client has a connected adapter.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.child.is_some()
    }

    /// Kills the adapter process.
    pub fn kill(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.child = None;
    }

    /// Returns the event sender (for testing / external use).
    #[must_use]
    pub fn event_sender(&self) -> &mpsc::Sender<DebugEvent> {
        &self.event_tx
    }
}

impl Drop for DapClient {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Parses a DAP stack trace response body into `StackFrame` objects.
#[must_use]
pub fn parse_stack_frames(body: &Value) -> Vec<StackFrame> {
    let Some(frames) = body.get("stackFrames").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    frames
        .iter()
        .filter_map(|f| {
            let id = f.get("id")?.as_i64()?;
            let name = f.get("name")?.as_str()?.to_string();
            let line = f.get("line")?.as_u64()? as u32;
            let column = f.get("column").and_then(|c| c.as_u64()).unwrap_or(0) as u32;
            let source_path = f
                .get("source")
                .and_then(|s| s.get("path"))
                .and_then(|p| p.as_str())
                .map(String::from);

            Some(StackFrame {
                id,
                name,
                source_path,
                line,
                column,
            })
        })
        .collect()
}

/// Parses a DAP variables response body into `Variable` objects.
#[must_use]
pub fn parse_variables(body: &Value) -> Vec<Variable> {
    let Some(vars) = body.get("variables").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    vars.iter()
        .filter_map(|v| {
            let name = v.get("name")?.as_str()?.to_string();
            let value = v.get("value")?.as_str()?.to_string();
            let type_name = v.get("type").and_then(|t| t.as_str()).map(String::from);
            let variables_reference = v
                .get("variablesReference")
                .and_then(|r| r.as_i64())
                .unwrap_or(0);

            let mut var = Variable::new(name, value);
            if let Some(tn) = type_name {
                var = var.with_type(tn);
            }
            if variables_reference > 0 {
                var = var.with_reference(variables_reference);
            }
            Some(var)
        })
        .collect()
}

/// Reads a single DAP message from a buffered reader.
///
/// Returns `None` at EOF.
pub fn read_dap_message(reader: &mut impl BufRead) -> Result<Option<Value>, DapError> {
    // Read headers until blank line
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let bytes_read = reader
            .read_line(&mut line)
            .map_err(|e| DapError::ReadFailed(e.to_string()))?;

        if bytes_read == 0 {
            return Ok(None); // EOF
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            break; // End of headers
        }

        if let Some(len_str) = trimmed.strip_prefix("Content-Length: ") {
            content_length = len_str.parse().ok();
        }
    }

    let len = content_length
        .ok_or_else(|| DapError::ProtocolError("Missing Content-Length header".to_string()))?;

    let mut body = vec![0u8; len];
    reader
        .read_exact(&mut body)
        .map_err(|e| DapError::ReadFailed(e.to_string()))?;

    let value: Value =
        serde_json::from_slice(&body).map_err(|e| DapError::ProtocolError(e.to_string()))?;

    Ok(Some(value))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_dap_client_new() {
        let (tx, _rx) = mpsc::channel();
        let client = DapClient::new(tx);
        assert!(!client.is_connected());
    }

    #[test]
    fn test_parse_stack_frames() {
        let body = json!({
            "stackFrames": [
                {
                    "id": 1,
                    "name": "main",
                    "source": {"path": "src/main.rs"},
                    "line": 42,
                    "column": 5
                },
                {
                    "id": 2,
                    "name": "foo",
                    "line": 10,
                    "column": 0
                }
            ]
        });

        let frames = parse_stack_frames(&body);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].name, "main");
        assert_eq!(frames[0].source_path.as_deref(), Some("src/main.rs"));
        assert_eq!(frames[0].line, 42);
        assert_eq!(frames[1].name, "foo");
        assert!(frames[1].source_path.is_none());
    }

    #[test]
    fn test_parse_stack_frames_empty() {
        let body = json!({"stackFrames": []});
        assert!(parse_stack_frames(&body).is_empty());

        let body = json!({});
        assert!(parse_stack_frames(&body).is_empty());
    }

    #[test]
    fn test_parse_variables() {
        let body = json!({
            "variables": [
                {
                    "name": "x",
                    "value": "42",
                    "type": "i32",
                    "variablesReference": 0
                },
                {
                    "name": "vec",
                    "value": "[1, 2, 3]",
                    "type": "Vec<i32>",
                    "variablesReference": 5
                }
            ]
        });

        let vars = parse_variables(&body);
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0].name, "x");
        assert_eq!(vars[0].value, "42");
        assert_eq!(vars[0].type_name.as_deref(), Some("i32"));
        assert!(!vars[0].is_expandable());

        assert_eq!(vars[1].name, "vec");
        assert!(vars[1].is_expandable());
    }

    #[test]
    fn test_parse_variables_empty() {
        let body = json!({"variables": []});
        assert!(parse_variables(&body).is_empty());

        let body = json!({});
        assert!(parse_variables(&body).is_empty());
    }

    #[test]
    fn test_read_dap_message() {
        let body = r#"{"type":"response","seq":1}"#;
        let raw = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        let mut reader = Cursor::new(raw.as_bytes().to_vec());

        let msg = read_dap_message(&mut reader).expect("should parse");
        assert!(msg.is_some());
        let msg = msg.expect("has value");
        assert_eq!(msg["type"], "response");
        assert_eq!(msg["seq"], 1);
    }

    #[test]
    fn test_read_dap_message_eof() {
        let mut reader = Cursor::new(Vec::<u8>::new());
        let msg = read_dap_message(&mut reader).expect("should handle EOF");
        assert!(msg.is_none());
    }

    #[test]
    fn test_read_dap_message_missing_header() {
        let raw = b"\r\n{\"type\":\"event\"}";
        let mut reader = Cursor::new(raw.to_vec());
        let result = read_dap_message(&mut reader);
        assert!(result.is_err());
    }
}
