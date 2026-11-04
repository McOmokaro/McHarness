//! MCP Client implementation

use crate::protocol::{
    InitializeParams, InitializeResult, McpMessage, McpRequest, McpResponse,
    McpTool, McpToolCall, McpToolResult, ClientCapabilities, ClientInfo,
    McpResource, McpResourceRead, McpPrompt, McpPromptGet, McpNotification,
};
use crate::protocol::McpError as ProtocolError;
use crate::transport::{McpTransport, McpTransportError, StdioTransport, WebSocketTransport};
use futures::stream::StreamExt;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, error, info, warn};

/// MCP Client configuration
#[derive(Debug, Clone)]
pub struct McpClientConfig {
    pub client_name: String,
    pub client_version: String,
    pub protocol_version: String,
    pub request_timeout_ms: u64,
}

impl Default for McpClientConfig {
    fn default() -> Self {
        Self {
            client_name: "mch-rea".to_string(),
            client_version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: "2024-11-05".to_string(),
            request_timeout_ms: 30000,
        }
    }
}

/// MCP Client errors
#[derive(Debug, thiserror::Error)]
pub enum McpClientError {
    #[error("Transport error: {0}")]
    Transport(#[from] McpTransportError),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Request timeout")]
    Timeout,

    #[error("Not initialized")]
    NotInitialized,

    #[error("Server error: {0}")]
    Server(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Protocol error: {0}")]
    ProtocolError(#[from] ProtocolError),
}

/// Pending request tracker
struct PendingRequest {
    tx: tokio::sync::oneshot::Sender<Result<McpResponse, McpClientError>>,
}

/// MCP Client
pub struct McpClient {
    config: McpClientConfig,
    transport: Arc<Mutex<Option<Box<dyn McpTransport>>>>,
    pending_requests: Arc<Mutex<HashMap<Value, PendingRequest>>>,
    next_id: Arc<Mutex<u64>>,
    initialized: Arc<Mutex<bool>>,
    server_capabilities: Arc<Mutex<Option<InitializeResult>>>,
    notification_tx: mpsc::UnboundedSender<McpNotification>,
    notification_rx: Arc<Mutex<mpsc::UnboundedReceiver<McpNotification>>>,
}

impl McpClient {
    /// Create a new MCP client
    pub fn new(config: McpClientConfig) -> Self {
        let (notification_tx, notification_rx) = mpsc::unbounded_channel();
        Self {
            config,
            transport: Arc::new(Mutex::new(None)),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(1)),
            initialized: Arc::new(Mutex::new(false)),
            server_capabilities: Arc::new(Mutex::new(None)),
            notification_tx,
            notification_rx: Arc::new(Mutex::new(notification_rx)),
        }
    }

    /// Connect via stdio to a local command
    pub async fn connect_stdio(&mut self, command: &str, args: &[&str]) -> Result<(), McpClientError> {
        let transport = StdioTransport::spawn(command, args).await?;
        *self.transport.lock().await = Some(Box::new(transport));
        self.start_message_loop().await;
        self.initialize().await
    }

    /// Connect via WebSocket
    pub async fn connect_websocket(&mut self, url: &str) -> Result<(), McpClientError> {
        let transport = WebSocketTransport::connect(url).await?;
        *self.transport.lock().await = Some(Box::new(transport));
        self.start_message_loop().await;
        self.initialize().await
    }

    /// Start the background message loop
    async fn start_message_loop(&mut self) {
        let transport = {
            let mut transport_guard = self.transport.lock().await;
            transport_guard.take().expect("Transport must be set")
        };
        let pending_requests = self.pending_requests.clone();
        let initialized = self.initialized.clone();
        let server_capabilities = self.server_capabilities.clone();
        let notification_tx = self.notification_tx.clone();

        tokio::spawn(async move {
            let mut transport = transport;
            loop {
                match transport.receive().await {
                    Ok(Some(message)) => {
                        match message {
                            McpMessage::Response(response) => {
                                let mut pending = pending_requests.lock().await;
                                if let Some(pending_req) = pending.remove(&response.id) {
                                    let _ = pending_req.tx.send(Ok(response));
                                } else {
                                    warn!("Received response for unknown request id: {:?}", response.id);
                                }
                            }
                            McpMessage::Notification(notification) => {
                                let _ = notification_tx.send(notification);
                            }
                            McpMessage::Request(request) => {
                                // Handle incoming requests (e.g., sampling, roots)
                                debug!("Received incoming request: {}", request.method);
                                // For now, just respond with method not found
                                let error = ProtocolError::new(-32601, "Method not found".to_string(), None);
                                let response = McpResponse::error(request.id, error);
                                let _ = transport.send(McpMessage::Response(response)).await;
                            }
                        }
                    }
                    Ok(None) => {
                        info!("Transport closed");
                        break;
                    }
                    Err(e) => {
                        error!("Transport error: {}", e);
                        break;
                    }
                }
            }
            // Mark as not initialized
            *initialized.lock().await = false;
        });
    }

    /// Initialize the MCP connection
    async fn initialize(&mut self) -> Result<(), McpClientError> {
        let params = InitializeParams {
            protocol_version: self.config.protocol_version.clone(),
            capabilities: ClientCapabilities::default(),
            client_info: ClientInfo {
                name: self.config.client_name.clone(),
                version: self.config.client_version.clone(),
            },
        };

        let result: InitializeResult = self.request("initialize", Some(serde_json::to_value(params)?)).await?;
        *self.server_capabilities.lock().await = Some(result.clone());
        *self.initialized.lock().await = true;

        // Send initialized notification
        self.notify("notifications/initialized", None).await?;

        info!("MCP client initialized: {} v{}", result.server_info.name, result.server_info.version);
        Ok(())
    }

    /// Send a request and wait for response
    async fn request<T: serde::de::DeserializeOwned>(&self, method: &str, params: Option<Value>) -> Result<T, McpClientError> {
        let id = {
            let mut next_id = self.next_id.lock().await;
            let id = Value::Number((*next_id).into());
            *next_id += 1;
            id
        };

        let request = McpRequest::new(id.clone(), method.to_string(), params);
        let (tx, rx) = tokio::sync::oneshot::channel();

        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(id.clone(), PendingRequest { tx });
        }

        {
            let mut transport_guard = self.transport.lock().await;
            let transport = transport_guard.as_mut().expect("Transport not set");
            transport.send(McpMessage::Request(request)).await?;
        }

        // Wait for response with timeout
        let response = tokio::time::timeout(
            std::time::Duration::from_millis(self.config.request_timeout_ms),
            rx,
        )
        .await
        .map_err(|_| McpClientError::Timeout)?
        .map_err(|_| McpClientError::Protocol("Request cancelled".to_string()))?;

        match response {
            Ok(resp) => {
                if let Some(error) = resp.error {
                    Err(McpClientError::Server(format!("{}: {}", error.code, error.message)))
                } else {
                    let result = resp.result.ok_or_else(|| McpClientError::Protocol("Empty response".to_string()))?;
                    Ok(serde_json::from_value(result)?)
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Send a notification
    async fn notify(&self, method: &str, params: Option<Value>) -> Result<(), McpClientError> {
        let notification = McpNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };
        {
            let mut transport_guard = self.transport.lock().await;
            let transport = transport_guard.as_mut().expect("Transport not set");
            transport.send(McpMessage::Notification(notification)).await?;
        }
        Ok(())
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        // This is a sync check - in practice you'd use a watch channel
        true // Simplified
    }

    /// Get server capabilities
    pub async fn server_capabilities(&self) -> Option<InitializeResult> {
        self.server_capabilities.lock().await.clone()
    }

    /// List available tools
    pub async fn list_tools(&self) -> Result<Vec<McpTool>, McpClientError> {
        #[derive(Deserialize)]
        struct ListToolsResult {
            tools: Vec<McpTool>,
        }
        let result: ListToolsResult = self.request("tools/list", None).await?;
        Ok(result.tools)
    }

    /// Call a tool
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<McpToolResult, McpClientError> {
        let params = McpToolCall {
            name: name.to_string(),
            arguments,
        };
        self.request("tools/call", Some(serde_json::to_value(params)?)).await
    }

    /// List resources
    pub async fn list_resources(&self) -> Result<Vec<McpResource>, McpClientError> {
        #[derive(Deserialize)]
        struct ListResourcesResult {
            resources: Vec<McpResource>,
        }
        let result: ListResourcesResult = self.request("resources/list", None).await?;
        Ok(result.resources)
    }

    /// Read a resource
    pub async fn read_resource(&self, uri: &str) -> Result<McpResourceRead, McpClientError> {
        let params = serde_json::json!({ "uri": uri });
        self.request("resources/read", Some(params)).await
    }

    /// List prompts
    pub async fn list_prompts(&self) -> Result<Vec<McpPrompt>, McpClientError> {
        #[derive(Deserialize)]
        struct ListPromptsResult {
            prompts: Vec<McpPrompt>,
        }
        let result: ListPromptsResult = self.request("prompts/list", None).await?;
        Ok(result.prompts)
    }

    /// Get a prompt
    pub async fn get_prompt(&self, name: &str, arguments: Option<Value>) -> Result<McpPromptGet, McpClientError> {
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments.unwrap_or(serde_json::json!({}))
        });
        self.request("prompts/get", Some(params)).await
    }

    /// Subscribe to notifications
    pub fn notifications(&self) -> Arc<Mutex<mpsc::UnboundedReceiver<McpNotification>>> {
        self.notification_rx.clone()
    }

    /// Close the connection
    pub async fn close(&mut self) -> Result<(), McpClientError> {
        let transport = {
            let mut transport_guard = self.transport.lock().await;
            transport_guard.take()
        };
        if let Some(mut transport) = transport {
            transport.close().await?;
        }
        *self.initialized.lock().await = false;
        Ok(())
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // Best effort cleanup
        if let Ok(mut transport_guard) = self.transport.try_lock() {
            if let Some(mut transport) = transport_guard.take() {
                let _ = transport.close();
            }
        }
    }
}