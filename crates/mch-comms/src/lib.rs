//! mch-comms — Telegram, Email (IMAP/SMTP), Calendar (CalDAV) integrations
//! with deterministic routing and local/cheap model classification.

pub mod telegram;
pub mod email;
pub mod calendar;
pub mod router;

pub use telegram::{TelegramClient, TelegramConfig};
pub use email::{EmailClient, EmailConfig};
pub use calendar::{CalendarClient, CalendarConfig};
pub use router::{DeterministicRouter, ModelClassifier, RouteDecision};

/// Communication error types
#[derive(Debug, thiserror::Error)]
pub enum CommsError {
    #[error("Telegram error: {0}")]
    Telegram(String),
    #[error("Email error: {0}")]
    Email(String),
    #[error("Calendar error: {0}")]
    Calendar(String),
    #[error("Routing error: {0}")]
    Routing(String),
}

pub type Result<T> = std::result::Result<T, CommsError>;