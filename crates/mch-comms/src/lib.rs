//! mch-comms — Telegram, Email, Calendar for the Overnight Agent, plus the
//! deterministic "Low Fuel" router that decides which tasks need a model.

pub mod calendar;
pub mod email;
pub mod router;
pub mod telegram;

pub use calendar::{CalendarClient, CalendarConfig, CalendarEvent};
pub use email::{EmailClient, EmailConfig, EmailMessage};
pub use router::{
    ClassificationResult, DeterministicRouter, MessageCategory, ModelClassifier, RouteDecision,
    RouteType,
};
pub use telegram::{TelegramClient, TelegramConfig};

/// Communication error types.
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
