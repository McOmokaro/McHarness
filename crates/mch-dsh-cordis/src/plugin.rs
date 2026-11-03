//! Plugin shapes and configuration.

use std::collections::HashMap;
use std::sync::Arc;

/// Inject declaration: service name -> optional intercept config.
pub type Inject = HashMap<String, Option<serde_json::Value>>;

/// Normalize an optional inject declaration into a plain map.
pub fn resolve_inject(inject: Option<Inject>) -> Inject {
    inject.unwrap_or_default()
}

/// A plugin entrypoint: a factory called with (context, config).
pub enum Plugin {
    Function(Box<dyn Fn(Arc<crate::context::Context>, Option<serde_json::Value>) + Send + Sync>),
    Constructor(Box<dyn Fn(Arc<crate::context::Context>, Option<serde_json::Value>) + Send + Sync>),
    Object(Box<dyn Fn(Arc<crate::context::Context>, Option<serde_json::Value>) + Send + Sync>),
}

impl std::fmt::Debug for Plugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Plugin").field(&self.name()).finish()
    }
}

impl Plugin {
    /// A short, stable name for the plugin shape.
    pub fn name(&self) -> &'static str {
        match self {
            Plugin::Function(_) => "Function",
            Plugin::Constructor(_) => "Constructor",
            Plugin::Object(_) => "Object",
        }
    }

    /// The registry key for this plugin.
    pub fn key(&self) -> String { self.name().to_string() }

    /// The inject declaration (currently always empty).
    pub fn inject(&self) -> Inject { Inject::new() }

    /// Invoke the plugin factory.
    pub fn invoke(&self, ctx: Arc<crate::context::Context>, config: Option<serde_json::Value>) {
        match self {
            Plugin::Function(f) | Plugin::Constructor(f) | Plugin::Object(f) => f(ctx, config),
        }
    }
}

/// A registered plugin runtime record.
#[derive(Debug, Clone)]
pub struct PluginRuntime {
    pub name: String,
    pub callback: String,
}

/// Static metadata describing a plugin.
#[derive(Debug, Clone, Default)]
pub struct PluginConfig {
    pub name: Option<String>,
    pub inject: Option<Inject>,
    pub provide: Option<Vec<String>>,
    pub intercept: Option<HashMap<String, bool>>,
    pub schema: Option<serde_json::Value>,
}

/// Backward-compatible alias.
pub type PluginShape = Plugin;
