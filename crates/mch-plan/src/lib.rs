//! mch-plan — Workflow orchestration from DSH workflow-lite pattern.
//!
//! Architecture: Canvas → compiled plan prompt → model executes (no separate execution engine).
//! Supports: branching, loops, parallelism, merge, fallback.
//! Pipeline: Probe → pattern analysis → generation → API validation → artifact output.
//! Workflow packages (.wflite.zip), versioning, run-state tracking, export-as-skill.
//! Safe analogue for McHarness pipelines: REA autopsy, security audit, code review, hackathon submission.
//! Every risky step routed through mch-scope fail-closed gates.

pub mod compiler;
pub mod executor;
pub mod gates;
pub mod state;

#[cfg(test)]
mod tests;

pub use compiler::{CanvasCompiler, PlanPrompt, CompiledPlan, StepPlan, StepKind, BranchSpec, LoopSpec, ParallelSpec, MergeSpec, FallbackSpec};
pub use executor::{PlanExecutor, StepRunner, ExecutionContext, StepResult, RunSummary};
pub use gates::{ScopeGate, GateError, StepGate, GateVerdict};
pub use state::{RunState, StepState, RunStatus, StepStatus, WorkflowPackage, PackageVersion, ArtifactRecord};