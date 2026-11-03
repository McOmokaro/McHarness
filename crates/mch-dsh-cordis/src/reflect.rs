//! Reflect — service reflection, provide/inject/intercept.

use std::sync::Arc;

use crate::service::Service;

/// A service implementation held by a fiber.
#[derive(Clone)]
pub struct Impl {
    pub name: String,
    pub value: Arc<dyn std::any::Any + Send + Sync>,
}

impl std::fmt::Debug for Impl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Impl").field("name", &self.name).finish()
    }
}

impl Impl {
    pub fn new(name: impl Into<String>, value: Arc<dyn std::any::Any + Send + Sync>) -> Self {
        Self { name: name.into(), value }
    }
}

/// A service that can be provided dynamically.
pub trait Provide {
    fn name(&self) -> &str;
    fn value(&self) -> Arc<dyn std::any::Any + Send + Sync>;
}

/// Object-safe service alias.
pub type DynService = dyn Service + Send + Sync;

/// Intercept configuration for a service.
pub type InterceptConfig = serde_json::Value;

/// Key of an injected service.
pub type InjectKey = String;
