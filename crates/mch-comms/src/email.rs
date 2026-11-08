//! Email (IMAP/SMTP) integration

use crate::CommsError;
use async_imap::ClientBuilder;
use lettre::{Message, SmtpTransport, Transport};
use lettre::transport::smtp::authentication::Credentials;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

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

pub struct EmailClient {
    config: EmailConfig,
    imap_client: Arc<Mutex<Option<async_imap::Client<native_tls::TlsStream<std::net::TcpStream>>>>>,
}

impl EmailClient {
    pub fn new(config: EmailConfig) -> Self {
        Self {
            config,
            imap_client: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn connect_imap(&self) -> Result<(), CommsError> {
        let tls = native_tls::TlsConnector::builder().build().map_err(|e| CommsError::Email(e.to_string()))?;
        let client = ClientBuilder::new(&self.config.imap_server, self.config.imap_port)
            .connect(tls)
            .await
            .map_err(|e| CommsError::Email(e.to_string()))?;
        
        let mut client = client
            .login(&self.config.username, &self.config.password)
            .await
            .map_err(|e| CommsError::Email(e.to_string()))?;
        
        client.select("INBOX").await.map_err(|e| CommsError::Email(e.to_string()))?;
        
        *self.imap_client.lock().await = Some(client);
        Ok(())
    }

    pub async fn fetch_unread(&self) -> Result<Vec<String>, CommsError> {
        // Simplified - would fetch actual emails
        Ok(vec![])
    }

    pub async fn send_email(&self, to: &str, subject: &str, body: &str) -> Result<(), CommsError> {
        let email = Message::builder()
            .from(self.config.username.parse().map_err(|e| CommsError::Email(e.to_string()))?)
            .to(to.parse().map_err(|e| CommsError::Email(e.to_string()))?)
            .subject(subject)
            .body(body.to_string())
            .map_err(|e| CommsError::Email(e.to_string()))?;

        let creds = Credentials::new(self.config.username.clone(), self.config.password.clone());
        let mailer = SmtpTransport::relay(&self.config.smtp_server)
            .map_err(|e| CommsError::Email(e.to_string()))?
            .port(self.config.smtp_port)
            .credentials(creds)
            .build();

        mailer.send(&email).map_err(|e| CommsError::Email(e.to_string()))?;
        Ok(())
    }
}