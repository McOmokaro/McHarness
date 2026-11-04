//! REA-specific MCP tool definitions

use crate::protocol::{McpTool, McpToolCall, McpToolResult};
use serde_json::Value;

/// REA tool names (from the REA MCP server)
pub mod tool_names {
    pub const OPEN_BINARY: &str = "open_binary";
    pub const CLOSE_BINARY: &str = "close_binary";
    pub const DECOMPILE_FUNCTION: &str = "decompile_function";
    pub const GET_CALL_GRAPH: &str = "get_call_graph";
    pub const FIND_XREFS: &str = "find_xrefs_to_name";
    pub const SEARCH_STRINGS: &str = "search_strings";
    pub const LIST_NAMES: &str = "list_names";
    pub const SET_ADDRESS_NAME: &str = "set_address_name";
    pub const LIST_SEGMENTS: &str = "list_segments";
    pub const LIST_PROCEDURES: &str = "list_procedures";
    pub const ANALYZE_FUNCTION: &str = "analyze_function";
    pub const EXPORT_EVIDENCE_BUNDLE: &str = "export_evidence_bundle";
    pub const IMPORT_EVIDENCE_BUNDLE: &str = "import_evidence_bundle";
    pub const START_FRIDA_SESSION: &str = "start_frida_session";
    pub const LOAD_FRIDA_SCRIPT: &str = "load_frida_script";
    pub const CAPTURE_ADB_SCREEN: &str = "capture_adb_screen";
    pub const START_ADB_ACTIVITY: &str = "start_adb_activity";
    pub const READ_ADB_LOGCAT: &str = "read_adb_logcat";
}

/// Get all REA tool definitions
pub fn rea_tools() -> Vec<McpTool> {
    vec![
        McpTool {
            name: tool_names::OPEN_BINARY.to_string(),
            description: "Open a binary file for analysis (ELF, PE, Mach-O, DEX, etc.)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to the binary file" },
                    "architecture": { "type": "string", "description": "Optional architecture hint" }
                },
                "required": ["path"]
            }),
        },
        McpTool {
            name: tool_names::CLOSE_BINARY.to_string(),
            description: "Close a previously opened binary".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "handle": { "type": "string", "description": "Binary handle from open_binary" }
                },
                "required": ["handle"]
            }),
        },
        McpTool {
            name: tool_names::DECOMPILE_FUNCTION.to_string(),
            description: "Decompile a function at the given address".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "handle": { "type": "string", "description": "Binary handle" },
                    "address": { "type": "string", "description": "Function address (hex)" }
                },
                "required": ["handle", "address"]
            }),
        },
        McpTool {
            name: tool_names::GET_CALL_GRAPH.to_string(),
            description: "Get call graph for a function".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "handle": { "type": "string" },
                    "address": { "type": "string" },
                    "depth": { "type": "integer", "default": 3 }
                },
                "required": ["handle", "address"]
            }),
        },
        McpTool {
            name: tool_names::FIND_XREFS.to_string(),
            description: "Find cross-references to a symbol/address".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "handle": { "type": "string" },
                    "name": { "type": "string" }
                },
                "required": ["handle", "name"]
            }),
        },
        McpTool {
            name: tool_names::SEARCH_STRINGS.to_string(),
            description: "Search for strings in the binary".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "handle": { "type": "string" },
                    "pattern": { "type": "string" },
                    "min_length": { "type": "integer", "default": 4 }
                },
                "required": ["handle", "pattern"]
            }),
        },
        McpTool {
            name: tool_names::LIST_NAMES.to_string(),
            description: "List all named symbols/addresses".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "handle": { "type": "string" },
                    "filter": { "type": "string" }
                },
                "required": ["handle"]
            }),
        },
        McpTool {
            name: tool_names::SET_ADDRESS_NAME.to_string(),
            description: "Set a name for an address".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "handle": { "type": "string" },
                    "address": { "type": "string" },
                    "name": { "type": "string" }
                },
                "required": ["handle", "address", "name"]
            }),
        },
        McpTool {
            name: tool_names::LIST_SEGMENTS.to_string(),
            description: "List memory segments/sections".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "handle": { "type": "string" }
                },
                "required": ["handle"]
            }),
        },
        McpTool {
            name: tool_names::LIST_PROCEDURES.to_string(),
            description: "List all procedures/functions".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "handle": { "type": "string" },
                    "filter": { "type": "string" }
                },
                "required": ["handle"]
            }),
        },
        McpTool {
            name: tool_names::ANALYZE_FUNCTION.to_string(),
            description: "Run full analysis on a function".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "handle": { "type": "string" },
                    "address": { "type": "string" }
                },
                "required": ["handle", "address"]
            }),
        },
        McpTool {
            name: tool_names::EXPORT_EVIDENCE_BUNDLE.to_string(),
            description: "Export an evidence bundle (forensic snapshot)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "handle": { "type": "string" },
                    "path": { "type": "string" }
                },
                "required": ["handle", "path"]
            }),
        },
        McpTool {
            name: tool_names::IMPORT_EVIDENCE_BUNDLE.to_string(),
            description: "Import an evidence bundle".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }),
        },
        McpTool {
            name: tool_names::START_FRIDA_SESSION.to_string(),
            description: "Start a Frida session for dynamic analysis".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "Process name or PID" },
                    "device": { "type": "string", "description": "Optional device ID" }
                },
                "required": ["target"]
            }),
        },
        McpTool {
            name: tool_names::LOAD_FRIDA_SCRIPT.to_string(),
            description: "Load a Frida script into a session".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "script": { "type": "string" }
                },
                "required": ["session_id", "script"]
            }),
        },
        McpTool {
            name: tool_names::CAPTURE_ADB_SCREEN.to_string(),
            description: "Capture Android device screen via ADB".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "device": { "type": "string", "description": "Optional device serial" }
                }
            }),
        },
        McpTool {
            name: tool_names::START_ADB_ACTIVITY.to_string(),
            description: "Start an Android activity via ADB".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "package": { "type": "string" },
                    "activity": { "type": "string" },
                    "device": { "type": "string" }
                },
                "required": ["package", "activity"]
            }),
        },
        McpTool {
            name: tool_names::READ_ADB_LOGCAT.to_string(),
            description: "Read Android logcat".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "filter": { "type": "string" },
                    "device": { "type": "string" },
                    "lines": { "type": "integer", "default": 100 }
                }
            }),
        },
    ]
}

/// REA client extension with typed tool calls
pub struct ReaClient {
    client: crate::client::McpClient,
}

impl ReaClient {
    pub fn new(client: crate::client::McpClient) -> Self {
        Self { client }
    }

    pub async fn open_binary(&self, path: &str, architecture: Option<&str>) -> Result<Value, crate::client::McpClientError> {
        let mut args = serde_json::json!({ "path": path });
        if let Some(arch) = architecture {
            args["architecture"] = serde_json::json!(arch);
        }
        let result = self.client.call_tool(tool_names::OPEN_BINARY, args).await?;
        let content = result.content.unwrap_or_default();
        if let Some(first) = content.first() {
            Ok(serde_json::to_value(first)?)
        } else {
            Ok(Value::Null)
        }
    }

    pub async fn close_binary(&self, handle: &str) -> Result<Value, crate::client::McpClientError> {
        let result = self.client.call_tool(tool_names::CLOSE_BINARY, serde_json::json!({ "handle": handle })).await?;
        let content = result.content.unwrap_or_default();
        if let Some(first) = content.first() {
            Ok(serde_json::to_value(first)?)
        } else {
            Ok(Value::Null)
        }
    }

    pub async fn decompile_function(&self, handle: &str, address: &str) -> Result<Value, crate::client::McpClientError> {
        let result = self.client.call_tool(tool_names::DECOMPILE_FUNCTION, serde_json::json!({ "handle": handle, "address": address })).await?;
        let content = result.content.unwrap_or_default();
        if let Some(first) = content.first() {
            Ok(serde_json::to_value(first)?)
        } else {
            Ok(Value::Null)
        }
    }

    pub async fn get_call_graph(&self, handle: &str, address: &str, depth: Option<u32>) -> Result<Value, crate::client::McpClientError> {
        let mut args = serde_json::json!({ "handle": handle, "address": address });
        if let Some(d) = depth {
            args["depth"] = serde_json::json!(d);
        }
        let result = self.client.call_tool(tool_names::GET_CALL_GRAPH, args).await?;
        let content = result.content.unwrap_or_default();
        if let Some(first) = content.first() {
            Ok(serde_json::to_value(first)?)
        } else {
            Ok(Value::Null)
        }
    }

    pub async fn find_xrefs(&self, handle: &str, name: &str) -> Result<Value, crate::client::McpClientError> {
        let result = self.client.call_tool(tool_names::FIND_XREFS, serde_json::json!({ "handle": handle, "name": name })).await?;
        let content = result.content.unwrap_or_default();
        if let Some(first) = content.first() {
            Ok(serde_json::to_value(first)?)
        } else {
            Ok(Value::Null)
        }
    }

    pub async fn search_strings(&self, handle: &str, pattern: &str, min_length: Option<u32>) -> Result<Value, crate::client::McpClientError> {
        let mut args = serde_json::json!({ "handle": handle, "pattern": pattern });
        if let Some(l) = min_length {
            args["min_length"] = serde_json::json!(l);
        }
        let result = self.client.call_tool(tool_names::SEARCH_STRINGS, args).await?;
        let content = result.content.unwrap_or_default();
        if let Some(first) = content.first() {
            Ok(serde_json::to_value(first)?)
        } else {
            Ok(Value::Null)
        }
    }

    pub async fn list_names(&self, handle: &str, filter: Option<&str>) -> Result<Value, crate::client::McpClientError> {
        let mut args = serde_json::json!({ "handle": handle });
        if let Some(f) = filter {
            args["filter"] = serde_json::json!(f);
        }
        let result = self.client.call_tool(tool_names::LIST_NAMES, args).await?;
        let content = result.content.unwrap_or_default();
        if let Some(first) = content.first() {
            Ok(serde_json::to_value(first)?)
        } else {
            Ok(Value::Null)
        }
    }

    pub async fn set_address_name(&self, handle: &str, address: &str, name: &str) -> Result<Value, crate::client::McpClientError> {
        let result = self.client.call_tool(tool_names::SET_ADDRESS_NAME, serde_json::json!({ "handle": handle, "address": address, "name": name })).await?;
        let content = result.content.unwrap_or_default();
        if let Some(first) = content.first() {
            Ok(serde_json::to_value(first)?)
        } else {
            Ok(Value::Null)
        }
    }

    pub async fn list_segments(&self, handle: &str) -> Result<Value, crate::client::McpClientError> {
        let result = self.client.call_tool(tool_names::LIST_SEGMENTS, serde_json::json!({ "handle": handle })).await?;
        let content = result.content.unwrap_or_default();
        if let Some(first) = content.first() {
            Ok(serde_json::to_value(first)?)
        } else {
            Ok(Value::Null)
        }
    }

    pub async fn list_procedures(&self, handle: &str, filter: Option<&str>) -> Result<Value, crate::client::McpClientError> {
        let mut args = serde_json::json!({ "handle": handle });
        if let Some(f) = filter {
            args["filter"] = serde_json::json!(f);
        }
        let result = self.client.call_tool(tool_names::LIST_PROCEDURES, args).await?;
        let content = result.content.unwrap_or_default();
        if let Some(first) = content.first() {
            Ok(serde_json::to_value(first)?)
        } else {
            Ok(Value::Null)
        }
    }

    pub async fn analyze_function(&self, handle: &str, address: &str) -> Result<Value, crate::client::McpClientError> {
        let result = self.client.call_tool(tool_names::ANALYZE_FUNCTION, serde_json::json!({ "handle": handle, "address": address })).await?;
        let content = result.content.unwrap_or_default();
        if let Some(first) = content.first() {
            Ok(serde_json::to_value(first)?)
        } else {
            Ok(Value::Null)
        }
    }

    pub async fn export_evidence_bundle(&self, handle: &str, path: &str) -> Result<Value, crate::client::McpClientError> {
        let result = self.client.call_tool(tool_names::EXPORT_EVIDENCE_BUNDLE, serde_json::json!({ "handle": handle, "path": path })).await?;
        let content = result.content.unwrap_or_default();
        if let Some(first) = content.first() {
            Ok(serde_json::to_value(first)?)
        } else {
            Ok(Value::Null)
        }
    }

    pub async fn import_evidence_bundle(&self, path: &str) -> Result<Value, crate::client::McpClientError> {
        let result = self.client.call_tool(tool_names::IMPORT_EVIDENCE_BUNDLE, serde_json::json!({ "path": path })).await?;
        let content = result.content.unwrap_or_default();
        if let Some(first) = content.first() {
            Ok(serde_json::to_value(first)?)
        } else {
            Ok(Value::Null)
        }
    }

    pub async fn start_frida_session(&self, target: &str, device: Option<&str>) -> Result<Value, crate::client::McpClientError> {
        let mut args = serde_json::json!({ "target": target });
        if let Some(d) = device {
            args["device"] = serde_json::json!(d);
        }
        let result = self.client.call_tool(tool_names::START_FRIDA_SESSION, args).await?;
        let content = result.content.unwrap_or_default();
        if let Some(first) = content.first() {
            Ok(serde_json::to_value(first)?)
        } else {
            Ok(Value::Null)
        }
    }

    pub async fn load_frida_script(&self, session_id: &str, script: &str) -> Result<Value, crate::client::McpClientError> {
        let result = self.client.call_tool(tool_names::LOAD_FRIDA_SCRIPT, serde_json::json!({ "session_id": session_id, "script": script })).await?;
        let content = result.content.unwrap_or_default();
        if let Some(first) = content.first() {
            Ok(serde_json::to_value(first)?)
        } else {
            Ok(Value::Null)
        }
    }

    pub async fn capture_adb_screen(&self, device: Option<&str>) -> Result<Value, crate::client::McpClientError> {
        let mut args = serde_json::json!({});
        if let Some(d) = device {
            args["device"] = serde_json::json!(d);
        }
        let result = self.client.call_tool(tool_names::CAPTURE_ADB_SCREEN, args).await?;
        let content = result.content.unwrap_or_default();
        if let Some(first) = content.first() {
            Ok(serde_json::to_value(first)?)
        } else {
            Ok(Value::Null)
        }
    }

    pub async fn start_adb_activity(&self, package: &str, activity: &str, device: Option<&str>) -> Result<Value, crate::client::McpClientError> {
        let mut args = serde_json::json!({ "package": package, "activity": activity });
        if let Some(d) = device {
            args["device"] = serde_json::json!(d);
        }
        let result = self.client.call_tool(tool_names::START_ADB_ACTIVITY, args).await?;
        let content = result.content.unwrap_or_default();
        if let Some(first) = content.first() {
            Ok(serde_json::to_value(first)?)
        } else {
            Ok(Value::Null)
        }
    }

    pub async fn read_adb_logcat(&self, filter: Option<&str>, device: Option<&str>, lines: Option<u32>) -> Result<Value, crate::client::McpClientError> {
        let mut args = serde_json::json!({});
        if let Some(f) = filter { args["filter"] = serde_json::json!(f); }
        if let Some(d) = device { args["device"] = serde_json::json!(d); }
        if let Some(l) = lines { args["lines"] = serde_json::json!(l); }
        let result = self.client.call_tool(tool_names::READ_ADB_LOGCAT, args).await?;
        let content = result.content.unwrap_or_default();
        if let Some(first) = content.first() {
            Ok(serde_json::to_value(first)?)
        } else {
            Ok(Value::Null)
        }
    }
}