//! mch-dsh-llm — provider-neutral LLM service with BYOK, forged from DeepSeek Harness.
//!
//! The provider-neutral model-call vocabulary (`ctx.llm`), a streaming adapter
//! trait, an adapter registry, and Bring-Your-Own-Key credential providers.

use std::collections::HashMap;
use std::sync::Arc;

/// One message in a model conversation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// A tool call requested by the model.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// The result of a tool call.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolResult {
    pub id: String,
    pub output: serde_json::Value,
    pub is_error: bool,
}

/// A provider-neutral model request.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelRequest {
    pub provider: String,
    pub model: String,
    pub messages: Vec<Message>,
}

/// A provider-neutral model response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelResponse {
    pub content: String,
}

/// One chunk of a streamed response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum StreamChunk {
    Text(String),
    ToolCall(ToolCall),
    Finish,
    Error(String),
}

/// Backward-compatible alias for a stream chunk.
pub type StreamEvent = StreamChunk;

/// A provider-neutral streaming model call.
#[async_trait::async_trait]
pub trait ModelStream: Send + Sync {
    async fn request(&self, req: ModelRequest) -> Result<Vec<StreamChunk>, String>;
}

/// A concrete provider adapter.
pub struct ProviderAdapter {
    pub name: String,
    pub stream: Arc<dyn ModelStream>,
}

impl std::fmt::Debug for ProviderAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderAdapter").field("name", &self.name).finish()
    }
}

/// A registry of provider adapters keyed by provider name.
#[derive(Default)]
pub struct ProviderRegistry {
    adapters: HashMap<String, Arc<dyn ModelStream>>,
}

impl ProviderRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, name: impl Into<String>, stream: Arc<dyn ModelStream>) {
        self.adapters.insert(name.into(), stream);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn ModelStream>> {
        self.adapters.get(name).cloned()
    }

    pub fn names(&self) -> Vec<String> { self.adapters.keys().cloned().collect() }
}

/// Bring-Your-Own-Key credential source.
pub trait CredentialProvider: Send + Sync {
    fn credential(&self) -> Option<String>;
}

/// API-key credential.
#[derive(Default)]
pub struct ApiKeyProvider {
    pub key: Option<String>,
}

impl CredentialProvider for ApiKeyProvider {
    fn credential(&self) -> Option<String> { self.key.clone() }
}

/// Account-token credential.
#[derive(Default)]
pub struct AccountTokenProvider {
    pub token: Option<String>,
}

impl CredentialProvider for AccountTokenProvider {
    fn credential(&self) -> Option<String> { self.token.clone() }
}

/// Re-export commonly used types.
pub mod prelude {
    pub use crate::{
        ApiKeyProvider, AccountTokenProvider, CredentialProvider, Message, ModelRequest,
        ModelResponse, ModelStream, ProviderAdapter, StreamChunk, ToolCall, ToolResult,
    };
}
