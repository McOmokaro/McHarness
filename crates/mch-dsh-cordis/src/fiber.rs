//! Fiber — the plugin runtime instance with a lifecycle state machine.

use std::collections::HashMap;
use std::sync::Arc;

use thiserror::Error;

use crate::context::Context;
use crate::plugin::{Inject, PluginRuntime};
use crate::utils::{Disposable, DisposableList, EffectMeta};

/// Fiber lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum FiberState {
    Pending,
    Loading,
    Active,
    Failed,
    Unloading,
    Disposed,
}

/// Errors raised by fiber operations.
#[derive(Debug, Error)]
pub enum FiberError {
    #[error("cannot create effect on inactive context")]
    InactiveEffect,
    #[error("plugin failed to load: {0}")]
    LoadError(String),
    #[error("config validation failed: {0}")]
    ValidationError(String),
}

/// Runtime instance of one plugin application.
pub struct Fiber {
    pub uid: u64,
    pub ctx: Arc<Context>,
    pub config: Option<serde_json::Value>,
    pub state: FiberState,
    pub runtime: Option<Arc<PluginRuntime>>,
    dispose_fns: DisposableList<Disposable>,
    effects: Vec<EffectMeta>,
}

impl std::fmt::Debug for Fiber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fiber").field("uid", &self.uid).field("state", &self.state).finish()
    }
}

impl Fiber {
    /// Create a new fiber under `parent`.
    pub fn new(
        parent: Arc<Context>,
        config: Option<serde_json::Value>,
        _inject: Inject,
        runtime: Option<Arc<PluginRuntime>>,
    ) -> Self {
        let uid = parent.registry.lock().unwrap().next_uid();
        let fiber_ctx = Arc::new(Context {
            parent: Some(parent.clone()),
            intercept: HashMap::new(),
            extensions: HashMap::new(),
            registry: parent.registry.clone(),
            fiber: None,
        });
        Self {
            uid,
            ctx: fiber_ctx,
            config,
            state: if runtime.is_some() { FiberState::Pending } else { FiberState::Active },
            runtime,
            dispose_fns: DisposableList::new(),
            effects: Vec::new(),
        }
    }

    /// The plugin's display name, else "root".
    pub fn name(&self) -> String {
        self.runtime.as_ref().map(|r| r.name.clone()).unwrap_or_else(|| "root".to_string())
    }

    /// Error if the fiber has been disposed.
    pub fn assert_active(&self) -> Result<(), FiberError> {
        if self.state == FiberState::Disposed { Err(FiberError::InactiveEffect) } else { Ok(()) }
    }

    /// The runtime record.
    pub fn runtime(&self) -> Option<&Arc<PluginRuntime>> { self.runtime.as_ref() }

    /// Register a cleanup-aware effect.
    pub fn effect(&mut self, dispose: Disposable, label: &str) {
        self.dispose_fns.push(dispose);
        self.effects.push(EffectMeta::new(label));
    }

    /// Labels of registered effects.
    pub fn effects(&self) -> &[EffectMeta] { &self.effects }

    /// Wait for lifecycle work and re-throw startup errors.
    pub async fn await_ready(&mut self) -> Result<(), FiberError> {
        if self.state == FiberState::Failed {
            return Err(FiberError::LoadError("fiber error".to_string()));
        }
        Ok(())
    }

    /// Reload the plugin with its current config.
    pub async fn restart(&mut self) -> Result<(), FiberError> {
        self.state = FiberState::Loading;
        self.await_ready().await?;
        self.state = FiberState::Active;
        Ok(())
    }

    /// Unload the plugin and run all disposers in reverse order.
    pub fn dispose(&mut self) {
        self.state = FiberState::Disposed;
        for d in self.dispose_fns.clear() { d(); }
    }
}
