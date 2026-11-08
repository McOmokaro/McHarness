//! Workflow Extraction — Ingest transcripts/recordings → procedural memory
//!
//! This module provides functionality to extract structured workflows from
//! unstructured sources (screen recordings, transcripts, logs) and convert
//! them into procedural skills for the mch-brain skill store.

use anyhow::Result;
use chrono::{DateTime, Utc};
use mch_brain::{SkillEntry, SkillStore, types::SessionId};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Source type for workflow extraction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowSourceType {
    /// Terminal session transcript
    TerminalTranscript,
    /// Screen recording with OCR/text extraction
    ScreenRecording,
    /// Meeting transcript
    MeetingTranscript,
    /// Code review session
    CodeReview,
    /// Debugging session
    DebuggingSession,
    /// Manual documentation
    ManualDocumentation,
    /// API call logs
    ApiLogs,
    /// Git history
    GitHistory,
}

/// A single step in an extracted workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    /// Unique step identifier
    pub step_id: Uuid,
    /// Step sequence number
    pub sequence: usize,
    /// Human-readable intent/goal of this step
    pub intent: String,
    /// Detailed procedure (commands, actions, decisions)
    pub procedure: String,
    /// Context in which this step applies
    pub context: String,
    /// Tools/commands used in this step
    pub tools: Vec<String>,
    /// Preconditions for this step
    pub preconditions: Vec<String>,
    /// Expected outcomes
    pub expected_outcomes: Vec<String>,
    /// Actual outcome observed (if available)
    pub actual_outcome: Option<String>,
    /// Duration in seconds
    pub duration_seconds: Option<u64>,
    /// Success indicator
    pub success: Option<bool>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Confidence in extraction (0.0 - 1.0)
    pub extraction_confidence: f32,
}

/// Complete extracted workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRecord {
    /// Unique workflow identifier
    pub workflow_id: Uuid,
    /// Source type
    pub source_type: WorkflowSourceType,
    /// Source identifier (file path, session ID, etc.)
    pub source_id: String,
    /// Human-readable title
    pub title: String,
    /// Description
    pub description: String,
    /// Extracted steps in sequence
    pub steps: Vec<WorkflowStep>,
    /// Domain/tags
    pub tags: Vec<String>,
    /// Extraction timestamp
    pub extracted_at: DateTime<Utc>,
    /// Overall confidence
    pub confidence: f32,
    /// Source session
    pub session_id: SessionId,
}

/// Parser for terminal transcripts
pub struct TerminalTranscriptParser {
    command_pattern: Regex,
    output_pattern: Regex,
    error_pattern: Regex,
    prompt_pattern: Regex,
}

impl Default for TerminalTranscriptParser {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalTranscriptParser {
    pub fn new() -> Self {
        Self {
            // Match common shell prompts and commands
            command_pattern: Regex::new(r"(?m)^[$\#>]\s*(.+)$").unwrap(),
            // Match command output (lines not starting with prompt)
            output_pattern: Regex::new(r"(?m)^(?![$#>]).+$").unwrap(),
            // Match error patterns
            error_pattern: Regex::new(r"(?i)(error|fail|exception|panic|abort|denied|not found|permission denied)").unwrap(),
            // Match common prompts
            prompt_pattern: Regex::new(r"^[\w\-]+[@:][\w\-/]*[$#>]\s*").unwrap(),
        }
    }

    /// Parse a terminal transcript into workflow steps
    pub fn parse(&self, transcript: &str, session_id: SessionId) -> Result<WorkflowRecord> {
        let lines: Vec<&str> = transcript.lines().collect();
        let mut steps = Vec::new();
        let mut current_command = None;
        let mut current_output = Vec::new();
        let mut sequence = 0;

        for line in lines {
            let trimmed = line.trim();

            // Skip empty lines
            if trimmed.is_empty() {
                continue;
            }

            // Check if this is a command
            if let Some(caps) = self.command_pattern.captures(line) {
                // Save previous command if exists
                if let Some(cmd) = current_command.take() {
                    sequence += 1;
                    let step = self.create_step(
                        sequence,
                        cmd,
                        current_output.clone(),
                        session_id,
                    );
                    steps.push(step);
                    current_output.clear();
                }

                current_command = Some(caps[1].to_string());
            } else if current_command.is_some() {
                // This is output for the current command
                current_output.push(line.to_string());
            } else {
                // Orphan output, treat as context
                current_output.push(line.to_string());
            }
        }

        // Don't forget the last command
        if let Some(cmd) = current_command {
            sequence += 1;
            let step = self.create_step(sequence, cmd, current_output, session_id);
            steps.push(step);
        }

        // Filter and merge steps
        let steps = self.post_process_steps(steps);

        let workflow_id = Uuid::new_v4();
        let confidence = self.calculate_confidence(&steps);

        Ok(WorkflowRecord {
            workflow_id,
            source_type: WorkflowSourceType::TerminalTranscript,
            source_id: format!("session_{}", session_id.0),
            title: "Terminal Session Workflow".to_string(),
            description: format!("Extracted {} steps from terminal transcript", steps.len()),
            steps,
            tags: vec!["terminal".to_string(), "cli".to_string()],
            extracted_at: Utc::now(),
            confidence,
            session_id,
        })
    }

    fn create_step(
        &self,
        sequence: usize,
        command: String,
        output: Vec<String>,
        session_id: SessionId,
    ) -> WorkflowStep {
        let intent = self.extract_intent(&command);
        let tools = self.extract_tools(&command);
        let success = self.assess_success(&output);
        let has_error = output.iter().any(|o| self.error_pattern.is_match(o));

        WorkflowStep {
            step_id: Uuid::new_v4(),
            sequence,
            intent,
            procedure: format!("```bash\n{}\n```", command),
            context: output.join("\n").chars().take(500).collect::<String>(),
            tools,
            preconditions: vec![],
            expected_outcomes: vec![],
            actual_outcome: if output.is_empty() { None } else { Some(output.join("\n").chars().take(1000).collect()) },
            duration_seconds: None,
            success,
            timestamp: Utc::now(),
            extraction_confidence: if has_error { 0.6 } else { 0.85 },
        }
    }

    fn extract_intent(&self, command: &str) -> String {
        // Extract the primary verb/action from command
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return "unknown".to_string();
        }

        let primary = parts[0];
        // Map common commands to intents
        let intent = match primary {
            "git" => "version control operation",
            "cargo" => "rust build/test operation",
            "npm" | "yarn" | "pnpm" => "node package operation",
            "docker" | "podman" => "container operation",
            "kubectl" | "k9s" => "kubernetes operation",
            "ssh" | "scp" | "rsync" => "remote operation",
            "curl" | "wget" | "http" => "http request",
            "psql" | "mysql" | "redis-cli" => "database operation",
            "vim" | "nvim" | "code" | "nano" => "edit file",
            "cat" | "less" | "more" | "head" | "tail" => "view file",
            "grep" | "rg" | "ag" => "search content",
            "find" | "fd" => "find files",
            "ls" | "exa" | "tree" => "list directory",
            "cd" => "change directory",
            "mkdir" => "create directory",
            "rm" | "rmdir" => "remove file/directory",
            "cp" | "mv" => "copy/move file",
            "chmod" | "chown" => "change permissions",
            "systemctl" | "service" => "service management",
            "journalctl" => "view logs",
            "top" | "htop" | "ps" => "process inspection",
            "kill" | "pkill" => "terminate process",
            _ => "execute command",
        };

        format!("{}: {}", intent, primary)
    }

    fn extract_tools(&self, command: &str) -> Vec<String> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return vec![];
        }
        vec![parts[0].to_string()]
    }

    fn assess_success(&self, output: &[String]) -> Option<bool> {
        if output.is_empty() {
            return None;
        }

        let has_error = output.iter().any(|o| self.error_pattern.is_match(o));
        Some(!has_error)
    }

    fn post_process_steps(&self, mut steps: Vec<WorkflowStep>) -> Vec<WorkflowStep> {
        // Merge consecutive steps with same tool
        let mut merged = Vec::new();
        for step in steps.drain(..) {
            if let Some(last) = merged.last_mut() {
                if last.tools == step.tools && last.tools.len() == 1 {
                    // Merge procedures
                    last.procedure.push_str(&format!("\n\n```bash\n{}\n```", step.procedure.trim_matches('`').trim()));
                    last.context.push_str(&format!("\n---\n{}", step.context));
                    last.extraction_confidence = (last.extraction_confidence + step.extraction_confidence) / 2.0;
                    continue;
                }
            }
            merged.push(step);
        }

        // Re-sequence
        for (i, step) in merged.iter_mut().enumerate() {
            step.sequence = i + 1;
        }

        merged
    }

    fn calculate_confidence(&self, steps: &[WorkflowStep]) -> f32 {
        if steps.is_empty() {
            return 0.0;
        }
        let avg: f32 = steps.iter().map(|s| s.extraction_confidence).sum::<f32>() / steps.len() as f32;
        avg.min(1.0)
    }
}

/// Parser for meeting transcripts
pub struct MeetingTranscriptParser {
    action_item_pattern: Regex,
    decision_pattern: Regex,
    question_pattern: Regex,
    speaker_pattern: Regex,
}

impl Default for MeetingTranscriptParser {
    fn default() -> Self {
        Self::new()
    }
}

impl MeetingTranscriptParser {
    pub fn new() -> Self {
        Self {
            action_item_pattern: Regex::new(r"(?i)(action item|todo|follow up|will do|i'll|assigned to|responsible for)").unwrap(),
            decision_pattern: Regex::new(r"(?i)(decided|agreed|concluded|resolution|we will|we'll proceed)").unwrap(),
            question_pattern: Regex::new(r"(?i)(what if|how about|should we|can we|why|when|who)").unwrap(),
            speaker_pattern: Regex::new(r"^(\w+):\s*(.+)$").unwrap(),
        }
    }

    pub fn parse(&self, transcript: &str, session_id: SessionId) -> Result<WorkflowRecord> {
        let lines: Vec<&str> = transcript.lines().collect();
        let mut steps = Vec::new();
        let mut sequence = 0;

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Check for action items
            if self.action_item_pattern.is_match(trimmed) {
                sequence += 1;
                steps.push(WorkflowStep {
                    step_id: Uuid::new_v4(),
                    sequence,
                    intent: "Action item identified".to_string(),
                    procedure: trimmed.to_string(),
                    context: "Meeting discussion".to_string(),
                    tools: vec!["meeting".to_string()],
                    preconditions: vec![],
                    expected_outcomes: vec!["Task completed".to_string()],
                    actual_outcome: None,
                    duration_seconds: None,
                    success: None,
                    timestamp: Utc::now(),
                    extraction_confidence: 0.9,
                });
            }

            // Check for decisions
            if self.decision_pattern.is_match(trimmed) {
                sequence += 1;
                steps.push(WorkflowStep {
                    step_id: Uuid::new_v4(),
                    sequence,
                    intent: "Decision made".to_string(),
                    procedure: trimmed.to_string(),
                    context: "Meeting discussion".to_string(),
                    tools: vec!["meeting".to_string()],
                    preconditions: vec![],
                    expected_outcomes: vec!["Decision implemented".to_string()],
                    actual_outcome: None,
                    duration_seconds: None,
                    success: None,
                    timestamp: Utc::now(),
                    extraction_confidence: 0.85,
                });
            }
        }

        let confidence = if steps.is_empty() { 0.3 } else { 0.8 };

        Ok(WorkflowRecord {
            workflow_id: Uuid::new_v4(),
            source_type: WorkflowSourceType::MeetingTranscript,
            source_id: format!("meeting_{}", session_id.0),
            title: "Meeting Workflow".to_string(),
            description: format!("Extracted {} action items/decisions from meeting", steps.len()),
            steps,
            tags: vec!["meeting".to_string(), "action_items".to_string()],
            extracted_at: Utc::now(),
            confidence,
            session_id,
        })
    }
}

/// Main workflow extractor that coordinates different parsers
pub struct WorkflowExtractor {
    terminal_parser: TerminalTranscriptParser,
    meeting_parser: MeetingTranscriptParser,
}

impl Default for WorkflowExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkflowExtractor {
    pub fn new() -> Self {
        Self {
            terminal_parser: TerminalTranscriptParser::new(),
            meeting_parser: MeetingTranscriptParser::new(),
        }
    }

    /// Extract workflow from transcript with auto-detection of source type
    pub fn extract(&self, transcript: &str) -> Result<WorkflowRecord> {
        self.extract_with_session(transcript, SessionId::new())
    }

    /// Extract workflow with explicit session ID
    pub fn extract_with_session(&self, transcript: &str, session_id: SessionId) -> Result<WorkflowRecord> {
        let source_type = self.detect_source_type(transcript);

        match source_type {
            WorkflowSourceType::TerminalTranscript => {
                self.terminal_parser.parse(transcript, session_id)
            }
            WorkflowSourceType::MeetingTranscript => {
                self.meeting_parser.parse(transcript, session_id)
            }
            _ => {
                // Fallback to terminal parser
                self.terminal_parser.parse(transcript, session_id)
            }
        }
    }

    /// Extract workflow with explicit source type
    pub fn extract_typed(&self, transcript: &str, source_type: WorkflowSourceType, session_id: SessionId) -> Result<WorkflowRecord> {
        match source_type {
            WorkflowSourceType::TerminalTranscript => self.terminal_parser.parse(transcript, session_id),
            WorkflowSourceType::MeetingTranscript => self.meeting_parser.parse(transcript, session_id),
            _ => self.terminal_parser.parse(transcript, session_id),
        }
    }

    /// Detect source type from transcript content
    fn detect_source_type(&self, transcript: &str) -> WorkflowSourceType {
        let lower = transcript.to_lowercase();

        // Check for meeting indicators
        if lower.contains("action item") || lower.contains("agenda") || lower.contains("attendee") ||
           lower.contains("meeting") || lower.contains("follow up") || lower.contains("decided") {
            return WorkflowSourceType::MeetingTranscript;
        }

        // Check for terminal indicators
        if lower.contains("$ ") || lower.contains("# ") || lower.contains("> ") ||
           lower.contains("cargo ") || lower.contains("git ") || lower.contains("npm ") ||
           lower.contains("docker ") || lower.contains("kubectl ") {
            return WorkflowSourceType::TerminalTranscript;
        }

        // Default to terminal
        WorkflowSourceType::TerminalTranscript
    }

    /// Convert workflow record to skill entries for mch-brain
    pub fn workflow_to_skills(&self, workflow: &WorkflowRecord) -> Vec<SkillEntry> {
        workflow.steps.iter().map(|step| {
            let markdown = format!(
                "# Workflow Step: {}\n\n**Intent:** {}\n\n**Procedure:**\n{}\n\n**Context:**\n{}\n\n**Tools:** {}\n\n**Preconditions:**\n{}\n\n**Expected Outcomes:**\n{}\n\n---\n*Source: {:?}*\n*Session: {}*\n*Extracted: {}*",
                step.intent,
                step.intent,
                step.procedure,
                step.context,
                step.tools.join(", "),
                if step.preconditions.is_empty() { "None".to_string() } else { step.preconditions.join("\n- ") },
                if step.expected_outcomes.is_empty() { "None".to_string() } else { step.expected_outcomes.join("\n- ") },
                workflow.source_type,
                workflow.session_id,
                workflow.extracted_at.format("%Y-%m-%d %H:%M:%S UTC")
            );

            SkillEntry::new(
                step.intent.clone(),
                markdown,
                workflow.session_id,
            )
        }).collect()
    }

    /// Extract and store workflow directly to skill store
    pub async fn extract_and_store(
        &self,
        transcript: &str,
        skill_store: &SkillStore,
        session_id: SessionId,
    ) -> Result<Vec<SkillEntry>> {
        let workflow = self.extract_with_session(transcript, session_id)?;
        let skills = self.workflow_to_skills(&workflow);

        for skill in &skills {
            skill_store.add_skill(skill.clone()).await?;
        }

        Ok(skills)
    }

    /// Extract workflows from multiple transcripts and merge related ones
    pub fn extract_batch(&self, transcripts: &[(String, SessionId)]) -> Result<Vec<WorkflowRecord>> {
        let mut workflows = Vec::new();
        for (transcript, session_id) in transcripts {
            workflows.push(self.extract_with_session(transcript, session_id)?);
        }
        Ok(workflows)
    }
}

/// Transcript parser trait for extensibility
pub trait TranscriptParser {
    fn parse(&self, transcript: &str, session_id: SessionId) -> Result<WorkflowRecord>;
    fn source_type(&self) -> WorkflowSourceType;
}

impl TranscriptParser for TerminalTranscriptParser {
    fn parse(&self, transcript: &str, session_id: SessionId) -> Result<WorkflowRecord> {
        self.parse(transcript, session_id)
    }

    fn source_type(&self) -> WorkflowSourceType {
        WorkflowSourceType::TerminalTranscript
    }
}

impl TranscriptParser for MeetingTranscriptParser {
    fn parse(&self, transcript: &str, session_id: SessionId) -> Result<WorkflowRecord> {
        self.parse(transcript, session_id)
    }

    fn source_type(&self) -> WorkflowSourceType {
        WorkflowSourceType::MeetingTranscript
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mch_brain::{SkillConfig, SkillStore};

    #[tokio::test]
    async fn test_terminal_transcript_parser() {
        let parser = TerminalTranscriptParser::new();
        let transcript = r#"$ git status
On branch main
Your branch is up to date with 'origin/main'.

$ cargo test
   Compiling mch-anatomy v0.0.1
    Finished test [unoptimized + debuginfo] target(s) in 5.23s
     Running tests/personas.rs
test test_aggressive_trader_friction_response ... ok
test test_polite_scheduler_friction_response ... ok

$ git commit -am "Add tests"
[main abc1234] Add tests
 1 file changed, 50 insertions(+)
"#;

        let workflow = parser.parse(transcript, SessionId::new()).unwrap();
        assert_eq!(workflow.source_type, WorkflowSourceType::TerminalTranscript);
        assert!(!workflow.steps.is_empty());
        assert!(workflow.steps.len() >= 3);

        // Check first step
        let step = &workflow.steps[0];
        assert!(step.intent.contains("git"));
        assert!(step.tools.contains(&"git".to_string()));
        assert_eq!(step.success, Some(true));
    }

    #[tokio::test]
    async fn test_terminal_parser_with_errors() {
        let parser = TerminalTranscriptParser::new();
        let transcript = r#"$ cargo build
error[E0308]: mismatched types
 --> src/main.rs:10:5
  |
10 |     let x: i32 = "hello";
  |     ^^^^^^^^^^^^^^^^^^^^^ expected i32, found &str

error: aborting due to previous error
"#;

        let workflow = parser.parse(transcript, SessionId::new()).unwrap();
        assert!(!workflow.steps.is_empty());
        let step = &workflow.steps[0];
        assert_eq!(step.success, Some(false));
        assert!(step.extraction_confidence < 0.8);
    }

    #[tokio::test]
    async fn test_meeting_transcript_parser() {
        let parser = MeetingTranscriptParser::new();
        let transcript = r#"Alice: Let's start the sprint planning
Bob: I'll work on the authentication module - that's my action item
Carol: We decided to use JWT for tokens
Alice: Follow up on the database migration next week
"#;

        let workflow = parser.parse(transcript, SessionId::new()).unwrap();
        assert_eq!(workflow.source_type, WorkflowSourceType::MeetingTranscript);
        assert!(!workflow.steps.is_empty());
        assert!(workflow.steps.iter().any(|s| s.intent == "Action item identified"));
        assert!(workflow.steps.iter().any(|s| s.intent == "Decision made"));
    }

    #[tokio::test]
    async fn test_workflow_extractor_auto_detect() {
        let extractor = WorkflowExtractor::new();

        // Terminal transcript
        let terminal = r#"$ git status
On branch main
$ cargo test
test ok
"#;
        let workflow = extractor.extract(terminal).unwrap();
        assert_eq!(workflow.source_type, WorkflowSourceType::TerminalTranscript);

        // Meeting transcript
        let meeting = r#"Alice: Action item - Bob will fix the bug
Bob: Agreed, we decided to prioritize this
"#;
        let workflow = extractor.extract(meeting).unwrap();
        assert_eq!(workflow.source_type, WorkflowSourceType::MeetingTranscript);
    }

    #[tokio::test]
    async fn test_workflow_to_skills() {
        let extractor = WorkflowExtractor::new();
        let transcript = r#"$ git status
On branch main
$ cargo build
Compiled successfully
"#;
        let workflow = extractor.extract(transcript).unwrap();
        let skills = extractor.workflow_to_skills(&workflow);

        assert!(!skills.is_empty());
        assert_eq!(skills.len(), workflow.steps.len());

        for skill in &skills {
            assert!(!skill.intent.is_empty());
            assert!(!skill.doc_markdown.is_empty());
            assert!(skill.doc_markdown.contains("Procedure"));
        }
    }

    #[tokio::test]
    async fn test_extract_and_store() {
        let extractor = WorkflowExtractor::new();
        let skill_store = SkillStore::new(SkillConfig::default());
        let session_id = SessionId::new();

        let transcript = r#"$ git status
On branch main
$ cargo test
test ok
"#;

        let skills = extractor.extract_and_store(transcript, &skill_store, session_id).await.unwrap();
        assert!(!skills.is_empty());

        // Verify skills were stored
        let stored = skill_store.list_skills().await;
        assert_eq!(stored.len(), skills.len());
    }

    #[tokio::test]
    async fn test_batch_extraction() {
        let extractor = WorkflowExtractor::new();
        let transcripts = vec![
            ("$ git status\nOn branch main".to_string(), SessionId::new()),
            ("$ cargo build\nCompiled".to_string(), SessionId::new()),
        ];

        let workflows = extractor.extract_batch(&transcripts).unwrap();
        assert_eq!(workflows.len(), 2);
    }

    #[test]
    fn test_workflow_step_serialization() {
        let step = WorkflowStep {
            step_id: Uuid::new_v4(),
            sequence: 1,
            intent: "Test intent".to_string(),
            procedure: "echo hello".to_string(),
            context: "test context".to_string(),
            tools: vec!["echo".to_string()],
            preconditions: vec![],
            expected_outcomes: vec!["hello printed".to_string()],
            actual_outcome: Some("hello".to_string()),
            duration_seconds: Some(1),
            success: Some(true),
            timestamp: Utc::now(),
            extraction_confidence: 0.9,
        };

        let json = serde_json::to_string(&step).unwrap();
        let deserialized: WorkflowStep = serde_json::from_str(&json).unwrap();
        assert_eq!(step.intent, deserialized.intent);
        assert_eq!(step.tools, deserialized.tools);
    }

    #[test]
    fn test_workflow_record_serialization() {
        let record = WorkflowRecord {
            workflow_id: Uuid::new_v4(),
            source_type: WorkflowSourceType::TerminalTranscript,
            source_id: "test".to_string(),
            title: "Test".to_string(),
            description: "Test workflow".to_string(),
            steps: vec![],
            tags: vec!["test".to_string()],
            extracted_at: Utc::now(),
            confidence: 0.8,
            session_id: SessionId::new(),
        };

        let json = serde_json::to_string(&record).unwrap();
        let deserialized: WorkflowRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record.title, deserialized.title);
        assert_eq!(record.source_type, deserialized.source_type);
    }
}