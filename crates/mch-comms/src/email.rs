//! Email (IMAP/SMTP) integration.
//!
//! The transport is stubbed at the boundary so the Overnight Agent can be wired
//! and tested without a live mail server; a real IMAP/SMTP backend can be
//! dropped in behind [`EmailClient`] without changing callers.

use crate::CommsError;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Email account configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub imap_server: String,
    pub imap_port: u16,
    pub smtp_server: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub use_tls: bool,
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            imap_server: String::new(),
            imap_port: 993,
            smtp_server: String::new(),
            smtp_port: 587,
            username: String::new(),
            password: String::new(),
            use_tls: true,
        }
    }
}

/// A fetched email message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailMessage {
    pub from: String,
    pub subject: String,
    pub body: String,
    pub unread: bool,
}

/// Email client. Holds an in-memory mailbox in stub mode.
pub struct EmailClient {
    config: EmailConfig,
    connected: Mutex<bool>,
    outbox: Mutex<Vec<EmailMessage>>,
    inbox: Mutex<Vec<EmailMessage>>,
}

impl EmailClient {
    pub fn new(config: EmailConfig) -> Self {
        Self {
            config,
            connected: Mutex::new(false),
            outbox: Mutex::new(Vec::new()),
            inbox: Mutex::new(Vec::new()),
        }
    }

    /// Establish an IMAP connection (stub: marks connected).
    pub async fn connect_imap(&self) -> Result<(), CommsError> {
        if self.config.imap_server.is_empty() {
            return Err(CommsError::Email("no imap_server configured".into()));
        }
        *self.connected.lock().unwrap() = true;
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        *self.connected.lock().unwrap()
    }

    /// Fetch unread messages (stub returns the seeded inbox).
    pub async fn fetch_unread(&self) -> Result<Vec<EmailMessage>, CommsError> {
        Ok(self
            .inbox
            .lock()
            .unwrap()
            .iter()
            .filter(|m| m.unread)
            .cloned()
            .collect())
    }

    /// Seed an inbound message (for tests / deterministic runs).
    pub fn seed_inbox(&self, msg: EmailMessage) {
        self.inbox.lock().unwrap().push(msg);
    }

    /// Send an email (stub records into the outbox).
    pub async fn send_email(&self, to: &str, subject: &str, body: &str) -> Result<(), CommsError> {
        self.outbox.lock().unwrap().push(EmailMessage {
            from: self.config.username.clone(),
            subject: format!("to={to} | {subject}"),
            body: body.to_string(),
            unread: false,
        });
        Ok(())
    }

    /// Messages "sent" during this session.
    pub fn outbox(&self) -> Vec<EmailMessage> {
        self.outbox.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn send_and_fetch() {
        let client = EmailClient::new(EmailConfig {
            imap_server: "imap.example.com".into(),
            username: "me@example.com".into(),
            ..Default::default()
        });
        client.connect_imap().await.unwrap();
        client.seed_inbox(EmailMessage {
            from: "lead@x.com".into(),
            subject: "Interested in pricing".into(),
            body: "tell me more".into(),
            unread: true,
        });
        assert_eq!(client.fetch_unread().await.unwrap().len(), 1);
        client.send_email("lead@x.com", "Re: pricing", "sure").await.unwrap();
        assert_eq!(client.outbox().len(), 1);
    }
}
