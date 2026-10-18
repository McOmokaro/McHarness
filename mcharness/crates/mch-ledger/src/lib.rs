//! mch-ledger — technique ledger with status taxonomy + crash-safe persistence.
//!
//! Mined from pentest-agents (`tools/brain.py` `_STATUS_TAG` + `file_safety.py`):
//! exhaustion knowledge as queryable records keyed by (target, class, technique),
//! a closed status vocabulary, and poisoned-file quarantine.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unknown status `{0}` — vocabulary is closed")]
    UnknownStatus(String),
    #[error("corrupt ledger file {0}: {1}")]
    Corrupt(PathBuf, String),
}

/// The 12-value status vocabulary (closed set, mirroring brain.py `_STATUS_TAG`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TechniqueStatus {
    Attempted,
    Confirmed,
    FalsePositive,
    Exhausted,
    Effective,
    WafBypass,
    Blocked,
    RateLimited,
    DaKilled,
    BrowserRejected,
    NeedsHumanReview,
    Retired,
}

impl TechniqueStatus {
    pub fn parse(s: &str) -> Result<Self, LedgerError> {
        serde_json::from_value(serde_json::Value::String(s.to_string()))
            .map_err(|_| LedgerError::UnknownStatus(s.to_string()))
    }

    pub fn is_exhaustion(&self) -> bool {
        matches!(self, TechniqueStatus::Exhausted | TechniqueStatus::Retired)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechniqueRecord {
    pub target: String,
    pub class: String,
    pub technique: String,
    pub status: TechniqueStatus,
    pub note: String,
    #[serde(default)]
    pub attempts: u32,
}

/// Queryable ledger — the anti-repetition brain. `EXHAUSTED — Do NOT retry`
/// comes from records with exhaustion statuses.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TechniqueLedger {
    records: BTreeMap<String, TechniqueRecord>,
}

fn key(target: &str, class: &str, technique: &str) -> String {
    format!("{target}\u{1f}{class}\u{1f}{technique}")
}

impl TechniqueLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(
        &mut self,
        target: &str,
        class: &str,
        technique: &str,
        status: TechniqueStatus,
        note: &str,
    ) {
        let entry = self
            .records
            .entry(key(target, class, technique))
            .or_insert_with(|| TechniqueRecord {
                target: target.to_string(),
                class: class.to_string(),
                technique: technique.to_string(),
                status,
                note: note.to_string(),
                attempts: 0,
            });
        entry.status = status;
        if !note.is_empty() {
            entry.note = note.to_string();
        }
        entry.attempts += 1;
    }

    pub fn get(&self, target: &str, class: &str, technique: &str) -> Option<&TechniqueRecord> {
        self.records.get(&key(target, class, technique))
    }

    pub fn by_target(&self, target: &str) -> Vec<&TechniqueRecord> {
        self.records.values().filter(|r| r.target == target).collect()
    }

    /// The brief handed to every agent dispatch: do-not-retry list.
    pub fn exhausted_brief(&self, target: &str) -> Vec<String> {
        self.by_target(target)
            .iter()
            .filter(|r| r.status.is_exhaustion())
            .map(|r| format!("EXHAUSTED — Do NOT retry: {}/{}", r.class, r.technique))
            .collect()
    }

    /// Crash-safe persistence: temp file + fsync + atomic rename, and corrupt
    /// existing files are quarantined as `.corrupt-<epoch>` rather than silently
    /// overwritten (mirrors `file_safety.py load_json_or_quarantine`).
    pub fn save(&self, path: &Path) -> Result<(), LedgerError> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        if path.exists() {
            if let Err(e) = serde_json::from_str::<TechniqueLedger>(
                &fs::read_to_string(path).unwrap_or_default(),
            ) {
                let quarantined = path.with_extension(format!(
                    "corrupt-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0)
                ));
                fs::rename(path, &quarantined)?;
                eprintln!("quarantined corrupt ledger -> {}", quarantined.display());
                let _ = e;
            }
        }
        let json = serde_json::to_string_pretty(self)?;
        let tmp = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
        {
            let mut f = fs::File::create(tmp.path())?;
            f.write_all(json.as_bytes())?;
            f.sync_all()?;
        }
        fs::rename(tmp.path(), path)?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, LedgerError> {
        match fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| {
                LedgerError::Corrupt(path.to_path_buf(), e.to_string())
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::new()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("mch-ledger-{name}-{}", std::process::id()))
    }

    #[test]
    fn closed_vocabulary() {
        assert!(TechniqueStatus::parse("exhausted").is_ok());
        assert!(TechniqueStatus::parse("made-up-status").is_err());
    }

    #[test]
    fn record_and_brief() {
        let mut l = TechniqueLedger::new();
        l.record("acme.com", "rce", "log4shell", TechniqueStatus::Exhausted, "tried all vectors");
        l.record("acme.com", "idor", "id-enumeration", TechniqueStatus::Confirmed, "10 findings");
        let brief = l.exhausted_brief("acme.com");
        assert_eq!(brief.len(), 1);
        assert!(brief[0].contains("rce/log4shell"));
        assert_eq!(l.get("acme.com", "idor", "id-enumeration").unwrap().attempts, 1);
    }

    #[test]
    fn save_load_roundtrip() {
        let path = tmp_path("roundtrip");
        let mut l = TechniqueLedger::new();
        l.record("t", "c", "tech", TechniqueStatus::WafBypass, "note");
        l.save(&path).unwrap();
        let l2 = TechniqueLedger::load(&path).unwrap();
        assert_eq!(l2.get("t", "c", "tech").unwrap().status, TechniqueStatus::WafBypass);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn corrupt_file_is_quarantined() {
        let path = tmp_path("corrupt");
        fs::write(&path, "{not json").unwrap();
        let mut l = TechniqueLedger::new();
        l.record("t", "c", "tech", TechniqueStatus::Blocked, "");
        l.save(&path).unwrap();
        let quarantines: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("corrupt"))
            .collect();
        assert!(!quarantines.is_empty());
        let l2 = TechniqueLedger::load(&path).unwrap();
        assert!(l2.get("t", "c", "tech").is_some());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn missing_file_loads_empty() {
        let l = TechniqueLedger::load(Path::new("/nonexistent/mch/ledger.json")).unwrap();
        assert_eq!(l.by_target("x").len(), 0);
    }
}
