//! mch-rea — Native Rust MCP client for reverse engineering
//!
//! Forged from morluto/rea's MCP integration patterns. This crate provides
//! a native Rust implementation of the Model Context Protocol client for
//! connecting to reverse engineering tools (Ghidra, Hopper, IDA, etc.)
//! via the REA MCP server.

pub mod client;
pub mod protocol;
pub mod transport;
pub mod tools;

pub use client::{McpClient, McpClientConfig, McpClientError};
pub use protocol::{McpMessage, McpRequest, McpResponse, McpNotification, McpError, McpTool, McpToolCall, McpToolResult};
pub use transport::{McpTransport, StdioTransport, WebSocketTransport, McpTransportError};
pub use tools::{ReaClient, tool_names};