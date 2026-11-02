//! Skill-sealing — experience becomes a reusable, ledgered, locked skill.
//!
//! Mined from Hermes Agent's `tools/skill_manager_tool.py`. Skills are the agent's
//! PROCEDURAL memory: narrow "how to do X", loaded on demand. Two rules make the engine
//! safe to let an agent drive: the description budget (the system-prompt index shows 60
//! chars, so anything longer silently never routes), and the append-only ledger that makes
//! every mutation reversible.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The system-prompt skill index shows this many characters of the description. Past it,
/// the description is silently cut and the skill never routes. This is a routing constraint.
pub const SKILL_INDEX_DESC_LIMIT: usize = 60;
/// A SKILL.md body above this is a consolidation target on its own.
pub const MAX_SKILL_BODY: usize = 100_000;
pub const MAX_NAME_LENGTH: usize = 64;
/// Supporting files live under one of these subdirectories.
pub const ALLOWED_SUBDIRS: [&str; 4] = ["references", "templates", "scripts", "assets"];

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SkillError {
    #[error("invalid skill name '{0}': lowercase letters, digits, hyphens, dots, underscores; must start alphanumeric")]
    BadName(String),
    #[error("SKILL.md must start with YAML frontmatter (---) and include name and description")]
    BadFrontmatter,
    #[error("description is {len} chars; the skill index shows {SKILL_INDEX_DESC_LIMIT}")]
    DescriptionBudget { len: usize },
    #[error("SKILL.md content is {len} chars (limit {MAX_SKILL_BODY})")]
    BodyTooLarge { len: usize },
    #[error("skill '{0}' already exists")]
    Exists(String),
    #[error("skill '{0}' not found")]
    NotFound(String),
    #[error("file must be under one of: {0}")]
    BadFilePath(String),
    #[error("read-before-write required: view '{0}' in this turn before editing it")]
    ReadBeforeWrite(String),
    #[error("a skill named '{0}' already exists; delete it first")]
    AbsorbTargetMissing(String),
    #[error("io: {0}")]
    Io(String),
}

/// One operation. A caller passes a list; the engine applies them in order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkillOp {
    Create {
        name: String,
        content: String,
        category: Option<String>,
    },
    /// Targeted find-and-replace in SKILL.md or a supporting file.
    Patch {
        name: String,
        old_string: String,
        new_string: String,
        file_path: Option<String>,
    },
    WriteFile {
        name: String,
        file_path: String,
        file_content: String,
    },
    RemoveFile {
        name: String,
        file_path: String,
    },
    /// Archive a skill. `absorbed_into` names the umbrella that took its content.
    Delete {
        name: String,
        absorbed_into: Option<String>,
    },
}

/// One append-only ledger entry. Before/after text so a change can be reversed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub action: String,
    pub name: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub actor: String,
}

/// Append-only JSONL ledger of every skill mutation.
#[derive(Debug, Default)]
pub struct SkillLedger {
    path: PathBuf,
}

impl SkillLedger {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn record(&self, entry: &LedgerEntry) -> Result<(), SkillError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(io)?;
        }
        let line = serde_json::to_string(entry).map_err(|e| SkillError::Io(e.to_string()))?;
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(io)?;
        writeln!(file, "{line}").map_err(io)
    }
}

/// File-backed skill store. Skills live at `<root>/[category/]<skill>/SKILL.md`.
pub struct SkillStore {
    root: PathBuf,
    ledger: SkillLedger,
    archive_dir: PathBuf,
    /// A minimal read-before-write set: names viewed (or created) this turn.
    read_this_turn: std::cell::RefCell<std::collections::HashSet<String>>,
}

impl SkillStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let archive_dir = root.join(".archive");
        let ledger = SkillLedger::new(root.join(".ledger.jsonl"));
        Self {
            root,
            ledger,
            archive_dir,
            read_this_turn: std::cell::RefCell::new(std::collections::HashSet::new()),
        }
    }

    pub fn ledger(&self) -> &SkillLedger {
        &self.ledger
    }

    pub fn skill_dir(&self, name: &str, category: Option<&str>) -> PathBuf {
        match category {
            Some(cat) => self.root.join(cat).join(name),
            None => self.root.join(name),
        }
    }

    /// Find a skill directory by name anywhere under the root.
    pub fn find(&self, name: &str) -> Option<PathBuf> {
        find_skill_dir(&self.root, name)
    }

    /// Mark a skill as read so a later patch is allowed (the read-before-write gate).
    pub fn mark_read(&self, name: &str) {
        self.read_this_turn.borrow_mut().insert(name.to_string());
    }

    /// Create a skill. Validates name, frontmatter and the description budget first.
    pub fn create(
        &self,
        name: &str,
        content: &str,
        category: Option<&str>,
    ) -> Result<PathBuf, SkillError> {
        validate_name(name)?;
        validate_frontmatter(content)?;
        validate_body_size(content)?;
        if self.find(name).is_some() {
            return Err(SkillError::Exists(name.to_string()));
        }
        let dir = self.skill_dir(name, category);
        fs::create_dir_all(&dir).map_err(io)?;
        let skill_md = dir.join("SKILL.md");
        fs::write(&skill_md, content).map_err(io)?;
        self.mark_read(name);
        self.ledger.record(&LedgerEntry {
            action: "create".into(),
            name: name.into(),
            before: None,
            after: Some(content.into()),
            actor: "agent".into(),
        })?;
        Ok(skill_md)
    }

    /// Targeted replace in SKILL.md (or a supporting file).
    pub fn patch(
        &self,
        name: &str,
        old_string: &str,
        new_string: &str,
        file_path: Option<&str>,
    ) -> Result<(), SkillError> {
        let dir = self.find(name).ok_or_else(|| SkillError::NotFound(name.into()))?;
        if !self.read_this_turn.borrow().contains(name) {
            return Err(SkillError::ReadBeforeWrite(name.into()));
        }
        let target = match file_path {
            Some(fp) => {
                validate_file_path(fp)?;
                dir.join(fp)
            }
            None => dir.join("SKILL.md"),
        };
        let before = fs::read_to_string(&target).map_err(io)?;
        let count = before.matches(old_string).count();
        if count != 1 {
            return Err(SkillError::Io(format!(
                "old_string must occur exactly once; found {count}"
            )));
        }
        let after = before.replacen(old_string, new_string, 1);
        if file_path.is_none() {
            validate_frontmatter(&after)?;
        }
        validate_body_size(&after)?;
        fs::write(&target, &after).map_err(io)?;
        self.ledger.record(&LedgerEntry {
            action: "patch".into(),
            name: name.into(),
            before: Some(before),
            after: Some(after),
            actor: "agent".into(),
        })
    }

    /// Add or overwrite a supporting file.
    pub fn write_file(
        &self,
        name: &str,
        file_path: &str,
        content: &str,
    ) -> Result<(), SkillError> {
        validate_file_path(file_path)?;
        let dir = self.find(name).ok_or_else(|| SkillError::NotFound(name.into()))?;
        let target = dir.join(file_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(io)?;
        }
        fs::write(&target, content).map_err(io)?;
        self.ledger.record(&LedgerEntry {
            action: "write_file".into(),
            name: name.into(),
            before: None,
            after: Some(content.into()),
            actor: "agent".into(),
        })
    }

    /// Remove a supporting file.
    pub fn remove_file(&self, name: &str, file_path: &str) -> Result<(), SkillError> {
        validate_file_path(file_path)?;
        let dir = self.find(name).ok_or_else(|| SkillError::NotFound(name.into()))?;
        let target = dir.join(file_path);
        if !target.exists() {
            return Err(SkillError::NotFound(file_path.to_string()));
        }
        fs::remove_file(&target).map_err(io)?;
        self.ledger.record(&LedgerEntry {
            action: "remove_file".into(),
            name: name.into(),
            before: Some(file_path.into()),
            after: None,
            actor: "agent".into(),
        })
    }

    /// Archive a skill. Never deletes: a consolidation must be recoverable.
    pub fn delete(&self, name: &str, absorbed_into: Option<&str>) -> Result<(), SkillError> {
        if let Some(target) = absorbed_into {
            if target == name {
                return Err(SkillError::Io(
                    "absorbed_into cannot equal the skill being deleted".into(),
                ));
            }
            if self.find(target).is_none() {
                return Err(SkillError::AbsorbTargetMissing(target.into()));
            }
        }
        let dir = self.find(name).ok_or_else(|| SkillError::NotFound(name.into()))?;
        fs::create_dir_all(&self.archive_dir).map_err(io)?;
        let leaf = dir.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| name.into());
        let dest = self.archive_dir.join(&leaf);
        fs::rename(&dir, &dest).map_err(io)?;
        self.ledger.record(&LedgerEntry {
            action: "archive".into(),
            name: name.into(),
            before: Some(dir.to_string_lossy().to_string()),
            after: Some(dest.to_string_lossy().to_string()),
            actor: "curator".into(),
        })
    }
}

fn io(e: std::io::Error) -> SkillError {
    SkillError::Io(e.to_string())
}

fn find_skill_dir(root: &Path, name: &str) -> Option<PathBuf> {
    let direct = root.join(name).join("SKILL.md");
    if direct.exists() {
        return Some(root.join(name));
    }
    // One level of category nesting. Skip dot-directories: .archive/.hub/.locks are not the
    // live library, and an archived skill must not resolve as if it were still active.
    if let Ok(read) = fs::read_dir(root) {
        for entry in read.flatten() {
            let path = entry.path();
            let is_hidden = path
                .file_name()
                .map(|n| n.to_string_lossy().starts_with('.'))
                .unwrap_or(false);
            if path.is_dir() && !is_hidden {
                let candidate = path.join(name).join("SKILL.md");
                if candidate.exists() {
                    return Some(path.join(name));
                }
            }
        }
    }
    None
}

/// Lowercase letters, digits, hyphens, dots, underscores; must start alphanumeric.
pub fn validate_name(name: &str) -> Result<(), SkillError> {
    if name.is_empty() || name.len() > MAX_NAME_LENGTH {
        return Err(SkillError::BadName(name.into()));
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphanumeric() {
        return Err(SkillError::BadName(name.into()));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '.' | '_'))
    {
        return Err(SkillError::BadName(name.into()));
    }
    Ok(())
}

/// Require `---` frontmatter with a `name:` and `description:` key, and a non-empty body.
pub fn validate_frontmatter(content: &str) -> Result<(), SkillError> {
    let trimmed = content.trim_start_matches('\u{feff}');
    if !trimmed.starts_with("---") {
        return Err(SkillError::BadFrontmatter);
    }
    let rest = &trimmed[3..];
    let end = rest.find("\n---").ok_or(SkillError::BadFrontmatter)?;
    let front = &rest[..end];
    let has_name = front.lines().any(|l| l.trim_start().starts_with("name:"));
    let has_desc = front.lines().any(|l| l.trim_start().starts_with("description:"));
    if !has_name || !has_desc {
        return Err(SkillError::BadFrontmatter);
    }
    let body = &rest[end + 4..];
    if body.trim().is_empty() {
        return Err(SkillError::BadFrontmatter);
    }
    let desc = front
        .lines()
        .find(|l| l.trim_start().starts_with("description:"))
        .map(|l| l.splitn(2, ':').nth(1).unwrap_or("").trim().trim_matches('"'))
        .unwrap_or("");
    if desc.chars().count() > SKILL_INDEX_DESC_LIMIT {
        return Err(SkillError::DescriptionBudget {
            len: desc.chars().count(),
        });
    }
    Ok(())
}

pub fn validate_body_size(content: &str) -> Result<(), SkillError> {
    if content.chars().count() > MAX_SKILL_BODY {
        return Err(SkillError::BodyTooLarge {
            len: content.chars().count(),
        });
    }
    Ok(())
}

/// A supporting file must sit under an allowed subdirectory, with no traversal.
pub fn validate_file_path(file_path: &str) -> Result<(), SkillError> {
    if file_path.contains("..") {
        return Err(SkillError::BadFilePath(file_path.into()));
    }
    if file_path == "SKILL.md" {
        return Ok(());
    }
    let first = file_path.split('/').next().unwrap_or("");
    if !ALLOWED_SUBDIRS.contains(&first) {
        return Err(SkillError::BadFilePath(ALLOWED_SUBDIRS.join(", ")));
    }
    if !file_path.contains('/') {
        return Err(SkillError::BadFilePath(
            "provide a file path, not just a directory".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> SkillStore {
        SkillStore::new("/tmp/mch-skill-test")
    }

    const GOOD: &str = "---\nname: demo\ndescription: Search arXiv papers by keyword, author, or ID.\n---\n\n# Demo\n\nBody here.\n";

    #[test]
    fn name_validation() {
        assert!(validate_name("good-name.v2").is_ok());
        assert!(validate_name("Bad Name").is_err());
        assert!(validate_name("-leading").is_err());
    }

    #[test]
    fn description_budget_enforced() {
        let long = format!(
            "---\nname: x\ndescription: {}\n---\n\nbody\n",
            "a".repeat(SKILL_INDEX_DESC_LIMIT + 1)
        );
        assert!(matches!(
            validate_frontmatter(&long),
            Err(SkillError::DescriptionBudget { .. })
        ));
    }

    #[test]
    fn create_patch_delete_roundtrip() {
        let s = store();
        let _ = fs::remove_dir_all(&s.root);
        s.create("demo", GOOD, Some("research")).unwrap();
        s.patch("demo", "Body here.", "New body.", None).unwrap();
        assert!(fs::read_to_string(s.find("demo").unwrap().join("SKILL.md"))
            .unwrap()
            .contains("New body."));
        s.delete("demo", None).unwrap();
        assert!(s.find("demo").is_none());
    }

    #[test]
    fn patch_requires_prior_read() {
        let s = store();
        let _ = fs::remove_dir_all(&s.root);
        s.create("demo2", GOOD, None).unwrap();
        let fresh = SkillStore::new("/tmp/mch-skill-test");
        let err = fresh
            .patch("demo2", "Body here.", "x", None)
            .unwrap_err();
        assert!(matches!(err, SkillError::ReadBeforeWrite(_)));
    }
}
