//! Service — base trait for provided capabilities.

/// A capability a plugin provides to the container.
pub trait Service: Send + Sync {
    fn name(&self) -> &str;
}

/// A simple named service.
pub struct BaseService { name: String }

impl BaseService {
    pub fn new(name: impl Into<String>) -> Self { Self { name: name.into() } }
}

impl Service for BaseService {
    fn name(&self) -> &str { &self.name }
}
