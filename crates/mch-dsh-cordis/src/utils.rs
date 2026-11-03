//! Utils — shared utilities for the Cordis kernel.

use std::collections::HashMap;

/// Dict type alias.
pub type Dict<T> = HashMap<String, T>;

/// A LIFO list of disposables run in reverse registration order.
#[derive(Debug, Default)]
pub struct DisposableList<T> {
    items: Vec<T>,
}

impl<T> DisposableList<T> {
    pub fn new() -> Self { Self { items: Vec::new() } }
    pub fn push(&mut self, item: T) { self.items.push(item); }
    pub fn pop(&mut self) -> Option<T> { self.items.pop() }
    pub fn clear(&mut self) -> Vec<T> { std::mem::take(&mut self.items) }
    pub fn len(&self) -> usize { self.items.len() }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl<T> IntoIterator for DisposableList<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter { self.items.into_iter() }
}

/// A boxed one-shot cleanup closure.
pub type Disposable = Box<dyn FnOnce() + Send>;

/// Human-readable label tree for effect diagnostics.
#[derive(Debug, Clone)]
pub struct EffectMeta {
    pub label: String,
    pub children: Vec<EffectMeta>,
}

impl EffectMeta {
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), children: Vec::new() }
    }
}

/// Build an (empty) outer-stack capture for diagnostics.
pub fn build_outer_stack() -> Vec<String> { Vec::new() }
