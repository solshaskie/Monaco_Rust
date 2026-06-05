use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A pending request waiting for a JSON-RPC response.
enum ResponseSender {
    Sync(std::sync::mpsc::Sender<Result<Value, String>>),
    Async(tokio::sync::oneshot::Sender<Result<Value, String>>),
}

struct PendingRequest {
    sender: ResponseSender,
}

/// Shared state between the client handle and the background reader thread.
struct ClientState {
    next_id: i64,
    pending: HashMap<String, PendingRequest>,
    shut_down: bool,
}

/// A thin JSON-RPC LSP client that communicates with a language server
/// over stdin/stdout.
pub struct LspClient {
    state: Arc<Mutex<ClientState>>,
    stdin: Arc<Mutex<ChildStdin>>,
    _child: Child,
    _reader_thread: Option<thread::JoinHandle<()>>,
}

/// A JSON-RPC request envelope.
#[derive(Serialize)]
struct JsonRpcRequest<T> {
    jsonrpc: String,
    id: String,
    method: String,
    params: T,
}

/// A JSON-RPC response envelope.
#[derive(Deserialize, Debug)]
struct JsonRpcResponse {
    _jsonrpc: String,
    id: Option<Value>,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
}

#[derive(Deserialize, Debug)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(default)]
    _data: Option<Value>,
}

impl LspClient {
    /// Spawns a language server process and performs the LSP initialize handshake.
    pub fn spawn(command: &str, args: &[&str], workspace_root: &str) -> Result<Self, String> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn {}: {}", command, e))?;

        let stdin = child.stdin.take().ok_or("missing stdin")?;
        let stdout = child.stdout.take().ok_or("missing stdout")?;

        let state = Arc::new(Mutex::new(ClientState {
            next_id: 1,
            pending: HashMap::new(),
            shut_down: false,
        }));

        let reader_state = Arc::clone(&state);
        let reader_thread = thread::spawn(move || {
            Self::reader_loop(stdout, reader_state);
        });

        let mut client = LspClient {
            state: Arc::clone(&state),
            stdin: Arc::new(Mutex::new(stdin)),
            _child: child,
            _reader_thread: Some(reader_thread),
        };

        // Perform initialize handshake
        let init_params = serde_json::json!({
            "processId": std::process::id() as i32,
            "rootUri": format!("file://{}", workspace_root),
            "capabilities": {},
            "clientInfo": {
                "name": "monaco-tauri",
                "version": env!("CARGO_PKG_VERSION"),
            },
        });

        let _: Value = client
            .request("initialize", init_params)
            .map_err(|e| format!("LSP initialize failed: {}", e))?;

        // Send initialized notification
        client.notify("initialized", serde_json::json!({}));

        Ok(client)
    }

    /// Sends a JSON-RPC request and blocks for the response (with custom timeout).
    pub fn request_with_timeout(
        &mut self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, String> {
        let id = {
            let mut state = self.state.lock().unwrap();
            let id = state.next_id;
            state.next_id += 1;
            id.to_string()
        };

        let (tx, rx) = std::sync::mpsc::channel();
        {
            let mut state = self.state.lock().unwrap();
            state
                .pending
                .insert(id.clone(), PendingRequest { sender: ResponseSender::Sync(tx) });
        }

        let envelope = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: id.clone(),
            method: method.to_string(),
            params,
        };

        let body = serde_json::to_string(&envelope).map_err(|e| e.to_string())?;
        let msg = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);

        {
            let mut stdin = self.stdin.lock().unwrap();
            stdin.write_all(msg.as_bytes()).map_err(|e| e.to_string())?;
            stdin.flush().map_err(|e| e.to_string())?;
        }

        match rx.recv_timeout(timeout) {
            Ok(result) => {
                let mut state = self.state.lock().unwrap();
                state.pending.remove(&id);
                result
            }
            Err(_) => {
                let mut state = self.state.lock().unwrap();
                state.pending.remove(&id);
                Err(format!("LSP request '{}' timed out", method))
            }
        }
    }

    /// Sends a JSON-RPC request and blocks for the response (with 2s timeout).
    pub fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.request_with_timeout(method, params, Duration::from_secs(2))
    }

    /// Sends a JSON-RPC request asynchronously, returning a future.
    pub async fn request_async(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        let id = {
            let mut state = self.state.lock().unwrap();
            let id = state.next_id;
            state.next_id += 1;
            id.to_string()
        };

        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut state = self.state.lock().unwrap();
            state
                .pending
                .insert(id.clone(), PendingRequest { sender: ResponseSender::Async(tx) });
        }

        let envelope = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: id.clone(),
            method: method.to_string(),
            params,
        };

        let body = serde_json::to_string(&envelope).map_err(|e| e.to_string())?;
        let msg = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);

        {
            let mut stdin = self.stdin.lock().unwrap();
            stdin.write_all(msg.as_bytes()).map_err(|e| e.to_string())?;
            stdin.flush().map_err(|e| e.to_string())?;
        }

        match rx.await {
            Ok(result) => {
                let mut state = self.state.lock().unwrap();
                state.pending.remove(&id);
                result
            }
            Err(_) => {
                let mut state = self.state.lock().unwrap();
                state.pending.remove(&id);
                Err(format!("LSP request '{}' cancelled", method))
            }
        }
    }

    /// Attempts a graceful shutdown with a bounded timeout.
    /// Returns Ok even if the server does not respond in time.
    pub fn try_shutdown(&mut self) -> Result<(), String> {
        let _ = self.request_with_timeout(
            "shutdown",
            serde_json::json!({}),
            Duration::from_millis(500),
        );
        self.notify("exit", serde_json::json!({}));
        let mut state = self.state.lock().unwrap();
        state.shut_down = true;
        Ok(())
    }

    /// Sends a JSON-RPC notification (no response expected).
    pub fn notify(&mut self, method: &str, params: Value) {
        let envelope = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        if let Ok(body) = serde_json::to_string(&envelope) {
            let msg = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
            if let Ok(mut stdin) = self.stdin.lock() {
                let _ = stdin.write_all(msg.as_bytes());
                let _ = stdin.flush();
            }
        }
    }

    /// Notifies the server that a document was opened.
    pub fn did_open(&mut self, uri: &str, language_id: &str, version: i32, text: &str) {
        let params = serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": language_id,
                "version": version,
                "text": text,
            }
        });
        self.notify("textDocument/didOpen", params);
    }

    /// Notifies the server that a document was changed.
    pub fn did_change(&mut self, uri: &str, version: i32, changes: Vec<Value>) {
        let params = serde_json::json!({
            "textDocument": {
                "uri": uri,
                "version": version,
            },
            "contentChanges": changes,
        });
        self.notify("textDocument/didChange", params);
    }

    /// Notifies the server that a document was closed.
    pub fn did_close(&mut self, uri: &str) {
        let params = serde_json::json!({
            "textDocument": {
                "uri": uri,
            }
        });
        self.notify("textDocument/didClose", params);
    }

    /// Shuts down the client gracefully.
    pub fn shutdown(&mut self) -> Result<(), String> {
        let _ = self.request("shutdown", serde_json::json!({}))?;
        self.notify("exit", serde_json::json!({}));
        let mut state = self.state.lock().unwrap();
        state.shut_down = true;
        Ok(())
    }

    /// Background thread that reads JSON-RPC responses from the server stdout.
    fn reader_loop(stdout: ChildStdout, state: Arc<Mutex<ClientState>>) {
        let mut reader = BufReader::new(stdout);
        let mut buffer = String::new();

        loop {
            buffer.clear();
            // Read Content-Length header
            match reader.read_line(&mut buffer) {
                Ok(0) => break, // EOF
                Ok(_) => {}
                Err(_) => break,
            }

            let content_length = buffer
                .trim()
                .strip_prefix("Content-Length: ")
                .and_then(|v| v.parse::<usize>().ok());

            if content_length.is_none() {
                // Skip empty lines or unexpected headers
                continue;
            }
            let content_length = content_length.unwrap();

            // Read blank line after headers
            buffer.clear();
            let _ = reader.read_line(&mut buffer);

            // Read JSON body
            let mut body = vec![0u8; content_length];
            if reader.read_exact(&mut body).is_err() {
                break;
            }

            let response: JsonRpcResponse = match serde_json::from_slice(&body) {
                Ok(r) => r,
                Err(_) => continue,
            };

            if let Some(id) = response.id {
                let id_str = match id {
                    Value::String(s) => s,
                    Value::Number(n) => n.to_string(),
                    _ => continue,
                };
                let sender = {
                    let mut state = state.lock().unwrap();
                    state.pending.remove(&id_str)
                };
                if let Some(pending) = sender {
                    let result = if let Some(err) = response.error {
                        Err(format!("LSP error {}: {}", err.code, err.message))
                    } else {
                        Ok(response.result.unwrap_or(Value::Null))
                    };
                    match pending.sender {
                        ResponseSender::Sync(tx) => { let _ = tx.send(result); }
                        ResponseSender::Async(tx) => { let _ = tx.send(result); }
                    }
                }
            }
        }
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let is_shutdown = self.state.lock().unwrap().shut_down;
        if !is_shutdown {
            let _ = self.try_shutdown();
        }
    }
}
