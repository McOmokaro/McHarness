//! Transports: how the client reaches a server.
//!
//! Forged from Goose `extension_manager/stdio.rs`, `streamable_http.rs`, and
//! `builtin.rs`. Every transport speaks the same newline-delimited JSON-RPC 2.0
//! framing, so the client above never knows which one it has — stdio subprocess,
//! HTTP/SSE, or an in-process duplex pipe (native tools talking MCP to
//! themselves).

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

use crate::env;
use crate::error::{McpError, McpResult, ProcessExit};
use crate::message::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, JSONRPC_VERSION};

/// A framed request/response channel to one MCP server.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send a request and await its matching response. Server notifications are
    /// skipped while waiting.
    async fn request(&self, method: &str, params: Option<Value>) -> McpResult<Value>;

    /// Send a fire-and-forget notification.
    async fn notify(&self, method: &str, params: Option<Value>) -> McpResult<()>;
}

/// The framing core, generic over any byte pipe. This is the piece that is unit-
/// tested; the stdio and duplex transports are thin wrappers around it.
pub struct JsonRpcTransport<R, W> {
    reader: Mutex<BufReader<R>>,
    writer: Mutex<W>,
    next_id: AtomicU64,
}

impl<R, W> JsonRpcTransport<R, W>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader: Mutex::new(BufReader::new(reader)),
            writer: Mutex::new(writer),
            next_id: AtomicU64::new(0),
        }
    }

    async fn send_line(&self, line: &str) -> McpResult<()> {
        // The write lock is held only for the write; the response wait happens
        // outside it so concurrent calls overlap.
        let mut writer = self.writer.lock().await;
        writer.write_all(line.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
        Ok(())
    }
}

#[async_trait]
impl<R, W> Transport for JsonRpcTransport<R, W>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    async fn request(&self, method: &str, params: Option<Value>) -> McpResult<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let req = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            method: method.to_string(),
            params,
        };
        self.send_line(&serde_json::to_string(&req)?).await?;

        loop {
            let mut buf = String::new();
            let n = {
                let mut reader = self.reader.lock().await;
                reader.read_line(&mut buf).await?
            };
            if n == 0 {
                return Err(McpError::TransportClosed);
            }
            let trimmed = buf.trim();
            if trimmed.is_empty() {
                continue;
            }
            // A server notification (no matching id) or unparseable noise: skip.
            let parsed: JsonRpcResponse = match serde_json::from_str(trimmed) {
                Ok(p) => p,
                Err(_) => continue,
            };
            if parsed.id != Some(id) {
                continue;
            }
            if let Some(err) = parsed.error {
                return Err(McpError::Server {
                    code: err.code,
                    message: err.message,
                });
            }
            return parsed.result.ok_or(McpError::UnexpectedResponse);
        }
    }

    async fn notify(&self, method: &str, params: Option<Value>) -> McpResult<()> {
        let note = JsonRpcNotification {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.to_string(),
            params,
        };
        self.send_line(&serde_json::to_string(&note)?).await
    }
}

/// A stdio transport: a spawned child process whose stdin/stdout are the pipe.
///
/// Mirrors Goose `stdio.rs`: the child's stderr is drained into a shared buffer,
/// and a transport death is reported as `ProcessExit` carrying that stderr.
pub struct StdioTransport {
    inner: JsonRpcTransport<ChildStdout, ChildStdin>,
    child: Mutex<Child>,
    stderr: Arc<Mutex<String>>,
}

impl StdioTransport {
    /// The captured stderr so far (used to build a `ProcessExit`).
    pub async fn stderr(&self) -> String {
        self.stderr.lock().await.clone()
    }

    /// Kill the child, if still running.
    pub async fn shutdown(&self) {
        let mut child = self.child.lock().await;
        let _ = child.kill().await;
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn request(&self, method: &str, params: Option<Value>) -> McpResult<Value> {
        match self.inner.request(method, params).await {
            Err(McpError::TransportClosed) => Err(McpError::ProcessExit(
                ProcessExit::new(self.stderr().await, format!("request `{method}` failed")).into()
            )),
            other => other,
        }
    }

    async fn notify(&self, method: &str, params: Option<Value>) -> McpResult<()> {
        self.inner.notify(method, params).await
    }
}

/// Spawn an MCP server as a child process and connect over its stdio.
///
/// The `env` block is filtered through [`crate::env::sanitize_env`] first: a config
/// cannot set `LD_PRELOAD` or `NODE_OPTIONS`.
pub async fn spawn_stdio(
    cmd: &str,
    args: &[String],
    envs: HashMap<String, String>,
) -> McpResult<StdioTransport> {
    use std::process::Stdio;
    use tokio::process::Command;

    let mut command = Command::new(cmd);
    command
        .args(args)
        .envs(crate::env::sanitize_env(envs))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn()?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| McpError::Initialize("failed to capture child stdin".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| McpError::Initialize("failed to capture child stdout".into()))?;

    let stderr_buf = Arc::new(Mutex::new(String::new()));
    if let Some(mut stderr) = child.stderr.take() {
        let buf = stderr_buf.clone();
        tokio::spawn(async move {
            let mut out = String::new();
            let _ = stderr.read_to_string(&mut out).await;
            *buf.lock().await = out;
        });
    }

    Ok(StdioTransport {
        inner: JsonRpcTransport::new(stdout, stdin),
        child: Mutex::new(child),
        stderr: stderr_buf,
    })
}

/// An in-process transport over a pair of duplex pipes. Native McHarness tools
/// speak MCP to themselves through this, so there is no separate "internal tool"
/// code path.
pub type DuplexTransport = JsonRpcTransport<tokio::io::DuplexStream, tokio::io::DuplexStream>;

/// Construct a duplex client transport plus the server's connected stream pair.
/// Hand `(server_read, server_write)` to the in-process server task.
pub fn duplex_pair() -> (
    DuplexTransport,
    (tokio::io::DuplexStream, tokio::io::DuplexStream),
) {
    let (server_read, client_write) = tokio::io::duplex(65536);
    let (client_read, server_write) = tokio::io::duplex(65536);
    let transport = JsonRpcTransport::new(client_read, client_write);
    (transport, (server_read, server_write))
}

/// An HTTP transport (streamable HTTP + SSE), mirroring Goose `streamable_http.rs`.
///
/// Handles the single-response JSON case and the SSE case. Auth challenges are
/// surfaced as `ConfigError`, which the caller turns into an OAuth flow — Goose
/// seeds that flow from the server's `WWW-Authenticate` header.
pub struct HttpTransport {
    client: reqwest::Client,
    uri: String,
    headers: HashMap<String, String>,
    next_id: std::sync::atomic::AtomicU64,
}

impl HttpTransport {
    pub fn new(uri: impl Into<String>, headers: HashMap<String, String>) -> McpResult<Self> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(|e| McpError::Initialize(format!("http client: {e}")))?;
        Ok(Self {
            client,
            uri: uri.into(),
            headers,
            next_id: std::sync::atomic::AtomicU64::new(0),
        })
    }

    async fn post(&self, body: &Value) -> McpResult<String> {
        let mut rb = self
            .client
            .post(&self.uri)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(body.to_string());
        for (k, v) in &self.headers {
            rb = rb.header(k, v);
        }
        let resp = rb
            .send()
            .await
            .map_err(|e| McpError::Initialize(format!("request failed: {e}")))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| McpError::Initialize(format!("reading body: {e}")))?;
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(McpError::ConfigError(format!(
                "auth required (HTTP {})",
                status.as_u16()
            )));
        }
        if !status.is_success() {
            return Err(McpError::Server {
                code: status.as_u16() as i64,
                message: text,
            });
        }
        Ok(text)
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn request(&self, method: &str, params: Option<Value>) -> McpResult<Value> {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let req = crate::message::JsonRpcRequest {
            jsonrpc: crate::message::JSONRPC_VERSION.to_string(),
            id,
            method: method.to_string(),
            params,
        };
        let body = serde_json::to_value(&req)?;
        let text = self.post(&body).await?;

        // Streamable HTTP may answer with a single JSON body or an SSE stream.
        let looks_like_sse = text.trim_start().starts_with("event:")
            || text.trim_start().starts_with("data:")
            || text.contains("\ndata:");
        let payload = if looks_like_sse {
            parse_sse_events(&text)
                .into_iter()
                .filter_map(|d| serde_json::from_str::<crate::message::JsonRpcResponse>(&d).ok())
                .find(|r| r.id == Some(id))
                .ok_or(McpError::UnexpectedResponse)?
        } else {
            serde_json::from_str::<crate::message::JsonRpcResponse>(&text)?
        };

        if let Some(err) = payload.error {
            return Err(McpError::Server {
                code: err.code,
                message: err.message,
            });
        }
        payload.result.ok_or(McpError::UnexpectedResponse)
    }

    async fn notify(&self, method: &str, params: Option<Value>) -> McpResult<()> {
        let note = crate::message::JsonRpcNotification {
            jsonrpc: crate::message::JSONRPC_VERSION.to_string(),
            method: method.to_string(),
            params,
        };
        self.post(&serde_json::to_value(&note)?).await.map(|_| ())
    }
}

/// Parse Server-Sent Events into a list of `data` payloads (one per event).
pub fn parse_sse_events(input: &str) -> Vec<String> {
    let mut events = Vec::new();
    let mut data = String::new();
    for line in input.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            if !data.is_empty() {
                events.push(std::mem::take(&mut data));
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
        }
    }
    if !data.is_empty() {
        events.push(data);
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::DuplexStream;

    /// An in-memory "server": answers every request with a fixed result.
    async fn echo_server(mut server_rx: DuplexStream, mut server_tx: DuplexStream, result: Value) {
        let mut reader = tokio::io::BufReader::new(&mut server_rx);
        let mut line = String::new();
        loop {
            line.clear();
            let n = reader.read_line(&mut line).await.unwrap();
            if n == 0 {
                break;
            }
            let req: crate::message::JsonRpcRequest = match serde_json::from_str(line.trim()) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let resp = json!({ "jsonrpc": "2.0", "id": req.id, "result": result.clone() });
            let _ = server_tx.write_all(format!("{resp}\n").as_bytes()).await;
            let _ = server_tx.flush().await;
        }
    }

    #[tokio::test]
    async fn framing_roundtrip() {
        let (transport, (server_read, server_write)) = duplex_pair();
        tokio::spawn(echo_server(server_read, server_write, json!({ "ok": true })));
        let res = transport.request("ping", None).await.unwrap();
        assert_eq!(res, json!({ "ok": true }));
    }

    #[tokio::test]
    async fn notification_does_not_break_framing() {
        let (server_read, client_write) = tokio::io::duplex(4096);
        let (client_read, server_write) = tokio::io::duplex(4096);
        let transport = JsonRpcTransport::new(client_read, client_write);

        tokio::spawn(async move {
            let mut server_rx = server_read;
            let mut server_tx = server_write;
            let mut reader = tokio::io::BufReader::new(&mut server_rx);
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            let req: crate::message::JsonRpcRequest = serde_json::from_str(line.trim()).unwrap();
            // Emit a notification first (no id), then the real response.
            let _ = server_tx
                .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\"}\n")
                .await;
            let resp = json!({ "jsonrpc": "2.0", "id": req.id, "result": 42 });
            let _ = server_tx.write_all(format!("{resp}\n").as_bytes()).await;
            let _ = server_tx.flush().await;
        });

        let res = transport.request("ping", None).await.unwrap();
        assert_eq!(res, json!(42));
    }

    #[tokio::test]
    async fn server_error_maps_to_mcp_error() {
        let (server_read, client_write) = tokio::io::duplex(4096);
        let (client_read, server_write) = tokio::io::duplex(4096);
        let transport = JsonRpcTransport::new(client_read, client_write);

        tokio::spawn(async move {
            let mut server_rx = server_read;
            let mut server_tx = server_write;
            let mut reader = tokio::io::BufReader::new(&mut server_rx);
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            let req: crate::message::JsonRpcRequest = serde_json::from_str(line.trim()).unwrap();
            let resp = json!({ "jsonrpc": "2.0", "id": req.id, "error": { "code": -32601, "message": "no such method" } });
            let _ = server_tx.write_all(format!("{resp}\n").as_bytes()).await;
            let _ = server_tx.flush().await;
        });

        let err = transport.request("bogus", None).await.unwrap_err();
        match err {
            McpError::Server { code, message } => {
                assert_eq!(code, -32601);
                assert_eq!(message, "no such method");
            }
            other => panic!("expected Server error, got {other:?}"),
        }
    }

    #[test]
    fn sse_parsing() {
        let body = "event: message\ndata: {\"a\":1}\n\ndata: {\"b\":2}\n\n";
        let events = parse_sse_events(body);
        assert_eq!(events, vec!["{\"a\":1}", "{\"b\":2}"]);
    }
}