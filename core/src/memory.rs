//! Memory: three strata, compiled in. Not a pile of prompts.

use anyhow::Result;
use rusqlite::Connection;
use std::sync::Mutex;

pub struct Memory {
    conn: Mutex<Connection>,
    working: Vec<(String, String)>,
}

impl Memory {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS persistent_memory (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
            working: Vec::new(),
        })
    }

    /// Working memory: the live session context.
    pub fn working_append(&mut self, kind: &str, content: &str) -> Result<()> {
        self.working.push((kind.to_string(), content.to_string()));
        Ok(())
    }

    /// Persistent memory: across sessions.
    pub fn persistent_append(&self, kind: &str, content: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO persistent_memory (kind, content) VALUES (?1, ?2)",
            [kind, content],
        )?;
        Ok(())
    }

    /// Build the live context the loop infers against — the Whole, not a transcript.
    pub fn build_context(&self) -> String {
        self.working
            .iter()
            .map(|(k, v)| format!("[{k}] {v}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn recall_persistent(&self, kind: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT content FROM persistent_memory WHERE kind = ?1 ORDER BY id DESC LIMIT 50",
        )?;
        let rows = stmt.query_map([kind], |row| row.get::<_, String>(0))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}
