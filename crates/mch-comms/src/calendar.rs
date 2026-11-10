//! Calendar (CalDAV) integration — in-memory store behind a CalDAV-shaped API.

use crate::CommsError;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Calendar account configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarConfig {
    pub caldav_url: String,
    pub username: String,
    pub password: String,
}

impl Default for CalendarConfig {
    fn default() -> Self {
        Self {
            caldav_url: String::new(),
            username: String::new(),
            password: String::new(),
        }
    }
}

/// A calendar event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub calendar: String,
    pub summary: String,
    pub start: chrono::DateTime<chrono::Utc>,
    pub end: chrono::DateTime<chrono::Utc>,
}

/// Calendar client with an in-memory event store.
pub struct CalendarClient {
    config: CalendarConfig,
    calendars: Vec<String>,
    events: Mutex<Vec<CalendarEvent>>,
}

impl CalendarClient {
    pub fn new(config: CalendarConfig) -> Result<Self, CommsError> {
        Ok(Self {
            config,
            calendars: vec!["default".to_string()],
            events: Mutex::new(Vec::new()),
        })
    }

    pub async fn list_calendars(&self) -> Result<Vec<String>, CommsError> {
        Ok(self.calendars.clone())
    }

    pub async fn create_event(
        &self,
        calendar: &str,
        summary: &str,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<String, CommsError> {
        let id = uuid::Uuid::new_v4().to_string();
        self.events.lock().unwrap().push(CalendarEvent {
            id: id.clone(),
            calendar: calendar.to_string(),
            summary: summary.to_string(),
            start,
            end,
        });
        Ok(id)
    }

    pub async fn get_events(
        &self,
        calendar: &str,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<CalendarEvent>, CommsError> {
        Ok(self
            .events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.calendar == calendar && e.start >= start && e.start <= end)
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_and_list_event() {
        let client = CalendarClient::new(CalendarConfig::default()).unwrap();
        let now = chrono::Utc::now();
        client
            .create_event("default", "hunt kickoff", now, now + chrono::Duration::hours(1))
            .await
            .unwrap();
        let events = client
            .get_events("default", now - chrono::Duration::hours(1), now + chrono::Duration::hours(2))
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].summary, "hunt kickoff");
    }
}
