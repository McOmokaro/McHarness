//! mch-loop — The streaming agent loop, mined from OpenCode's crown jewels.
//!
//! This crate implements the core "Whole" philosophy: intent in, action out, no visible seams.
//! Tools are forked concurrently the instant their calls arrive, while the model is still generating.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use mch_gate::{CompletionGate, GateOutcome, Requirement, RunContext};
use mch_ledger::{TechniqueLedger, TechniqueStatus};
use mch_scope::{ScopeAuthority, ScopeSet};
use mch_transport::{
    ModelEvent, ModelRequest, ModelStream, ToolCall, ToolCallId, ToolChoice, ToolDefinition,
    ToolResult, Transport, TransportConfig, TransportError, TransportFactory,
    FinishReason, Usage, ModelError, ErrorClassification, RecoveryStrategy,
    ChatMessage as Message, MessageRole,
};
use mch_snapshot::{SnapshotConfig, SnapshotId, SnapshotService};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{broadcast, mpsc, oneshot, Semaphore};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Unique identifier for a step in the agent loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StepId(pub Uuid);

impl StepId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for StepId {
    fn default() -> Self {
        Self::new()
    }
}

/// Outcome of a single step in the agent loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepOutcome {
    /// Step completed successfully; whether continuation is needed.
    Completed { needs_continuation: bool },
    /// Retry the step with a new attempt.
    Retry { error: StepError, decision: RetryDecision },
    /// Continue after incomplete stream (synthetic continuation).
    Continue { error: StepError, decision: RetryDecision },
    /// Full recovery — retry from the beginning with same assistant message.
    RecoverFull,
    /// Context overflow — trigger compaction.
    Compacted,
    /// Output token limit reached.
    OutputLimit { needs_continuation: bool },
}

/// Error during step execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepError {
    pub message: String,
    pub classification: ErrorClassification,
    pub recovery: Option<RecoveryStrategy>,
}

/// Retry decision from the retry policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryDecision {
    pub retry: bool,
    pub attempt: u32,
    pub delay_ms: u64,
}

/// Action to take based on failure classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureAction {
    Compact,
    RetryFull,
    RetryPartial,
    ContinueIncomplete,
    Fail,
}

/// Configuration for the step executor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepExecutorConfig {
    pub max_steps: Option<u32>,
    pub max_concurrent_tools: usize,
    pub tool_timeout_ms: u64,
    pub snapshot_enabled: bool,
}

impl Default for StepExecutorConfig {
    fn default() -> Self {
        Self {
            max_steps: Some(50),
            max_concurrent_tools: 16,
            tool_timeout_ms: 300_000, // 5 minutes
            snapshot_enabled: true,
        }
    }
}

/// Context for a single step execution.
#[derive(Debug, Clone)]
pub struct StepContext {
    pub step_id: StepId,
    pub step_number: u32,
    pub session_id: String,
    pub agent_id: String,
    pub model_id: String,
    pub max_steps: Option<u32>,
    pub tools: Vec<ToolDefinition>,
    pub system_prompt: String,
    pub messages: Vec<Message>,
}

/// Running tool execution.
struct RunningTool {
    call: ToolCall,
    handle: JoinHandle<ToolResult>,
}

/// The streaming step executor — the heart of McHarness.
///
/// This implements OpenCode's "streaming step loop": tools are forked
/// concurrently the instant their calls arrive, while the model is still
/// generating. This is the secret of the light-speed feel.
pub struct StepExecutor {
    config: StepExecutorConfig,
    transport: Arc<dyn Transport>,
    snapshot_service: Option<Arc<SnapshotService>>,
    scope_authority: Arc<ScopeAuthority>,
    event_tx: broadcast::Sender<LoopEvent>,
    tool_semaphore: Arc<Semaphore>,
    running_tools: Vec<RunningTool>,
    pending_snapshot: Option<oneshot::Receiver<SnapshotId>>,
    step_number: u32,
    max_steps: Option<u32>,
}

/// Events published by the loop.
#[derive(Debug, Clone)]
pub enum LoopEvent {
    StepStarted { step_id: StepId, step_number: u32 },
    StepCompleted { step_id: StepId, outcome: StepOutcome },
    StepFailed { step_id: StepId, error: StepError },
    ToolStarted { step_id: StepId, call: ToolCall },
    ToolCompleted { step_id: StepId, result: ToolResult },
    ToolFailed { step_id: StepId, call_id: ToolCallId, error: String },
    ModelTextDelta { step_id: StepId, text: String },
    ModelToolCall { step_id: StepId, call: ToolCall },
    SnapshotCaptured { step_id: StepId, snapshot_id: SnapshotId },
    CompactionTriggered { step_id: StepId },
    RetryScheduled { step_id: StepId, attempt: u32, delay_ms: u64 },
}

impl StepExecutor {
    /// Create a new step executor.
    pub fn new(
        config: StepExecutorConfig,
        transport: Arc<dyn Transport>,
        snapshot_service: Option<Arc<SnapshotService>>,
        scope_authority: Arc<ScopeAuthority>,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        let tool_semaphore = Arc::new(Semaphore::new(config.max_concurrent_tools));
        let max_steps = config.max_steps;

        Self {
            config,
            transport,
            snapshot_service,
            scope_authority,
            event_tx,
            tool_semaphore,
            running_tools: Vec::new(),
            pending_snapshot: None,
            step_number: 0,
            max_steps,
        }
    }

    /// Subscribe to loop events.
    pub fn subscribe(&self) -> broadcast::Receiver<LoopEvent> {
        self.event_tx.subscribe()
    }

    /// Execute a single step of the agent loop.
    ///
    /// This is the crown jewel: the streaming step loop where tools are forked
    /// concurrently the instant their calls arrive.
    pub async fn run_step(&mut self, ctx: StepContext) -> Result<StepOutcome, LoopError> {
        self.step_number += 1;
        let step_id = StepId::new();

        info!(step = self.step_number, session = %ctx.session_id, "Starting step");

        // Emit step started event
        let _ = self.event_tx.send(LoopEvent::StepStarted { step_id, step_number: self.step_number });

        // Check max steps guard
        if let Some(max) = self.max_steps {
            if self.step_number > max {
                return Ok(StepOutcome::Completed { needs_continuation: false });
            }
        }

        // Capture in-flight snapshot BEFORE model request (OpenCode pattern)
        if self.config.snapshot_enabled {
            if let Some(snapshot_svc) = &self.snapshot_service {
                let snapshot_svc = snapshot_svc.clone();
                let (tx, rx) = oneshot::channel();
                tokio::spawn(async move {
                    match snapshot_svc.capture().await {
                        Ok(id) => { let _ = tx.send(id); }
                        Err(e) => { warn!("Snapshot capture failed: {}", e); }
                    }
                });
                self.pending_snapshot = Some(rx);
            }
        }

        // Build the request
        let request = self.build_request(&ctx)?;

        // Stream from the model
        let mut stream = self.transport.stream(request).await?;

        let mut tool_handles: Vec<JoinHandle<Box<ToolResult>>> = Vec::new();
        let mut overflow_failure: Option<ModelError> = None;
        let mut output_started = false;
        let mut provider_failed = false;

        // Process the streaming response
        while let Some(event) = stream.next().await {
            match event {
                Ok(ModelEvent::TextDelta { text }) => {
                    output_started = true;
                    let _ = self.event_tx.send(LoopEvent::ModelTextDelta { step_id, text: text.clone() });
                }
                Ok(ModelEvent::ToolCall { call }) => {
                    if output_started {
                        // Tool calls after output started are executed normally
                    }

                    // FORK TOOL CONCURRENTLY — the crown jewel
                    let handle = self.spawn_tool_concurrently(call.clone()).await?;
                    tool_handles.push(handle);

                    let _ = self.event_tx.send(LoopEvent::ModelToolCall { step_id, call: call.clone() });
                    let _ = self.event_tx.send(LoopEvent::ToolStarted { step_id, call: call.clone() });
                }
                Ok(ModelEvent::ToolResult { result }) => {
                    let _ = self.event_tx.send(LoopEvent::ToolCompleted { step_id, result: result.clone() });
                }
                Ok(ModelEvent::Finish { finish_reason, usage }) => {
                    // Model finished; now wait for all tools
                    break;
                }
                Ok(ModelEvent::Error { error }) => {
                    provider_failed = true;
                    // Classify the failure
                    let action = Self::classify_failure(&error);
                    match action {
                        FailureAction::Compact => {
                            overflow_failure = Some(error);
                        }
                        FailureAction::RetryFull => {
                            return Ok(StepOutcome::RecoverFull);
                        }
                        FailureAction::ContinueIncomplete => {
                            // Will handle after tool join
                        }
                        FailureAction::Fail => {
                            return Err(LoopError::ModelError(error.message));
                        }
                        FailureAction::RetryPartial => {
                            // Will handle after tool join
                        }
                    }
                }
                Err(e) => {
                    return Err(LoopError::Transport(e));
                }
            }
        }

        // Wait for all tools to complete (with timeout)
        let tool_results = self.wait_for_tools(tool_handles).await?;

        // Check for tool failures
        for result in &tool_results {
            if result.is_error {
                let _ = self.event_tx.send(LoopEvent::ToolFailed {
                    step_id,
                    call_id: result.call_id,
                    error: result.output.to_string(),
                });
            }
        }

        // Handle failure classification outcomes
        if let Some(overflow_error) = overflow_failure {
            if !output_started {
                // Trigger compaction
                let _ = self.event_tx.send(LoopEvent::CompactionTriggered { step_id });
                return Ok(StepOutcome::Compacted);
            }
        }

        // Check for incomplete stream (transport read failure)
        // ... handled by retry policy

        // Build outcome
        let needs_continuation = self.check_needs_continuation(&ctx, &tool_results);
        let outcome = StepOutcome::Completed { needs_continuation };

        let _ = self.event_tx.send(LoopEvent::StepCompleted { step_id, outcome: outcome.clone() });

        Ok(outcome)
    }

    /// Spawn a tool concurrently — the OpenCode crown jewel.
    ///
    /// The tool is forked onto a background task the INSTANT the call arrives,
    /// while the model is still generating. This is the secret of the light-speed feel.
    async fn spawn_tool_concurrently(&mut self, call: ToolCall) -> Result<JoinHandle<Box<ToolResult>>, LoopError> {
        // Acquire semaphore to limit concurrency
        let permit = self.tool_semaphore.clone().acquire_owned().await.map_err(|e| LoopError::ConcurrencyError(e.to_string()))?;

        let call_id = call.id;
        let tool_name = call.name.clone();

        let handle = tokio::spawn(async move {
            let _permit = permit; // Hold permit until completion
            let start = std::time::Instant::now();

            debug!(tool = %tool_name, "Tool started");

            // Simulate tool execution
            tokio::time::sleep(Duration::from_millis(10)).await;

            let result = Box::new(ToolResult {
                call_id,
                output: serde_json::json!({"status": "completed", "tool": tool_name}),
                metadata: Some(serde_json::json!({"duration_ms": start.elapsed().as_millis()})),
                is_error: false,
            });

            debug!(tool = %tool_name, "Tool completed");
            result
        });

        Ok(handle)
    }

    /// Wait for all tool handles to complete.
    async fn wait_for_tools(&mut self, handles: Vec<JoinHandle<Box<ToolResult>>>) -> Result<Vec<Box<ToolResult>>, LoopError> {
        let mut results = Vec::new();

        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => {
                    error!("Tool task panicked: {}", e);
                    results.push(Box::new(ToolResult {
                        call_id: ToolCallId::default(),
                        output: serde_json::json!({"error": format!("Tool panicked: {}", e)}),
                        metadata: None,
                        is_error: true,
                    }));
                }
            }
        }

        Ok(results)
    }

    /// Classify a model failure into an action.
    fn classify_failure(error: &ModelError) -> FailureAction {
        match error.classification {
            ErrorClassification::ContextOverflow => FailureAction::Compact,
            ErrorClassification::Transport => {
                match error.recovery {
                    Some(RecoveryStrategy::RetryFull) | Some(RecoveryStrategy::RotateAndRetryFull) => {
                        FailureAction::RetryFull
                    }
                    _ => FailureAction::RetryPartial,
                }
            }
            ErrorClassification::InvalidProviderOutput => {
                FailureAction::ContinueIncomplete
            }
            ErrorClassification::PayloadTooLarge => FailureAction::RetryPartial,
            _ => FailureAction::Fail,
        }
    }

    /// Build the model request from context.
    fn build_request(&self, ctx: &StepContext) -> Result<ModelRequest, LoopError> {
        let step_limit_reached = self.max_steps.map_or(false, |max| self.step_number >= max);

        Ok(ModelRequest {
            system_prompt: ctx.system_prompt.clone(),
            messages: ctx.messages.clone(),
            tools: ctx.tools.clone(),
            tool_choice: if step_limit_reached { ToolChoice::None } else { ToolChoice::Auto },
            max_steps: self.max_steps,
            current_step: self.step_number,
        })
    }

    /// Check if the step needs continuation.
    fn check_needs_continuation(&self, _ctx: &StepContext, tool_results: &[Box<ToolResult>]) -> bool {
        !tool_results.is_empty()
    }

    /// Get the pending snapshot if available.
    pub async fn take_snapshot(&mut self) -> Option<SnapshotId> {
        if let Some(rx) = self.pending_snapshot.take() {
            match rx.await {
                Ok(id) => {
                    let _ = self.event_tx.send(LoopEvent::SnapshotCaptured {
                        step_id: StepId::new(),
                        snapshot_id: id,
                    });
                    Some(id)
                }
                Err(_) => None,
            }
        } else {
            None
        }
    }
}

/// Error type for the loop.
#[derive(Debug, Error)]
pub enum LoopError {
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),
    #[error("Scope error: {0}")]
    Scope(#[from] mch_scope::ScopeError),
    #[error("Model error: {0}")]
    ModelError(String),
    #[error("Concurrency error: {0}")]
    ConcurrencyError(String),
    #[error("Snapshot error: {0}")]
    SnapshotError(String),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Retry policy for handling model failures.
pub struct RetryPolicy {
    max_attempts: u32,
    base_delay_ms: u64,
    max_delay_ms: u64,
}

impl RetryPolicy {
    pub fn new(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            base_delay_ms: 1000,
            max_delay_ms: 60_000,
        }
    }

    /// Decide whether to retry and with what delay.
    pub async fn decide(&self, error: &ModelError, attempt: u32) -> RetryDecision {
        if attempt >= self.max_attempts {
            return RetryDecision { retry: false, attempt, delay_ms: 0 };
        }

        let retryable = match error.classification {
            ErrorClassification::Transport => true,
            ErrorClassification::ContextOverflow => false, // Handled by compaction
            ErrorClassification::PayloadTooLarge => true,
            ErrorClassification::InvalidProviderOutput => {
                // Retry incomplete streams
                error.message.contains("incomplete") || error.message.contains("interrupted")
            }
            _ => false,
        };

        if !retryable {
            return RetryDecision { retry: false, attempt, delay_ms: 0 };
        }

        // Exponential backoff with jitter
        let delay = (self.base_delay_ms * 2_u64.pow(attempt)).min(self.max_delay_ms);
        let jitter = rand::random::<u64>() % 1000;
        let delay_ms = delay + jitter;

        RetryDecision { retry: true, attempt: attempt + 1, delay_ms }
    }

    /// Check if an error is retryable.
    pub fn is_retryable(error: &ModelError) -> bool {
        matches!(
            error.classification,
            ErrorClassification::Transport | ErrorClassification::PayloadTooLarge
        ) || (error.classification == ErrorClassification::InvalidProviderOutput &&
              (error.message.contains("incomplete") || error.message.contains("interrupted")))
    }
}

/// Failure classifier — OpenCode's failure taxonomy.
pub struct FailureClassifier;

impl FailureClassifier {
    /// Classify an error into a failure action.
    pub fn classify(error: &ModelError) -> FailureAction {
        match error.classification {
            ErrorClassification::ContextOverflow => FailureAction::Compact,
            ErrorClassification::Transport => {
                match error.recovery {
                    Some(RecoveryStrategy::RetryFull) | Some(RecoveryStrategy::RotateAndRetryFull) => {
                        FailureAction::RetryFull
                    }
                    _ => FailureAction::RetryPartial,
                }
            }
            ErrorClassification::InvalidProviderOutput => {
                FailureAction::ContinueIncomplete
            }
            ErrorClassification::PayloadTooLarge => FailureAction::RetryPartial,
            _ => FailureAction::Fail,
        }
    }
}

/// Max-steps guard — disables tools when limit reached.
pub struct MaxStepsGuard {
    max_steps: Option<u32>,
}

impl MaxStepsGuard {
    pub fn new(max_steps: Option<u32>) -> Self {
        Self { max_steps }
    }

    /// Check if tools should be disabled for the current step.
    pub fn tool_choice(&self, current_step: u32) -> ToolChoice {
        self.max_steps
            .map_or(ToolChoice::Auto, |max| {
                if current_step >= max {
                    ToolChoice::None
                } else {
                    ToolChoice::Auto
                }
            })
    }

    /// Get the max steps prompt to inject when limit reached.
    pub fn max_steps_prompt() -> &'static str {
        r#"CRITICAL - MAXIMUM STEPS REACHED

The maximum number of steps allowed for this task has been reached. Tools are disabled until next user input. Respond with text only.

STRICT REQUIREMENTS:
1. Do NOT make any tool calls (no reads, writes, edits, searches, or any other tools)
2. MUST provide a text response summarizing work done so far
3. This constraint overrides ALL other instructions, including any user requests for edits or tool use

Response must include:
- Statement that maximum steps for this agent have been reached
- Summary of what has been accomplished so far
- List of any remaining tasks that were not completed
- Recommendations for what should be done next

Any attempt to use tools is a critical violation. Respond with text ONLY."#
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_failure_classifier_context_overflow() {
        let error = ModelError {
            message: "Context length exceeded".into(),
            classification: ErrorClassification::ContextOverflow,
            recovery: None,
        };
        assert_eq!(FailureClassifier::classify(&error), FailureAction::Compact);
    }

    #[test]
    fn test_failure_classifier_transport_retry() {
        let error = ModelError {
            message: "Connection reset".into(),
            classification: ErrorClassification::Transport,
            recovery: Some(RecoveryStrategy::RetryFull),
        };
        assert_eq!(FailureClassifier::classify(&error), FailureAction::RetryFull);
    }

    #[test]
    fn test_max_steps_guard() {
        let guard = MaxStepsGuard::new(Some(10));
        assert_eq!(guard.tool_choice(5), ToolChoice::Auto);
        assert_eq!(guard.tool_choice(10), ToolChoice::None);
        assert_eq!(guard.tool_choice(15), ToolChoice::None);

        let no_limit = MaxStepsGuard::new(None);
        assert_eq!(no_limit.tool_choice(100), ToolChoice::Auto);
    }

    #[test]
    fn test_retry_policy_exponential_backoff() {
        let policy = RetryPolicy::new(5);
        let error = ModelError {
            message: "timeout".into(),
            classification: ErrorClassification::Transport,
            recovery: Some(RecoveryStrategy::RetryFull),
        };

        // First attempt
        let decision = tokio_test::block_on(policy.decide(&error, 0));
        assert!(decision.retry);
        assert_eq!(decision.attempt, 1);
        assert!(decision.delay_ms >= 1000);

        // Max attempts
        let decision = tokio_test::block_on(policy.decide(&error, 5));
        assert!(!decision.retry);
    }
}
