//! mch-transport — Model transport with WebSocket and HTTP fallback.
//!
//! Mined from OpenCode's SessionModelTransport: WebSocket streaming with
/// HTTP fallback, connection pooling, rotation, and stream failure tracking.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::{Stream, StreamExt, sink::SinkExt};
use http::{HeaderMap, HeaderValue, HeaderName};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{Mutex, Semaphore};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMessage};
use tracing::{info, warn};
use uuid::Uuid;

/// Transport error.
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Fallback exhausted")]
    FallbackExhausted,
    #[error("Stream failed: {0}")]
    StreamFailed(String),
}

/// Configuration for the transport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    pub ws_url: String,
    pub http_url: String,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_stream_failures: u32,
    pub rotate_after_ms: u64,
    pub headers: HashMap<String, String>,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            ws_url: "ws://localhost:8080/ws".into(),
            http_url: "http://localhost:8080/api".into(),
            connect_timeout: Duration::from_secs(15),
            idle_timeout: Duration::from_secs(30 * 60), // 30 minutes
            max_stream_failures: 5,
            rotate_after_ms: 55 * 60 * 1000, // 55 minutes
            headers: HashMap::new(),
        }
    }
}

/// Unique identifier for a tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolCallId(pub uuid::Uuid);

impl ToolCallId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for ToolCallId {
    fn default() -> Self {
        Self::new()
    }
}

/// Tool call from the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: ToolCallId,
    pub name: String,
    pub input: serde_json::Value,
}

/// Result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id: ToolCallId,
    pub output: serde_json::Value,
    pub metadata: Option<serde_json::Value>,
    pub is_error: bool,
}

/// Model streaming event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelEvent {
    TextDelta { text: String },
    ToolCall { call: ToolCall },
    ToolResult { result: ToolResult },
    Finish { finish_reason: FinishReason, usage: Option<Usage> },
    Error { error: ModelError },
}

/// Why the model finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    Error,
    Unknown,
}

/// Token usage from the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
}

/// Model error from the provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelError {
    pub message: String,
    pub classification: ErrorClassification,
    pub recovery: Option<RecoveryStrategy>,
}

/// Classification of model errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorClassification {
    ContextOverflow,
    PayloadTooLarge,
    InvalidRequest,
    Transport,
    InvalidProviderOutput,
    ContentFilter,
    Unknown,
}

/// How to recover from an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    RetryFull,
    RotateAndRetryFull,
    RetryPartial,
    None,
}

/// Tool call definition for the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// A message in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<ToolCallId>,
}

/// ChatMessage role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// Model request sent to the transport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    pub system_prompt: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    pub tool_choice: ToolChoice,
    pub max_steps: Option<u32>,
    pub current_step: u32,
}

/// Tool choice configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolChoice {
    Auto,
    None,
    Required,
}

/// A streaming response from the model.
pub type ModelStream = Pin<Box<dyn Stream<Item = Result<ModelEvent, TransportError>> + Send>>;

/// Transport trait for model communication.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Stream a request to the model.
    async fn stream(&self, request: ModelRequest) -> Result<ModelStream, TransportError>;

    /// Close the transport for a session.
    async fn close(&self, session_id: &str) -> Result<(), TransportError>;

    /// Close all connections.
    async fn close_all(&self) -> Result<(), TransportError>;
}

/// WebSocket transport with HTTP fallback.
pub struct WebSocketTransport {
    config: TransportConfig,
    http_client: Client,
    connections: Arc<Mutex<HashMap<String, ConnectionState>>>,
    stream_failures: Arc<Mutex<HashMap<String, u32>>>,
    semaphore: Arc<Semaphore>,
}

/// State of a WebSocket connection.
#[derive(Debug, Clone)]
struct ConnectionState {
    url: String,
    headers: HeaderMap,
    opened_at: Instant,
    closing: bool,
    http_fallback: bool,
    stream_failures: u32,
}

impl WebSocketTransport {
    /// Create a new WebSocket transport.
    pub fn new(config: TransportConfig) -> Self {
        let http_client = Client::builder()
            .timeout(config.connect_timeout)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config,
            http_client,
            connections: Arc::new(Mutex::new(HashMap::new())),
            stream_failures: Arc::new(Mutex::new(HashMap::new())),
            semaphore: Arc::new(Semaphore::new(10)), // Max 10 concurrent connections
        }
    }

    /// Get or create a connection for a session.
    async fn get_connection(&self, session_id: &str, exchange: &Exchange) -> Result<ConnectionState, TransportError> {
        let mut connections = self.connections.lock().await;
        let _failures = self.stream_failures.lock().await;

        let entry = connections.entry(session_id.to_string()).or_insert_with(|| ConnectionState {
            url: exchange.connect.url.clone(),
            headers: build_headers(&exchange.connect.headers),
            opened_at: Instant::now(),
            closing: false,
            http_fallback: false,
            stream_failures: 0,
        });

        // Check if we need to rotate
        let rotate_after = Duration::from_millis(self.config.rotate_after_ms);
        let should_rotate = entry.url != exchange.connect.url
            || entry.opened_at.elapsed() >= rotate_after;

        if should_rotate && !entry.closing {
            info!("Rotating WebSocket connection for session {}", session_id);
            entry.closing = true;
            // Would close old connection here
            *entry = ConnectionState {
                url: exchange.connect.url.clone(),
                headers: build_headers(&exchange.connect.headers),
                opened_at: Instant::now(),
                closing: false,
                http_fallback: entry.http_fallback,
                stream_failures: 0,
            };
        }

        // Check stream failures
        if entry.stream_failures >= self.config.max_stream_failures {
            warn!("Session {} exceeded max stream failures; forcing HTTP fallback", session_id);
            entry.http_fallback = true;
        }

        Ok(entry.clone())
    }

    /// Stream via WebSocket.
    async fn stream_ws(&self, session_id: &str, request: ModelRequest) -> Result<ModelStream, TransportError> {
        let exchange = self.build_exchange(request)?;
        let connection = self.get_connection(session_id, &exchange).await?;

        if connection.http_fallback {
            return Err(TransportError::FallbackExhausted);
        }

        // Connect to WebSocket
        let (ws_stream, _) = connect_async(&exchange.connect.url).await
            .map_err(|e| TransportError::ConnectionFailed(e.to_string()))?;

        let (mut write, mut read) = ws_stream.split();

        // Send the request
        let message = serde_json::to_string(&exchange.request).map_err(|e| TransportError::Protocol(e.to_string()))?;
        write.send(WsMessage::Text(message.into())).await.map_err(TransportError::WebSocket)?;

        // Create stream from WebSocket read
        let stream = read.filter_map(|msg| async move {
            match msg {
                Ok(WsMessage::Text(text)) => {
                    match serde_json::from_str::<ModelEvent>(&text) {
                        Ok(event) => Some(Ok(event)),
                        Err(e) => Some(Err(TransportError::Protocol(e.to_string()))),
                    }
                }
                Ok(WsMessage::Close(_)) => None,
                Err(e) => Some(Err(TransportError::WebSocket(e))),
                _ => None,
            }
        }).boxed();

        Ok(stream)
    }

    /// Fallback to HTTP streaming.
    async fn stream_http(&self, request: ModelRequest) -> Result<ModelStream, TransportError> {
        let url = format!("{}/stream", self.config.http_url.trim_end_matches('/'));

        let response = self.http_client
            .post(&url)
            .json(&request)
            .timeout(self.config.idle_timeout)
            .send()
            .await
            .map_err(|e| TransportError::Http(e.to_string()))?;

        let stream = response
            .bytes_stream()
            .filter_map(|chunk| async move {
                chunk.ok().and_then(|bytes| {
                    String::from_utf8(bytes.to_vec()).ok().and_then(|text| {
                        // Parse SSE or JSON lines
                        text.lines()
                            .filter(|line| !line.is_empty())
                            .filter_map(|line| {
                                if let Some(data) = line.strip_prefix("data: ") {
                                    if data == "[DONE]" {
                                        return None;
                                    }
                                    serde_json::from_str::<ModelEvent>(data).ok()
                                } else {
                                    serde_json::from_str::<ModelEvent>(line).ok()
                                }
                            })
                            .next()
                            .map(Ok)
                    })
                })
            })
            .boxed();

        Ok(stream)
    }

    fn build_exchange(&self, request: ModelRequest) -> Result<Exchange, TransportError> {
        let mut headers = HeaderMap::new();
        for (k, v) in &self.config.headers {
            headers.insert(k.parse::<HeaderName>().unwrap(), v.parse::<HeaderValue>().unwrap());
        }
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));

        Ok(Exchange {
            connect: ConnectInfo {
                url: self.config.ws_url.clone(),
                headers: self.config.headers.clone(),
                rotate_after_ms: Some(self.config.rotate_after_ms),
            },
            request,
        })
    }
}

#[async_trait]
impl Transport for WebSocketTransport {
    async fn stream(&self, request: ModelRequest) -> Result<ModelStream, TransportError> {
        let _permit = self.semaphore.acquire().await.map_err(|e| TransportError::ConnectionFailed(e.to_string()))?;

        // Try WebSocket first
        match self.stream_ws("default", request.clone()).await {
            Ok(stream) => Ok(stream),
            Err(e) => {
                warn!("WebSocket stream failed, falling back to HTTP: {}", e);
                self.stream_http(request).await
            }
        }
    }

    async fn close(&self, session_id: &str) -> Result<(), TransportError> {
        let mut connections = self.connections.lock().await;
        if let Some(conn) = connections.get_mut(session_id) {
            conn.closing = true;
        }
        Ok(())
    }

    async fn close_all(&self) -> Result<(), TransportError> {
        let mut connections = self.connections.lock().await;
        for conn in connections.values_mut() {
            conn.closing = true;
        }
        connections.clear();
        Ok(())
    }
}

/// Exchange information for a single request/response.
#[derive(Debug, Clone)]
struct Exchange {
    connect: ConnectInfo,
    request: ModelRequest,
}

/// Connection info.
#[derive(Debug, Clone)]
struct ConnectInfo {
    url: String,
    headers: HashMap<String, String>,
    rotate_after_ms: Option<u64>,
}

/// Build HTTP headers from a map.
fn build_headers(headers: &HashMap<String, String>) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (k, v) in headers {
        if let (Ok(name), Ok(value)) = (k.parse::<HeaderName>(), v.parse::<HeaderValue>()) {
            map.insert(name, value);
        }
    }
    map
}

/// Factory for creating transports.
pub struct TransportFactory;

impl TransportFactory {
    /// Create a transport from config.
    pub fn create(config: TransportConfig) -> Arc<dyn Transport> {
        Arc::new(WebSocketTransport::new(config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_config_default() {
        let config = TransportConfig::default();
        assert_eq!(config.max_stream_failures, 5);
        assert_eq!(config.rotate_after_ms, 55 * 60 * 1000);
    }

    #[test]
    fn test_build_headers() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".into(), "Bearer token".into());
        let map = build_headers(&headers);
        assert!(map.contains_key("authorization"));
    }
}