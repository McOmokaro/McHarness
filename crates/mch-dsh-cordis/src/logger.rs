//! Logger — a small leveled logger.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// Log levels, ordered from most to least verbose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel { Trace = 0, Debug = 1, Info = 2, Warn = 3, Error = 4 }

/// A leveled logger scoped to a named component.
#[derive(Debug, Clone)]
pub struct Logger {
    level: Arc<AtomicU8>,
    name: Arc<str>,
}

impl Logger {
    pub fn new(name: impl Into<String>) -> Self {
        Self { level: Arc::new(AtomicU8::new(LogLevel::Info as u8)), name: name.into().into() }
    }

    pub fn with_level(level: LogLevel) -> Self {
        let l = Self::new("default");
        l.set_level(level);
        l
    }

    pub fn set_level(&self, level: LogLevel) { self.level.store(level as u8, Ordering::SeqCst); }

    pub fn get_level(&self) -> LogLevel {
        match self.level.load(Ordering::SeqCst) {
            0 => LogLevel::Trace,
            1 => LogLevel::Debug,
            2 => LogLevel::Info,
            3 => LogLevel::Warn,
            _ => LogLevel::Error,
        }
    }

    pub fn log(&self, level: LogLevel, msg: &str) {
        if level >= self.get_level() {
            eprintln!("[{:?}] {}: {}", level, self.name, msg);
        }
    }
}

impl Default for Logger { fn default() -> Self { Self::new("default") } }
