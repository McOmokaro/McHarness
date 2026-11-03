//! Context — the scoped dependency container.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::plugin::Inject;
use crate::registry::Registry;

/// Intercept configuration for a service.
#[derive(Debug, Clone, Default)]
pub struct InterceptConfig {
    pub config: serde_json::Value,
}

/// The scoped dependency container. `extend` builds a child context.
#[derive(Clone)]
pub struct Context {
    pub parent: Option<Arc<Context>>,
    pub intercept: HashMap<String, Option<InterceptConfig>>,
    pub extensions: HashMap<String, Arc<dyn std::any::Any + Send + Sync>>,
    pub registry: Arc<Mutex<Registry>>,
    pub fiber: Option<Arc<crate::fiber::Fiber>>,
}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("has_parent", &self.parent.is_some())
            .field("intercept", &self.intercept.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Context {
    /// Create a new root context.
    pub fn new() -> Self {
        Self {
            parent: None,
            intercept: HashMap::new(),
            extensions: HashMap::new(),
            registry: Arc::new(Mutex::new(Registry::new())),
            fiber: None,
        }
    }

    /// Create a scoped child context.
    pub fn extend(&self, props: HashMap<String, Arc<dyn std::any::Any + Send + Sync>>) -> Self {
        let mut child = Self {
            parent: Some(Arc::new(self.clone())),
            intercept: self.intercept.clone(),
            extensions: HashMap::new(),
            registry: self.registry.clone(),
            fiber: None,
        };
        child.extensions.extend(props);
        child
    }

    pub fn get_intercept(&self, name: &str) -> Option<&InterceptConfig> {
        self.intercept.get(name).and_then(|c| c.as_ref())
            .or_else(|| self.parent.as_ref()?.get_intercept(name))
    }

    pub fn set_intercept(&mut self, name: String, config: Option<InterceptConfig>) {
        self.intercept.insert(name, config);
    }

    pub fn get<T: 'static + Send + Sync>(&self, key: &str) -> Option<Arc<T>> {
        self.extensions.get(key).and_then(|v| v.clone().downcast::<T>().ok())
            .or_else(|| self.parent.as_ref()?.get(key))
    }

    pub fn set<T: 'static + Send + Sync>(&mut self, key: String, value: Arc<T>) {
        self.extensions.insert(key, value);
    }

    /// Run a callback once the requested services are available.
    pub fn inject(&self, _deps: Inject, _cb: impl Fn(Arc<Context>) + Send + Sync) {}
}

impl Default for Context { fn default() -> Self { Self::new() } }
