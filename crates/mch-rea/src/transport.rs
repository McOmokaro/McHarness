//! MCP Transport layer

use crate::protocol::{McpMessage, McpRequest, McpResponse, McpNotification};
use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message, WebSocketStream, MaybeTlsStream};
use tokio::net::TcpStream;

/// Transport trait for MCP communication
#[async_trait]
pub trait McpTransport: Send + Sync {
    /// Send a message
    async fn send(&mut self, message: McpMessage) -> Result<(), McpTransportError>;

    /// Receive a message
    async fn receive(&mut self) -> Result<Option<McpMessage>, McpTransportError>;

    /// Close the transport
    async fn close(&mut self) -> Result<(), McpTransportError>;
}

/// Transport errors
#[derive(Debug, thiserror::Error)]
pub enum McpTransportError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Protocol error: {0}")]
    Protocol(String),
}

/// Stdio transport for local MCP servers
pub struct StdioTransport {
    reader: tokio::io::BufReader<tokio::process::ChildStdout>,
    writer: tokio::process::ChildStdin,
}

impl StdioTransport {
    /// Create a new stdio transport by spawning a child process
    pub async fn spawn(command: &str, args: &[&str]) -> Result<Self, McpTransportError> {
        use tokio::process::Command;

        let mut child = Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;

        let stdin = child.stdin.take().ok_or_else(|| McpTransportError::Protocol("Failed to get stdin".to_string()))?;
        let stdout = child.stdout.take().ok_or_else(|| McpTransportError::Protocol("Failed to get stdout".to_string()))?;

        Ok(Self {
            reader: tokio::io::BufReader::new(stdout),
            writer: stdin,
        })
    }

    /// Create from existing stdin/stdout (for testing)
    pub fn new(reader: impl AsyncRead + Send + Unpin + 'static, writer: impl AsyncWrite + Send + Unpin + 'static) -> Self {
        // This is a simplified version - in practice you'd use tokio::io::split
        todo!("Use tokio::io::split for generic AsyncRead/AsyncWrite")
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn send(&mut self, message: McpMessage) -> Result<(), McpTransportError> {
        let json = serde_json::to_string(&message)?;
        use tokio::io::AsyncWriteExt;
        self.writer.write_all(json.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn receive(&mut self) -> Result<Option<McpMessage>, McpTransportError> {
        use tokio::io::AsyncBufReadExt;
        let mut line = String::new();
        let bytes_read = self.reader.read_line(&mut line).await?;

        if bytes_read == 0 {
            return Ok(None);
        }

        let message: McpMessage = serde_json::from_str(line.trim())?;
        Ok(Some(message))
    }

    async fn close(&mut self) -> Result<(), McpTransportError> {
        use tokio::io::AsyncWriteExt;
        self.writer.shutdown().await?;
        Ok(())
    }
}

/// WebSocket transport for remote MCP servers
pub struct WebSocketTransport {
    ws_stream: Pin<Box<WebSocketStream<MaybeTlsStream<TcpStream>>>>,
}

impl WebSocketTransport {
    /// Connect to a WebSocket MCP server
    pub async fn connect(url: &str) -> Result<Self, McpTransportError> {
        let (ws_stream, _) = connect_async(url).await?;
        Ok(Self {
            ws_stream: Box::pin(ws_stream),
        })
    }
}

#[async_trait]
impl McpTransport for WebSocketTransport {
    async fn send(&mut self, message: McpMessage) -> Result<(), McpTransportError> {
        let json = serde_json::to_string(&message)?;
        self.ws_stream.send(Message::Text(json)).await?;
        Ok(())
    }

    async fn receive(&mut self) -> Result<Option<McpMessage>, McpTransportError> {
        while let Some(msg) = self.ws_stream.next().await {
            let msg = msg?;
            match msg {
                Message::Text(text) => {
                    let message: McpMessage = serde_json::from_str(&text)?;
                    return Ok(Some(message));
                }
                Message::Close(_) => return Ok(None),
                _ => continue,
            }
        }
        Ok(None)
    }

    async fn close(&mut self) -> Result<(), McpTransportError> {
        self.ws_stream.close().await?;
        Ok(())
    }
}