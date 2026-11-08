//! Calendar (CalDAV) integration using fast-dav-rs

use crate::CommsError;
use fast_dav_rs::{
    client::{DavClient, DavClientBuilder},
    models::{Calendar, Event},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarConfig {
    pub caldav_url: String,
    pub username: String,
    pub password: String,
}

pub struct CalendarClient {
    client: DavClient,
    config: CalendarConfig,
}

impl CalendarClient {
    pub fn new(config: CalendarConfig) -> Result<Self, CommsError> {
        let client = DavClientBuilder::new()
            .url(&config.caldav_url)
            .username(&config.username)
            .password(&config.password)
            .build()
            .map_err(|e| CommsError::Calendar(e.to_string()))?;
        Ok(Self { client, config })
    }

    pub async fn list_calendars(&self) -> Result<Vec<String>, CommsError> {
        let calendars = self.client.list_calendars().await
            .map_err(|e| CommsError::Calendar(e.to_string()))?;
        Ok(calendars.into_iter().map(|c: Calendar| c.name).collect())
    }

    pub async fn create_event(&self, calendar: &str, summary: &str, start: chrono::DateTime<chrono::Utc>, end: chrono::DateTime<chrono::Utc>) -> Result<String, CommsError> {
        let event = Event {
            summary: summary.to_string(),
            dtstart: start,
            dtend: end,
            ..Default::default()
        };
        let id = self.client.create_event(calendar, event).await
            .map_err(|e| CommsError::Calendar(e.to_string()))?;
        Ok(id)
    }

    pub async fn get_events(&self, calendar: &str, start: chrono::DateTime<chrono::Utc>, end: chrono::DateTime<chrono::Utc>) -> Result<Vec<Event>, CommsError> {
        let events = self.client.get_events(calendar, start, end).await
            .map_err(|e| CommsError::Calendar(e.to_string()))?;
        Ok(events)
    }
}