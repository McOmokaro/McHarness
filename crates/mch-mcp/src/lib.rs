//! # mch-mcp — the McHarness MCP core
//!
//! A native MCP client and multi-server tool aggregator, forged from Goose
//! (`block/goose`). No `rmcp` dependency: the wire protocol, the transports, and
//! the aggregation are ours, so the seams stay visible and the "Whole" law is not
//! someone else's abstraction.
//!
//! Three things this crate does that a bare MCP client does not:
//!
//! 1. **One flat tool list from many servers.** [`catalog::ToolCatalog`] merges
//!    every server's tools, namespaces them (`{owner}__{tool}`), dedups, and
//!    rewrites names so any provider accepts them.
//! 2. **Timeouts that tell the server to stop.** [`client::McpClient::request`]
//!    races the response against the clock and sends `notifications/cancelled`
//!    on the way out.
//! 3. **Errors as data.** A tool failure returns a [`message::CallToolResult`]
//!    with `is_error = true`, which the model can read and recover from; only a
//!    dead transport is a hard error.
//!
//! ```
//! use mch_mcp::catalog::{namespace_tool, ToolCatalog};
//! use mch_mcp::message::Tool;
//! use serde_json::json;
//!
//! let tool = Tool {
//!     name: "shell".into(),
//!     description: None,
//!     input_schema: json!({ "type": "object" }),
//!     meta: None,
//! };
//! let namespaced = namespace_tool("developer", tool, false);
//! let catalog = ToolCatalog::build(vec![("developer".into(), vec![namespaced])]);
//! assert_eq!(catalog.tools()[0].name, "developer__shell");
//! ```

pub mod catalog;
pub mod client;
pub mod content;
pub mod env;
pub mod error;
pub mod message;
pub mod transport;

pub use catalog::{namespace_tool, provider_safe_name, tool_owner, ToolCatalog};
pub use client::{McpClient, DEFAULT_TIMEOUT_SECS};
pub use error::{McpError, McpResult, ProcessExit};
pub use message::{CallToolResult, ListToolsResult, Tool};
pub use transport::{
    duplex_pair, spawn_stdio, DuplexTransport, HttpTransport, JsonRpcTransport, StdioTransport,
    Transport,
};
