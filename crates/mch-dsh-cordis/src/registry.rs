//! Plugin registry — normalizes plugin shapes, tracks runtimes, starts fibers.

use std::collections::HashMap;
use std::sync::Arc;

use crate::context::Context;
use crate::fiber::Fiber;
use crate::plugin::{Plugin, PluginRuntime};

/// Inner registry state (guarded by the owning `Context`'s mutex).
#[derive(Debug, Default)]
struct RegistryInner {
    counter: u64,
    runtimes: HashMap<String, Arc<PluginRuntime>>,
}

/// The plugin registry.
#[derive(Debug, Default)]
pub struct Registry {
    inner: RegistryInner,
}

impl Registry {
    pub fn new() -> Self { Self::default() }

    /// Allocate the next fiber uid.
    pub fn next_uid(&mut self) -> u64 { self.inner.counter += 1; self.inner.counter }

    /// Number of registered plugin runtimes.
    pub fn size(&self) -> usize { self.inner.runtimes.len() }

    /// Look up a runtime by key.
    pub fn get(&self, key: &str) -> Option<Arc<PluginRuntime>> { self.inner.runtimes.get(key).cloned() }

    /// Whether a runtime is registered.
    pub fn has(&self, key: &str) -> bool { self.inner.runtimes.contains_key(key) }

    /// Remove a runtime.
    pub fn delete(&mut self, key: &str) -> Option<Arc<PluginRuntime>> { self.inner.runtimes.remove(key) }

    /// All registered runtimes.
    pub fn values(&self) -> Vec<Arc<PluginRuntime>> { self.inner.runtimes.values().cloned().collect() }

    /// Register a plugin and start its fiber.
    pub fn plugin(&mut self, ctx: Arc<Context>, plugin: Plugin, config: Option<serde_json::Value>) -> Arc<Fiber> {
        let runtime = Arc::new(PluginRuntime { name: plugin.key(), callback: plugin.key() });
        self.inner.runtimes.insert(runtime.name.clone(), runtime.clone());
        let inject = plugin.inject();
        let fiber = Arc::new(Fiber::new(ctx, config, inject, Some(runtime)));
        plugin.invoke(fiber.ctx.clone(), fiber.config.clone());
        fiber
    }
}
