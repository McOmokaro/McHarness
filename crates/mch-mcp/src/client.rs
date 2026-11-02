//! The MCP client: handshake, request/timeout, tool listing, tool calls.
//!
//! Forged from Goose `crates/goose/src/agents/mcp_client.rs`. The two crown
//! jewels are here: the three-arm `tokio::select!` that turns a stuck request
//! into a timeout *and tells the server to stop*, and the paginated tool list.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::error::{McpError, McpResult};
use crate::message::{CallToolResult, ListToolsResult, Tool};
use crate::transport::Transport;

/// The default per-request budget, in seconds. Matches Goose
/// `DEFAULT_EXTENSION_TIMEOUT` and can be overridden per client.
pub const DEFAULT_TIMEOUT_SECS: u64 = 300;

pub struct McpClient {
    transport: Arc<dyn Transport>,
    timeout: Duration,
    server_info: Option<Value>,
}

impl McpClient {
    /// Connect a transport and run the MCP handshake.
    pub async fn connect<T: Transport + 'static>(
        transport: T,
        timeout: Duration,
    ) -> McpResult<Self> {
        Self::from_transport(Arc::new(transport), timeout).await
    }

    /// Connect from an already-boxed transport.
    pub async fn from_transport(
        transport: Arc<dyn Transport>,
        timeout: Duration,
    ) -> McpResult<Self> {
        let mut client = Self {
            transport,
            timeout,
            server_info: None,
        };
        client.handshake().await?;
        Ok(client)
    }

    async fn handshake(&mut self) -> McpResult<()> {
        let params = json!({
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {
                "name": "mch-mcp",
                "version": env!("CARGO_PKG_VERSION"),
            }
        });
        let result = self.request("initialize", Some(params)).await?;
        self.server_info = Some(result);
        self.transport
            .notify("notifications/initialized", None)
            .await?;
        Ok(())
    }

    pub fn server_info(&self) -> Option<&Value> {
        self.server_info.as_ref()
    }

    /// Send a request under the client's timeout. On timeout the client sends a
    /// `notifications/cancelled` upstream before returning, so the server stops
    /// working instead of running to completion and dumping a late result.
    pub async fn request(&self, method: &str, params: Option<Value>) -> McpResult<Value> {
        let fut = self.transport.request(method, params);
        tokio::select! {
            result = fut => result,
            _ = tokio::time::sleep(self.timeout) => {
                let _ = self
                    .transport
                    .notify(
                        "notifications/cancelled",
                        Some(json!({ "reason": "timed out" })),
                    )
                    .await;
                Err(McpError::Timeout(self.timeout))
            }
        }
    }

    /// List every tool the server advertises, following `nextCursor` to the end.
    pub async fn list_tools(&self) -> McpResult<Vec<Tool>> {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let params = match &cursor {
                Some(c) => json!({ "cursor": c }),
                None => json!({}),
            };
            let result = self.request("tools/list", Some(params)).await?;
            let page: ListToolsResult = serde_json::from_value(result)?;
            tools.extend(page.tools);
            match page.next_cursor {
                Some(next) if !next.is_empty() => cursor = Some(next),
                _ => break,
            }
        }
        Ok(tools)
    }

    /// Call a tool. A tool-level failure comes back as a `CallToolResult` with
    /// `is_error = true` — data the model can act on, not a transport error.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Option<Value>,
    ) -> McpResult<CallToolResult> {
        let params = match arguments {
            Some(args) => json!({ "name": name, "arguments": args }),
            None => json!({ "name": name }),
        };
        let result = self.request("tools/call", Some(params)).await?;
        let call: CallToolResult = serde_json::from_value(result)?;
        Ok(call)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Tool;
    use async_trait::async_trait;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    fn tool(name: &str) -> Tool {
        Tool {
            name: name.to_string(),
            description: None,
            input_schema: json!({ "type": "object" }),
            meta: None,
        }
    }

    /// A mock server driven by a queue of `tools/list` pages.
    struct PaginatedMock {
        pages: Mutex<VecDeque<Value>>,
    }

    #[async_trait]
    impl Transport for PaginatedMock {
        async fn request(&self, method: &str, _params: Option<Value>) -> McpResult<Value> {
            match method {
                "initialize" => Ok(json!({ "serverInfo": { "name": "mock" } })),
                "tools/list" => Ok(self
                    .pages
                    .lock()
                    .unwrap()
                    .pop_front()
                    .unwrap_or_else(|| json!({ "tools": [] }))),
                "tools/call" => Ok(json!({
                    "content": [{ "type": "text", "text": "ok" }],
                    "isError": false
                })),
                _ => Ok(Value::Null),
            }
        }

        async fn notify(&self, _method: &str, _params: Option<Value>) -> McpResult<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn paginates_the_tool_list() {
        let mock = PaginatedMock {
            pages: Mutex::new(VecDeque::from(vec![
                json!({ "tools": [tool("a")], "nextCursor": "c2" }),
                json!({ "tools": [tool("b")] }),
            ])),
        };
        let client = McpClient::connect(mock, Duration::from_secs(5)).await.unwrap();
        let tools = client.list_tools().await.unwrap();
        let names: Vec<_> = tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[tokio::test]
    async fn call_tool_preserves_is_error_flag() {
        let mock = PaginatedMock {
            pages: Mutex::new(VecDeque::new()),
        };
        let client = McpClient::connect(mock, Duration::from_secs(5)).await.unwrap();
        let result = client.call_tool("anything", None).await.unwrap();
        assert!(!result.is_error);
        assert!(!result.is_error_result());
    }

    /// A mock that answers `initialize` fast but stalls other calls, recording
    /// notifications it receives.
    struct SlowMock {
        notified: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl Transport for SlowMock {
        async fn request(&self, method: &str, _params: Option<Value>) -> McpResult<Value> {
            if method == "initialize" {
                return Ok(json!({ "ok": true }));
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
            Ok(json!({}))
        }

        async fn notify(&self, method: &str, _params: Option<Value>) -> McpResult<()> {
            self.notified.lock().unwrap().push(method.to_string());
            Ok(())
        }
    }

    #[tokio::test]
    async fn timeout_returns_error_and_sends_cancel() {
        let notified = Arc::new(Mutex::new(Vec::new()));
        let client = McpClient::connect(
            SlowMock {
                notified: notified.clone(),
            },
            Duration::from_millis(20),
        )
        .await
        .unwrap();

        let err = client.request("slow/method", None).await.unwrap_err();
        assert!(matches!(err, McpError::Timeout(_)), "got {err:?}");
        assert!(notified
            .lock()
            .unwrap()
            .iter()
            .any(|m| m == "notifications/cancelled"));
    }
}
