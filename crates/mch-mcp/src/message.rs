//! JSON-RPC 2.0 wire types and the MCP model types we actually use.
//!
//! Forged from Goose `crates/goose/src/agents/mcp_client.rs` (which leans on the
//! `rmcp` crate's model). We implement the wire shape ourselves so the seams stay
//! visible.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The `_meta` key carrying the owning extension on every aggregated tool.
pub const TOOL_EXTENSION_META_KEY: &str = "mch_extension";

pub const JSONRPC_VERSION: &str = "2.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<JsonRpcError>,
}

/// An MCP tool as advertised by a server, before aggregation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tool {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListToolsResult {
    #[serde(default)]
    pub tools: Vec<Tool>,
    #[serde(default, rename = "nextCursor")]
    pub next_cursor: Option<String>,
}

/// The result of a tool call. `is_error` is the model-visible failure flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolResult {
    #[serde(default)]
    pub content: Vec<Value>,
    #[serde(default, rename = "isError")]
    pub is_error: bool,
}

impl CallToolResult {
    /// A failure surfaced to the model as data, so it can recover next turn.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![serde_json::json!({ "type": "text", "text": message.into() })],
            is_error: true,
        }
    }

    pub fn text(message: impl Into<String>) -> Self {
        Self {
            content: vec![serde_json::json!({ "type": "text", "text": message.into() })],
            is_error: false,
        }
    }

    pub fn is_error_result(&self) -> bool {
        self.is_error
    }
}
