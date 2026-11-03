//! Events — lightweight synchronous pub/sub for plugin lifecycle.

use std::collections::HashMap;

/// An event listener.
pub type Callback = Box<dyn Fn(&[serde_json::Value]) -> Option<serde_json::Value> + Send + Sync>;

/// A simple synchronous event bus.
#[derive(Default)]
pub struct EventBus {
    listeners: HashMap<String, Vec<Callback>>,
}

impl EventBus {
    pub fn new() -> Self { Self::default() }

    pub fn on(&mut self, event: impl Into<String>, cb: Callback) {
        self.listeners.entry(event.into()).or_default().push(cb);
    }

    pub fn emit(&self, event: &str, args: &[serde_json::Value]) {
        if let Some(cbs) = self.listeners.get(event) {
            for cb in cbs { let _ = cb(args); }
        }
    }

    pub fn waterfall(&self, event: &str, mut value: serde_json::Value) -> serde_json::Value {
        if let Some(cbs) = self.listeners.get(event) {
            for cb in cbs {
                if let Some(next) = cb(&[value.clone()]) { value = next; }
            }
        }
        value
    }
}

/// Result of a waterfall transform.
#[derive(Debug, Clone)]
pub struct WaterfallResult<T> {
    pub value: T,
    pub modified: bool,
}

impl<T> WaterfallResult<T> {
    pub fn new(value: T) -> Self { Self { value, modified: false } }
}

/// Alias.
pub type Waterfall<T> = WaterfallResult<T>;
