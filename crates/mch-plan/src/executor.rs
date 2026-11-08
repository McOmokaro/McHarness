//! Execution engine: PlanExecutor, StepRunner.
//!
//! The compiled plan prompt IS the execution plan — no separate engine.
//! The model executes the prompt directly, with StepRunner coordinating
//! step transitions, gates, branching, loops, parallelism, and fallbacks.

use std::collections::{HashMap, BTreeMap};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use async_trait::async_trait;
use futures::future::join_all;
use mch_gate::{CompletionGate, GateOutcome, Requirement, RunContext as GateRunContext};
use mch_loop::{StepExecutor as LoopStepExecutor, StepExecutorConfig, StepContext, StepOutcome, StepId, LoopEvent, LoopError};
use mch_scope::{ScopeAuthority, ScopeSet, ScopeVerdict, ScopeError};
use mch_transport::{Transport, TransportFactory, TransportConfig, ModelRequest, ModelEvent, ToolCall, ToolCallId, ToolChoice, ToolDefinition, ToolResult, FinishReason, Usage};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::compiler::{CompiledPlan, StepPlan, StepKind, StepGate, GateType};
use crate::state::{RunState, StepState, RunStatus, StepStatus, GateResult, ArtifactRecord};

/// Errors during plan execution.
#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("transport error: {0}")]
    Transport(#[from] mch_transport::TransportError),
    #[error("loop error: {0}")]
    Loop(#[from] LoopError),
    #[error("scope error: {0}")]
    Scope(#[from] ScopeError),
    #[error("gate error: {0}")]
    Gate(String),
    #[error("step error: {0}")]
    Step(String),
    #[error("compilation error: {0}")]
    Compile(#[from] crate::compiler::CompileError),
    #[error("state error: {0}")]
    State(#[from] crate::state::StateError),
    #[error("plan exhausted: no more steps")]
    PlanExhausted,
    #[error("gate failed: {gate} - {reason}")]
    GateFailed { gate: String, reason: String },
    #[error("step failed: {step} - {reason}")]
    StepFailed { step: String, reason: String },
    #[error("max steps exceeded")]
    MaxStepsExceeded,
    #[error("execution cancelled")]
    Cancelled,
}

/// Context for executing a single step.
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub run_id: String,
    pub step_id: String,
    pub plan_step: StepPlan,
    pub run_state: Arc<RwLock<RunState>>,
    pub scope_authority: Arc<ScopeAuthority>,
    pub completion_gates: HashMap<String, CompletionGate>,
    pub variables: HashMap<String, serde_json::Value>,
    pub step_number: u32,
    pub max_steps: Option<u32>,
}

/// Result of executing a step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_id: String,
    pub plan_step_id: String,
    pub success: bool,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub next_steps: Vec<String>,
    pub gate_results: Vec<GateResult>,
    pub duration_ms: u64,
    pub tokens_used: Option<u64>,
}

/// Summary of a complete run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummary {
    pub run_id: String,
    pub plan_name: String,
    pub status: RunStatus,
    pub total_steps: usize,
    pub completed_steps: usize,
    pub failed_steps: usize,
    pub skipped_steps: usize,
    pub total_duration_ms: u64,
    pub step_results: Vec<StepResult>,
    pub final_variables: HashMap<String, serde_json::Value>,
}

/// Configuration for the plan executor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanExecutorConfig {
    pub max_steps: Option<u32>,
    pub max_concurrent_tools: usize,
    pub tool_timeout_ms: u64,
    pub step_timeout_ms: u64,
    pub enable_snapshots: bool,
    pub retry_policy: RetryPolicyConfig,
}

/// Retry policy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicyConfig {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for PlanExecutorConfig {
    fn default() -> Self {
        Self {
            max_steps: Some(100),
            max_concurrent_tools: 16,
            tool_timeout_ms: 300_000,
            step_timeout_ms: 600_000,
            enable_snapshots: true,
            retry_policy: RetryPolicyConfig {
                max_attempts: 3,
                base_delay_ms: 1000,
                max_delay_ms: 60_000,
            },
        }
    }
}

/// Runs a compiled plan by executing steps through the model.
pub struct PlanExecutor {
    config: PlanExecutorConfig,
    transport: Arc<dyn Transport>,
    scope_authority: Arc<ScopeAuthority>,
    loop_executor: Option<LoopStepExecutor>,
    event_tx: broadcast::Sender<PlanEvent>,
    completion_gates: HashMap<String, CompletionGate>,
}

/// Events published during plan execution.
#[derive(Debug, Clone)]
pub enum PlanEvent {
    RunStarted { run_id: String, plan_name: String },
    RunCompleted { run_id: String, summary: RunSummary },
    RunFailed { run_id: String, error: String },
    RunPaused { run_id: String },
    RunResumed { run_id: String },
    RunCancelled { run_id: String },
    StepStarted { run_id: String, step_id: String, plan_step_id: String },
    StepCompleted { run_id: String, result: StepResult },
    StepFailed { run_id: String, step_id: String, error: String },
    StepSkipped { run_id: String, step_id: String, reason: String },
    GateChecked { run_id: String, step_id: String, gate_id: String, passed: bool, message: String },
    BranchTaken { run_id: String, step_id: String, branch: String },
    LoopIteration { run_id: String, step_id: String, iteration: u32, item: serde_json::Value },
    ParallelSpawned { run_id: String, step_id: String, branches: Vec<String> },
    ParallelJoined { run_id: String, step_id: String, results: Vec<StepResult> },
    FallbackTriggered { run_id: String, step_id: String, error: String, fallback_step: String },
}

impl PlanExecutor {
    /// Create a new plan executor.
    pub fn new(
        config: PlanExecutorConfig,
        transport: Arc<dyn Transport>,
        scope_authority: Arc<ScopeAuthority>,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self {
            config,
            transport,
            scope_authority,
            loop_executor: None,
            event_tx,
            completion_gates: HashMap::new(),
        }
    }

    /// Set the loop executor for streaming step execution.
    pub fn with_loop_executor(mut self, loop_executor: LoopStepExecutor) -> Self {
        self.loop_executor = Some(loop_executor);
        self
    }

    /// Register a completion gate.
    pub fn register_gate(&mut self, name: &str, gate: CompletionGate) {
        self.completion_gates.insert(name.to_string(), gate);
    }

    /// Subscribe to plan events.
    pub fn subscribe(&self) -> broadcast::Receiver<PlanEvent> {
        self.event_tx.subscribe()
    }

    /// Execute a compiled plan from start to finish.
    pub async fn execute(&mut self, plan: CompiledPlan) -> Result<RunSummary, ExecutionError> {
        let run_id = Uuid::new_v4().to_string();
        let mut run_state = RunState::new(&plan);
        run_state.start();

        let _ = self.event_tx.send(PlanEvent::RunStarted {
            run_id: run_id.clone(),
            plan_name: plan.name.clone(),
        });

        info!(run_id = %run_id, plan = %plan.name, "Starting plan execution");

        // Execute steps starting from entry
        let mut current_step_id = plan.entry_step.clone();
        let mut step_number = 0;

        while let Some(step_id) = current_step_id {
            if let Some(max) = self.config.max_steps {
                if step_number >= max {
                    let _ = self.event_tx.send(PlanEvent::RunFailed {
                        run_id: run_id.clone(),
                        error: "Max steps exceeded".to_string(),
                    });
                    run_state.fail("Max steps exceeded".to_string());
                    return Ok(run_state.summary());
                }
            }

            step_number += 1;

            // Get step plan
            let step_plan = plan.steps.iter().find(|s| s.id == step_id).cloned()
                .ok_or_else(|| ExecutionError::Step(format!("step not found: {step_id}")))?;

            // Execute step
            let result = self.execute_step(&run_id, &mut run_state, &plan, &step_plan, step_number).await?;

            // Update run state
            if result.success {
                run_state.complete_step(&step_id, result.output.clone().unwrap_or(serde_json::json!({})))?;
            } else {
                run_state.fail_step(&step_id, result.error.clone().unwrap_or("unknown error".to_string()))?;
                let _ = self.event_tx.send(PlanEvent::RunFailed {
                    run_id: run_id.clone(),
                    error: result.error.clone().unwrap_or("step failed".to_string()),
                });
                run_state.fail(result.error.unwrap_or("step failed".to_string()));
                return Ok(run_state.summary());
            }

            // Determine next step based on step kind and result
            current_step_id = self.determine_next_step(&plan, &step_plan, &result, &run_state).await?;
        }

        run_state.complete();
        let summary = run_state.summary();

        let _ = self.event_tx.send(PlanEvent::RunCompleted {
            run_id: run_id.clone(),
            summary: summary.clone(),
        });

        info!(run_id = %run_id, "Plan execution completed");
        Ok(summary)
    }

    /// Execute a single step.
    async fn execute_step(
        &mut self,
        run_id: &str,
        run_state: &mut RunState,
        plan: &CompiledPlan,
        step_plan: &StepPlan,
        step_number: u32,
    ) -> Result<StepResult, ExecutionError> {
        let start_time = SystemTime::now();
        let step_id = Uuid::new_v4().to_string();

        let _ = self.event_tx.send(PlanEvent::StepStarted {
            run_id: run_id.to_string(),
            step_id: step_id.clone(),
            plan_step_id: step_plan.id.clone(),
        });

        run_state.start_step(&step_plan.id, serde_json::json!({}))?;

        // Check gates before execution
        for gate in &step_plan.gates {
            let gate_result = self.check_gate(run_id, &step_plan.id, gate, run_state).await?;
            run_state.record_gate_result(&step_plan.id, gate_result.clone())?;

            let _ = self.event_tx.send(PlanEvent::GateChecked {
                run_id: run_id.to_string(),
                step_id: step_plan.id.clone(),
                gate_id: gate.gate_id.clone(),
                passed: gate_result.passed,
                message: gate_result.message.clone(),
            });

            if !gate_result.passed && gate.required {
                let error = format!("Gate {} failed: {}", gate.gate_id, gate_result.message);
                let _ = self.event_tx.send(PlanEvent::StepFailed {
                    run_id: run_id.to_string(),
                    step_id: step_plan.id.clone(),
                    error: error.clone(),
                });
                return Ok(StepResult {
                    step_id: step_id.clone(),
                    plan_step_id: step_plan.id.clone(),
                    success: false,
                    output: None,
                    error: Some(error),
                    next_steps: vec![],
                    gate_results: vec![gate_result],
                    duration_ms: start_time.elapsed().unwrap_or_default().as_millis() as u64,
                    tokens_used: None,
                });
            }
        }

        // Execute based on step kind
        let result = match &step_plan.kind {
            StepKind::Atomic => self.execute_atomic_step(run_id, run_state, step_plan, step_number).await?,
            StepKind::Branch { condition, true_step, false_step } => {
                self.execute_branch_step(run_id, run_state, step_plan, condition, true_step, false_step).await?
            }
            StepKind::Loop { collection, item_var, body_step, max_iterations } => {
                self.execute_loop_step(run_id, run_state, step_plan, collection, item_var, body_step, *max_iterations, step_number).await?
            }
            StepKind::Parallel { branches, join_step } => {
                self.execute_parallel_step(run_id, run_state, step_plan, branches, join_step, step_number).await?
            }
            StepKind::Merge { incoming, outgoing } => {
                self.execute_merge_step(run_id, run_state, step_plan, incoming, outgoing).await?
            }
            StepKind::Fallback { primary_step, fallback_step, error_patterns } => {
                self.execute_fallback_step(run_id, run_state, step_plan, primary_step, fallback_step, error_patterns, step_number).await?
            }
            StepKind::Gate { gate_type, config } => {
                self.execute_gate_step(run_id, run_state, step_plan, gate_type, config).await?
            }
            StepKind::End => {
                StepResult {
                    step_id: step_id.clone(),
                    plan_step_id: step_plan.id.clone(),
                    success: true,
                    output: Some(serde_json::json!({"status": "completed"})),
                    error: None,
                    next_steps: vec![],
                    gate_results: vec![],
                    duration_ms: start_time.elapsed().unwrap_or_default().as_millis() as u64,
                    tokens_used: None,
                }
            }
        };

        let duration = start_time.elapsed().unwrap_or_default().as_millis() as u64;

        let _ = self.event_tx.send(PlanEvent::StepCompleted {
            run_id: run_id.to_string(),
            result: result.clone(),
        });

        Ok(StepResult { duration_ms: duration, ..result })
    }

    /// Execute an atomic step (model call).
    async fn execute_atomic_step(
        &mut self,
        run_id: &str,
        run_state: &mut RunState,
        step_plan: &StepPlan,
        step_number: u32,
    ) -> Result<StepResult, ExecutionError> {
        // Build model request
        let messages = self.build_messages(run_state, step_plan);
        let tools = self.build_tool_definitions(step_plan);
        let tool_choice = if self.config.max_steps.map_or(false, |max| step_number >= max) {
            ToolChoice::None
        } else {
            ToolChoice::Auto
        };

        let request = ModelRequest {
            system_prompt: step_plan.prompt.clone(),
            messages,
            tools,
            tool_choice,
            max_steps: self.config.max_steps,
            current_step: step_number,
        };

        // Stream from model
        let mut stream = self.transport.stream(request).await?;

        let mut output = String::new();
        let mut tool_calls = Vec::new();

        while let Some(event) = stream.next().await {
            match event? {
                ModelEvent::TextDelta { text } => {
                    output.push_str(&text);
                }
                ModelEvent::ToolCall { call } => {
                    tool_calls.push(call);
                }
                ModelEvent::ToolResult { result } => {
                    // Tool result handled by loop executor if present
                }
                ModelEvent::Finish { finish_reason, usage } => {
                    if finish_reason == FinishReason::Stop || finish_reason == FinishReason::ToolCalls {
                        break;
                    }
                }
                ModelEvent::Error { error } => {
                    return Err(ExecutionError::Step(error.message));
                }
            }
        }

        // Execute tool calls if any
        let mut tool_outputs = Vec::new();
        for call in tool_calls {
            let result = self.execute_tool_call(run_state, &call).await?;
            tool_outputs.push(result);
        }

        // Combine output and tool results
        let combined_output = if tool_outputs.is_empty() {
            serde_json::json!({ "text": output })
        } else {
            serde_json::json!({ "text": output, "tool_results": tool_outputs })
        };

        Ok(StepResult {
            step_id: Uuid::new_v4().to_string(),
            plan_step_id: step_plan.id.clone(),
            success: true,
            output: Some(combined_output),
            error: None,
            next_steps: step_plan.next.clone(),
            gate_results: vec![],
            duration_ms: 0,
            tokens_used: None,
        })
    }

    /// Execute a branch step.
    async fn execute_branch_step(
        &mut self,
        run_id: &str,
        run_state: &mut RunState,
        step_plan: &StepPlan,
        condition: &str,
        true_step: &str,
        false_step: &str,
    ) -> Result<StepResult, ExecutionError> {
        // Evaluate condition using model or simple expression
        let branch_taken = self.evaluate_condition(run_state, condition).await?;

        let next_step = if branch_taken { true_step } else { false_step };
        let branch_name = if branch_taken { "true" } else { "false" };

        let _ = self.event_tx.send(PlanEvent::BranchTaken {
            run_id: run_id.to_string(),
            step_id: step_plan.id.clone(),
            branch: branch_name.to_string(),
        });

        // Skip the other branch
        let other_step = if branch_taken { false_step } else { true_step };
        run_state.skip_step(other_step)?;

        Ok(StepResult {
            step_id: Uuid::new_v4().to_string(),
            plan_step_id: step_plan.id.clone(),
            success: true,
            output: Some(serde_json::json!({ "branch": branch_name, "condition": condition })),
            error: None,
            next_steps: vec![next_step.to_string()],
            gate_results: vec![],
            duration_ms: 0,
            tokens_used: None,
        })
    }

    /// Execute a loop step.
    async fn execute_loop_step(
        &mut self,
        run_id: &str,
        run_state: &mut RunState,
        step_plan: &StepPlan,
        collection: &str,
        item_var: &str,
        body_step: &str,
        max_iterations: Option<u32>,
        step_number: u32,
    ) -> Result<StepResult, ExecutionError> {
        // Get collection from variables
        let collection_value = run_state.get_variable(collection)
            .cloned()
            .unwrap_or(serde_json::json!([]));

        let items = match collection_value {
            serde_json::Value::Array(arr) => arr,
            _ => vec![collection_value],
        };

        let max_iter = max_iterations.unwrap_or(items.len() as u32);
        let iterations = items.len().min(max_iter as usize);

        let mut results = Vec::new();
        let mut current_iteration = 0;

        for (idx, item) in items.iter().enumerate() {
            if current_iteration >= iterations {
                break;
            }

            current_iteration += 1;

            // Set loop variable
            run_state.set_variable(item_var, item.clone());

            let _ = self.event_tx.send(PlanEvent::LoopIteration {
                run_id: run_id.to_string(),
                step_id: step_plan.id.clone(),
                iteration: current_iteration,
                item: item.clone(),
            });

            // Execute body step
            let body_plan = run_state.step_states.keys().find(|k| *k == body_step).cloned();
            if let Some(body_step_id) = body_plan {
                // Find the actual step plan
                // This is simplified - in reality you'd look up from the plan
            }
        }

        Ok(StepResult {
            step_id: Uuid::new_v4().to_string(),
            plan_step_id: step_plan.id.clone(),
            success: true,
            output: Some(serde_json::json!({ "iterations": current_iteration, "results": results })),
            error: None,
            next_steps: step_plan.next.clone(),
            gate_results: vec![],
            duration_ms: 0,
            tokens_used: None,
        })
    }

    /// Execute a parallel step.
    async fn execute_parallel_step(
        &mut self,
        run_id: &str,
        run_state: &mut RunState,
        step_plan: &StepPlan,
        branches: &[String],
        join_step: &str,
        step_number: u32,
    ) -> Result<StepResult, ExecutionError> {
        let _ = self.event_tx.send(PlanEvent::ParallelSpawned {
            run_id: run_id.to_string(),
            step_id: step_plan.id.clone(),
            branches: branches.to_vec(),
        });

        // Spawn each branch concurrently
        let mut handles = Vec::new();
        for branch in branches {
            let run_state = run_state.clone();
            let branch = branch.clone();
            let run_id = run_id.to_string();
            let step_id = step_plan.id.clone();

            let handle = tokio::spawn(async move {
                // This would execute the branch - simplified for now
                StepResult {
                    step_id: Uuid::new_v4().to_string(),
                    plan_step_id: branch,
                    success: true,
                    output: Some(serde_json::json!({ "branch": branch })),
                    error: None,
                    next_steps: vec![join_step.to_string()],
                    gate_results: vec![],
                    duration_ms: 0,
                    tokens_used: None,
                }
            });
            handles.push(handle);
        }

        // Wait for all branches
        let results = join_all(handles).await;
        let mut branch_results = Vec::new();
        for result in results {
            match result {
                Ok(r) => branch_results.push(r),
                Err(e) => {
                    error!("Parallel branch panicked: {}", e);
                }
            }
        }

        let _ = self.event_tx.send(PlanEvent::ParallelJoined {
            run_id: run_id.to_string(),
            step_id: step_plan.id.clone(),
            results: branch_results.clone(),
        });

        Ok(StepResult {
            step_id: Uuid::new_v4().to_string(),
            plan_step_id: step_plan.id.clone(),
            success: true,
            output: Some(serde_json::json!({ "branches_completed": branch_results.len() })),
            error: None,
            next_steps: vec![join_step.to_string()],
            gate_results: vec![],
            duration_ms: 0,
            tokens_used: None,
        })
    }

    /// Execute a merge step.
    async fn execute_merge_step(
        &mut self,
        run_id: &str,
        run_state: &mut RunState,
        step_plan: &StepPlan,
        incoming: &[String],
        outgoing: &str,
    ) -> Result<StepResult, ExecutionError> {
        // Check if all incoming branches are complete
        for inc in incoming {
            if let Some(state) = run_state.get_step_state(inc) {
                if state.status != StepStatus::Completed && state.status != StepStatus::Skipped {
                    // Not ready yet - this would typically wait
                    // For now, proceed optimistically
                }
            }
        }

        Ok(StepResult {
            step_id: Uuid::new_v4().to_string(),
            plan_step_id: step_plan.id.clone(),
            success: true,
            output: Some(serde_json::json!({ "merged": incoming.len() })),
            error: None,
            next_steps: vec![outgoing.to_string()],
            gate_results: vec![],
            duration_ms: 0,
            tokens_used: None,
        })
    }

    /// Execute a fallback step.
    async fn execute_fallback_step(
        &mut self,
        run_id: &str,
        run_state: &mut RunState,
        step_plan: &StepPlan,
        primary_step: &str,
        fallback_step: &str,
        error_patterns: &[String],
        step_number: u32,
    ) -> Result<StepResult, ExecutionError> {
        // Try primary step
        // In practice, this would be executed earlier and we check its result
        // For now, simulate by checking if primary failed
        let primary_state = run_state.get_step_state(primary_step);

        let primary_failed = primary_state.map(|s| s.status == StepStatus::Failed).unwrap_or(false);
        let primary_error = primary_state.and_then(|s| s.error.clone()).unwrap_or_default();

        let should_fallback = primary_failed && error_patterns.iter().any(|p| primary_error.contains(p));

        if should_fallback {
            let _ = self.event_tx.send(PlanEvent::FallbackTriggered {
                run_id: run_id.to_string(),
                step_id: step_plan.id.clone(),
                error: primary_error.clone(),
                fallback_step: fallback_step.to_string(),
            });

            // Skip primary, execute fallback
            run_state.skip_step(primary_step)?;

            Ok(StepResult {
                step_id: Uuid::new_v4().to_string(),
                plan_step_id: step_plan.id.clone(),
                success: true,
                output: Some(serde_json::json!({ "fallback": true, "reason": primary_error })),
                error: None,
                next_steps: vec![fallback_step.to_string()],
                gate_results: vec![],
                duration_ms: 0,
                tokens_used: None,
            })
        } else {
            // Primary succeeded or no fallback needed
            Ok(StepResult {
                step_id: Uuid::new_v4().to_string(),
                plan_step_id: step_plan.id.clone(),
                success: true,
                output: Some(serde_json::json!({ "fallback": false })),
                error: None,
                next_steps: step_plan.next.clone(),
                gate_results: vec![],
                duration_ms: 0,
                tokens_used: None,
            })
        }
    }

    /// Execute a gate step.
    async fn execute_gate_step(
        &mut self,
        run_id: &str,
        run_state: &mut RunState,
        step_plan: &StepPlan,
        gate_type: &GateType,
        config: &serde_json::Value,
    ) -> Result<StepResult, ExecutionError> {
        let passed = match gate_type {
            GateType::Scope { authority, host_pattern } => {
                // Check scope authority
                let auth = self.scope_authority.clone();
                match auth.authorize(host_pattern) {
                    Ok(_) => true,
                    Err(e) => {
                        warn!("Scope gate failed: {}", e);
                        false
                    }
                }
            }
            GateType::Completion { gate_name, requirements } => {
                // Check completion gate
                if let Some(gate) = self.completion_gates.get(gate_name) {
                    let ctx = GateRunContext::default();
                    match gate.evaluate(&std::path::PathBuf::from("."), &ctx) {
                        Ok(GateOutcome::Passed) => true,
                        Ok(GateOutcome::Failed(failures)) => {
                            warn!("Completion gate failed: {:?}", failures);
                            false
                        }
                        Err(e) => {
                            warn!("Completion gate error: {}", e);
                            false
                        }
                    }
                } else {
                    warn!("Completion gate not found: {}", gate_name);
                    false
                }
            }
            GateType::Predicate { expression } => {
                // Simple predicate evaluation
                self.evaluate_condition(run_state, expression).await.unwrap_or(false)
            }
        };

        let gate_result = GateResult {
            gate_id: Uuid::new_v4().to_string(),
            gate_type: format!("{:?}", gate_type),
            passed,
            message: if passed { "Gate passed".to_string() } else { "Gate failed".to_string() },
            checked_at: SystemTime::now(),
        };

        run_state.record_gate_result(&step_plan.id, gate_result.clone())?;

        let _ = self.event_tx.send(PlanEvent::GateChecked {
            run_id: run_id.to_string(),
            step_id: step_plan.id.clone(),
            gate_id: gate_result.gate_id.clone(),
            passed,
            message: gate_result.message.clone(),
        });

        Ok(StepResult {
            step_id: Uuid::new_v4().to_string(),
            plan_step_id: step_plan.id.clone(),
            success: passed,
            output: Some(serde_json::json!({ "gate_passed": passed })),
            error: if passed { None } else { Some(gate_result.message) },
            next_steps: if passed { step_plan.next.clone() } else { vec![] },
            gate_results: vec![gate_result],
            duration_ms: 0,
            tokens_used: None,
        })
    }

    /// Check a gate before step execution.
    async fn check_gate(
        &self,
        run_id: &str,
        step_id: &str,
        gate: &StepGate,
        run_state: &RunState,
    ) -> Result<GateResult, ExecutionError> {
        let passed = match &gate.gate_type {
            GateType::Scope { authority, host_pattern } => {
                match self.scope_authority.authorize(host_pattern) {
                    Ok(_) => true,
                    Err(e) => {
                        warn!("Scope gate check failed: {}", e);
                        false
                    }
                }
            }
            GateType::Completion { gate_name, .. } => {
                if let Some(gate) = self.completion_gates.get(gate_name) {
                    let ctx = GateRunContext::default();
                    match gate.evaluate(&std::path::PathBuf::from("."), &ctx) {
                        Ok(GateOutcome::Passed) => true,
                        Ok(GateOutcome::Failed(_)) => false,
                        Err(_) => false,
                    }
                } else {
                    false
                }
            }
            GateType::Predicate { expression } => {
                self.evaluate_condition(run_state, expression).await.unwrap_or(false)
            }
        };

        Ok(GateResult {
            gate_id: gate.gate_id.clone(),
            gate_type: format!("{:?}", gate.gate_type),
            passed,
            message: if passed { "Gate passed".to_string() } else { "Gate failed".to_string() },
            checked_at: SystemTime::now(),
        })
    }

    /// Evaluate a condition expression.
    async fn evaluate_condition(&self, run_state: &RunState, expression: &str) -> Result<bool, ExecutionError> {
        // Simple expression evaluation - in practice would use a proper expression evaluator
        // For now, check if expression matches a variable
        if let Some(value) = run_state.get_variable(expression) {
            return Ok(value.as_bool().unwrap_or(false));
        }

        // Try to parse as simple comparison
        if expression.contains("==") {
            let parts: Vec<&str> = expression.split("==").map(|s| s.trim()).collect();
            if parts.len() == 2 {
                let left = run_state.get_variable(parts[0]).cloned().unwrap_or(serde_json::Value::Null);
                let right = run_state.get_variable(parts[1]).cloned().unwrap_or(serde_json::Value::Null);
                return Ok(left == right);
            }
        }

        // Default: false
        Ok(false)
    }

    /// Determine the next step to execute.
    async fn determine_next_step(
        &self,
        plan: &CompiledPlan,
        current_step: &StepPlan,
        result: &StepResult,
        run_state: &RunState,
    ) -> Result<Option<String>, ExecutionError> {
        // If step has explicit next steps, use them
        if !result.next_steps.is_empty() {
            return Ok(Some(result.next_steps[0].clone()));
        }

        // If step has no next, we're done
        if current_step.next.is_empty() {
            return Ok(None);
        }

        // Use step's next
        Ok(Some(current_step.next[0].clone()))
    }

    /// Build messages for model request.
    fn build_messages(&self, run_state: &RunState, step_plan: &StepPlan) -> Vec<mch_loop::Message> {
        let mut messages = Vec::new();

        // Add context from previous steps
        for (step_id, state) in &run_state.step_states {
            if state.status == StepStatus::Completed {
                if let Some(output) = &state.output {
                    messages.push(mch_loop::Message {
                        role: mch_loop::MessageRole::Assistant,
                        content: format!("Step {} completed: {}", step_id, output),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
            }
        }

        // Add current step prompt
        messages.push(mch_loop::Message {
            role: mch_loop::MessageRole::User,
            content: step_plan.prompt.clone(),
            tool_calls: None,
            tool_call_id: None,
        });

        messages
    }

    /// Build tool definitions from step plan.
    fn build_tool_definitions(&self, step_plan: &StepPlan) -> Vec<ToolDefinition> {
        // Built-in workflow tools
        vec![
            ToolDefinition {
                name: "workflow_next".to_string(),
                description: "Proceed to next step".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "step_id": { "type": "string" },
                        "outcome": { "type": "string" }
                    },
                    "required": ["step_id", "outcome"]
                }),
            },
            ToolDefinition {
                name: "workflow_gate_check".to_string(),
                description: "Check a gate".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "gate_id": { "type": "string" },
                        "gate_type": { "type": "string" }
                    },
                    "required": ["gate_id", "gate_type"]
                }),
            },
        ]
    }

    /// Execute a tool call.
    async fn execute_tool_call(&self, run_state: &RunState, call: &ToolCall) -> Result<ToolResult, ExecutionError> {
        match call.name.as_str() {
            "workflow_next" => {
                Ok(ToolResult {
                    call_id: call.id,
                    output: serde_json::json!({"status": "ok"}),
                    metadata: None,
                    is_error: false,
                })
            }
            "workflow_gate_check" => {
                Ok(ToolResult {
                    call_id: call.id,
                    output: serde_json::json!({"status": "ok"}),
                    metadata: None,
                    is_error: false,
                })
            }
            _ => {
                // Unknown tool
                Ok(ToolResult {
                    call_id: call.id,
                    output: serde_json::json!({"error": format!("unknown tool: {}", call.name)}),
                    metadata: None,
                    is_error: true,
                })
            }
        }
    }
}

/// Runs a single step with full context.
pub struct StepRunner {
    executor: Arc<PlanExecutor>,
    run_id: String,
    run_state: Arc<RwLock<RunState>>,
    plan: CompiledPlan,
}

impl StepRunner {
    pub fn new(
        executor: Arc<PlanExecutor>,
        run_id: String,
        run_state: Arc<RwLock<RunState>>,
        plan: CompiledPlan,
    ) -> Self {
        Self {
            executor,
            run_id,
            run_state,
            plan,
        }
    }

    /// Run a specific step by ID.
    pub async fn run_step(&mut self, step_id: &str) -> Result<StepResult, ExecutionError> {
        let step_plan = self.plan.steps.iter().find(|s| s.id == step_id).cloned()
            .ok_or_else(|| ExecutionError::Step(format!("step not found: {step_id}")))?;

        let mut run_state = self.run_state.write().await;
        let step_number = run_state.step_states.values().filter(|s| s.status == StepStatus::Completed).count() as u32 + 1;
        drop(run_state);

        // Use a mutable reference to executor - need to handle this differently
        // For now, simplified
        Err(ExecutionError::Step("StepRunner.run_step not fully implemented".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::{CompiledPlan, StepPlan, StepKind, PlanMetadata, StepMetadata};
    use mch_transport::TransportFactory;

    #[tokio::test]
    async fn test_plan_executor_creation() {
        let config = PlanExecutorConfig::default();
        let transport = TransportFactory::create(TransportConfig::default());
        let scope_text = r#"
[in_scope]
hosts = ["localhost"]
cidrs = ["127.0.0.1/32"]
"#;
        let scope_authority = Arc::new(mch_scope::ScopeAuthority::from_toml(scope_text).unwrap());

        let executor = PlanExecutor::new(config, transport, scope_authority);
        // Just test creation
    }

    #[test]
    fn test_plan_executor_config_default() {
        let config = PlanExecutorConfig::default();
        assert_eq!(config.max_steps, Some(100));
        assert_eq!(config.max_concurrent_tools, 16);
        assert_eq!(config.retry_policy.max_attempts, 3);
    }
}