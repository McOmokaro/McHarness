//! Compiler: Canvas → compiled plan prompt.
//!
//! Transforms a visual Canvas (nodes + edges) into a structured PlanPrompt
//! that the model executes directly. No separate execution engine — the
//! compiled prompt IS the execution plan.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use uuid::Uuid;

/// Errors during Canvas compilation.
#[derive(Debug, Error)]
pub enum CompileError {
    #[error("invalid canvas: {0}")]
    InvalidCanvas(String),
    #[error("unresolved reference: {0}")]
    UnresolvedRef(String),
    #[error("cycle detected: {0}")]
    CycleDetected(String),
    #[error("invalid step kind: {0}")]
    InvalidStepKind(String),
    #[error("missing required field: {0}")]
    MissingField(String),
}

/// A node in the visual Canvas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasNode {
    pub id: String,
    pub kind: CanvasNodeKind,
    pub label: String,
    pub config: serde_json::Value,
    pub position: (f64, f64),
}

/// Kinds of Canvas nodes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CanvasNodeKind {
    /// Entry point of the workflow.
    Start,
    /// A single atomic step executed by the model.
    Step,
    /// Branching: condition → true/false branches.
    Branch,
    /// Loop: iterate over collection or repeat N times.
    Loop,
    /// Parallel: fan-out to multiple concurrent branches.
    Parallel,
    /// Merge: fan-in from multiple branches, wait for all.
    Merge,
    /// Fallback: try primary, on failure execute fallback.
    Fallback,
    /// Gate: scope/mch-gate check before proceeding.
    Gate,
    /// Exit point.
    End,
}

/// An edge connecting two Canvas nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub condition: Option<String>,
}

/// The visual Canvas: nodes + edges forming a DAG (with loop-back edges allowed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Canvas {
    pub nodes: Vec<CanvasNode>,
    pub edges: Vec<CanvasEdge>,
    pub metadata: CanvasMetadata,
}

/// Canvas-level metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CanvasMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub tags: Vec<String>,
}

/// The compiled plan prompt — a linearized, structured representation
/// the model can execute directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledPlan {
    pub plan_id: String,
    pub name: String,
    pub version: String,
    pub steps: Vec<StepPlan>,
    pub entry_step: String,
    pub metadata: PlanMetadata,
}

/// Metadata attached to the compiled plan.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanMetadata {
    pub description: String,
    pub estimated_steps: usize,
    pub has_branching: bool,
    pub has_loops: bool,
    pub has_parallelism: bool,
    pub has_gates: bool,
    pub required_scopes: Vec<String>,
}

/// A single step in the compiled plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepPlan {
    pub id: String,
    pub kind: StepKind,
    pub label: String,
    pub prompt: String,
    pub config: serde_json::Value,
    pub next: Vec<String>,
    pub gates: Vec<StepGate>,
    pub metadata: StepMetadata,
}

/// The kind of execution step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    /// Atomic step: model executes prompt, may call tools.
    Atomic,
    /// Branch: evaluate condition, follow true/false path.
    Branch { condition: String, true_step: String, false_step: String },
    /// Loop: iterate over items or repeat N times.
    Loop { collection: String, item_var: String, body_step: String, max_iterations: Option<u32> },
    /// Parallel: fan out to multiple steps concurrently.
    Parallel { branches: Vec<String>, join_step: String },
    /// Merge: wait for all incoming branches.
    Merge { incoming: Vec<String>, outgoing: String },
    /// Fallback: try primary, on error execute fallback.
    Fallback { primary_step: String, fallback_step: String, error_patterns: Vec<String> },
    /// Gate: enforce scope/mch-gate check.
    Gate { gate_type: GateType, config: serde_json::Value },
    /// End: terminate workflow.
    End,
}

/// Types of gates that can be embedded in steps.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GateType {
    /// mch-scope authority check.
    Scope { authority: String, host_pattern: String },
    /// mch-gate completion check.
    Completion { gate_name: String, requirements: Vec<String> },
    /// Custom predicate gate.
    Predicate { expression: String },
}

/// Per-step metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StepMetadata {
    pub description: String,
    pub estimated_duration_ms: Option<u64>,
    pub retry_policy: Option<RetryPolicy>,
    pub tags: Vec<String>,
}

/// Retry policy for a step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub retry_on: Vec<String>,
}

/// A gate reference embedded in a step plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepGate {
    pub gate_id: String,
    pub gate_type: GateType,
    pub required: bool,
}

/// The PlanPrompt — the compiled prompt text + structured plan
/// that the model receives and executes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanPrompt {
    pub plan: CompiledPlan,
    pub prompt_text: String,
    pub system_prompt: String,
    pub tools_schema: Vec<ToolSchema>,
}

/// Tool schema available to the model during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Branch specification for Canvas branching nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchSpec {
    pub condition: String,
    pub true_branch: String,
    pub false_branch: String,
}

/// Loop specification for Canvas loop nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopSpec {
    pub collection: String,
    pub item_variable: String,
    pub body_step: String,
    pub max_iterations: Option<u32>,
}

/// Parallel specification for Canvas parallel nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelSpec {
    pub branches: Vec<String>,
    pub join_step: String,
}

/// Merge specification for Canvas merge nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeSpec {
    pub incoming: Vec<String>,
    pub outgoing: String,
}

/// Fallback specification for Canvas fallback nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackSpec {
    pub primary: String,
    pub fallback: String,
    pub error_patterns: Vec<String>,
}

/// Compiles a Canvas into a PlanPrompt.
pub struct CanvasCompiler {
    node_map: HashMap<String, CanvasNode>,
    edge_map: HashMap<String, Vec<CanvasEdge>>,
    reverse_edge_map: HashMap<String, Vec<CanvasEdge>>,
    visited: HashSet<String>,
    step_counter: u32,
}

impl CanvasCompiler {
    /// Create a new compiler.
    pub fn new() -> Self {
        Self {
            node_map: HashMap::new(),
            edge_map: HashMap::new(),
            reverse_edge_map: HashMap::new(),
            visited: HashSet::new(),
            step_counter: 0,
        }
    }

    /// Compile a Canvas into a PlanPrompt.
    pub fn compile(&mut self, canvas: Canvas) -> Result<PlanPrompt, CompileError> {
        self.reset();
        self.build_maps(&canvas)?;

        // Find start node
        let start_node = canvas.nodes.iter().find(|n| n.kind == CanvasNodeKind::Start)
            .ok_or_else(|| CompileError::InvalidCanvas("no start node found".to_string()))?;

        // Topological sort with cycle detection (allowing loop-back edges)
        let order = self.topological_sort(&canvas, &start_node.id)?;

        // Build step plans in execution order
        let mut steps = Vec::new();
        for node_id in order {
            let node = self.node_map.get(&node_id).unwrap();
            let step = self.compile_node(node, &canvas)?;
            steps.push(step);
        }

        // Build compiled plan
        let plan_id = Uuid::new_v4().to_string();
        let compiled = CompiledPlan {
            plan_id: plan_id.clone(),
            name: canvas.metadata.name.clone(),
            version: canvas.metadata.version.clone(),
            steps,
            entry_step: start_node.id.clone(),
            metadata: self.build_plan_metadata(&canvas),
        };

        // Generate prompt text
        let prompt_text = self.generate_prompt_text(&compiled);
        let system_prompt = self.generate_system_prompt(&compiled);
        let tools_schema = self.generate_tools_schema(&compiled);

        Ok(PlanPrompt {
            plan: compiled,
            prompt_text,
            system_prompt,
            tools_schema,
        })
    }

    fn reset(&mut self) {
        self.node_map.clear();
        self.edge_map.clear();
        self.reverse_edge_map.clear();
        self.visited.clear();
        self.step_counter = 0;
    }

    fn build_maps(&mut self, canvas: &Canvas) -> Result<(), CompileError> {
        for node in &canvas.nodes {
            self.node_map.insert(node.id.clone(), node.clone());
        }
        for edge in &canvas.edges {
            self.edge_map.entry(edge.from.clone()).or_default().push(edge.clone());
            self.reverse_edge_map.entry(edge.to.clone()).or_default().push(edge.clone());
        }
        Ok(())
    }

    fn topological_sort(&mut self, canvas: &Canvas, start: &str) -> Result<Vec<String>, CompileError> {
        let mut result = Vec::new();
        let mut visiting = HashSet::new();
        let mut visited = HashSet::new();

        fn visit(
            node_id: &str,
            node_map: &HashMap<String, CanvasNode>,
            edge_map: &HashMap<String, Vec<CanvasEdge>>,
            visiting: &mut HashSet<String>,
            visited: &mut HashSet<String>,
            result: &mut Vec<String>,
        ) -> Result<(), CompileError> {
            if visiting.contains(node_id) {
                // Allow loop-back edges (edges to already-visited nodes that are not currently visiting)
                if visited.contains(node_id) {
                    return Ok(()); // Loop back, already processed
                }
                return Err(CompileError::CycleDetected(format!("cycle at node: {node_id}")));
            }
            if visited.contains(node_id) {
                return Ok(());
            }

            visiting.insert(node_id.to_string());

            if let Some(edges) = edge_map.get(node_id) {
                for edge in edges {
                    visit(&edge.to, node_map, edge_map, visiting, visited, result)?;
                }
            }

            visiting.remove(node_id);
            visited.insert(node_id.to_string());
            result.push(node_id.to_string());
            Ok(())
        }

        visit(start, &self.node_map, &self.edge_map, &mut visiting, &mut visited, &mut result)?;
        Ok(result)
    }

    fn compile_node(&mut self, node: &CanvasNode, canvas: &Canvas) -> Result<StepPlan, CompileError> {
        self.step_counter += 1;
        let step_id = format!("step_{}", self.step_counter);

        let (kind, prompt, config, next, gates) = match &node.kind {
            CanvasNodeKind::Start => {
                let edges = self.edge_map.get(&node.id).unwrap_or(&vec![]);
                let next = edges.iter().map(|e| e.to.clone()).collect();
                (StepKind::Atomic, "Begin workflow".to_string(), node.config.clone(), next, vec![])
            }
            CanvasNodeKind::Step => {
                let edges = self.edge_map.get(&node.id).unwrap_or(&vec![]);
                let next = edges.iter().map(|e| e.to.clone()).collect();
                let prompt = node.config.get("prompt").and_then(|v| v.as_str()).unwrap_or(&node.label).to_string();
                (StepKind::Atomic, prompt, node.config.clone(), next, self.extract_gates(&node.config))
            }
            CanvasNodeKind::Branch => {
                let edges = self.edge_map.get(&node.id).unwrap_or(&vec![]);
                let true_edge = edges.iter().find(|e| e.label.as_deref() == Some("true")).ok_or_else(|| CompileError::MissingField("true branch".to_string()))?;
                let false_edge = edges.iter().find(|e| e.label.as_deref() == Some("false")).ok_or_else(|| CompileError::MissingField("false branch".to_string()))?;
                let condition = node.config.get("condition").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let next = vec![true_edge.to.clone(), false_edge.to.clone()];
                let prompt = format!("Evaluate condition: {}. If true, proceed to {}; if false, proceed to {}", condition, true_edge.to, false_edge.to);
                (StepKind::Branch { condition, true_step: true_edge.to.clone(), false_step: false_edge.to.clone() }, prompt, node.config.clone(), next, self.extract_gates(&node.config))
            }
            CanvasNodeKind::Loop => {
                let edges = self.edge_map.get(&node.id).unwrap_or(&vec![]);
                let body_edge = edges.first().ok_or_else(|| CompileError::MissingField("loop body".to_string()))?;
                let collection = node.config.get("collection").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let item_var = node.config.get("item_var").and_then(|v| v.as_str()).unwrap_or("item").to_string();
                let max_iter = node.config.get("max_iterations").and_then(|v| v.as_u64()).map(|v| v as u32);
                let next = vec![body_edge.to.clone()];
                let prompt = format!("Loop over {} as {} (max {} iterations), execute {}", collection, item_var, max_iter.map(|v| v.to_string()).unwrap_or("unlimited".to_string()), body_edge.to);
                (StepKind::Loop { collection, item_var, body_step: body_edge.to.clone(), max_iterations: max_iter }, prompt, node.config.clone(), next, self.extract_gates(&node.config))
            }
            CanvasNodeKind::Parallel => {
                let edges = self.edge_map.get(&node.id).unwrap_or(&vec![]);
                let branches: Vec<String> = edges.iter().map(|e| e.to.clone()).collect();
                let join_step = node.config.get("join_step").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let next = branches.clone();
                let prompt = format!("Execute in parallel: {}. Join at {}", branches.join(", "), join_step);
                (StepKind::Parallel { branches: branches.clone(), join_step: join_step.clone() }, prompt, node.config.clone(), next, self.extract_gates(&node.config))
            }
            CanvasNodeKind::Merge => {
                let edges = self.edge_map.get(&node.id).unwrap_or(&vec![]);
                let outgoing = edges.first().map(|e| e.to.clone()).unwrap_or_default();
                let incoming: Vec<String> = self.reverse_edge_map.get(&node.id).unwrap_or(&vec![]).iter().map(|e| e.from.clone()).collect();
                let next = if outgoing.is_empty() { vec![] } else { vec![outgoing.clone()] };
                let prompt = format!("Merge {} incoming branches, proceed to {}", incoming.len(), outgoing);
                (StepKind::Merge { incoming, outgoing }, prompt, node.config.clone(), next, self.extract_gates(&node.config))
            }
            CanvasNodeKind::Fallback => {
                let edges = self.edge_map.get(&node.id).unwrap_or(&vec![]);
                let primary_edge = edges.iter().find(|e| e.label.as_deref() == Some("primary")).ok_or_else(|| CompileError::MissingField("primary branch".to_string()))?;
                let fallback_edge = edges.iter().find(|e| e.label.as_deref() == Some("fallback")).ok_or_else(|| CompileError::MissingField("fallback branch".to_string()))?;
                let error_patterns: Vec<String> = node.config.get("error_patterns").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
                let next = vec![primary_edge.to.clone(), fallback_edge.to.clone()];
                let prompt = format!("Try {}; on error matching {:?}, execute {}", primary_edge.to, error_patterns, fallback_edge.to);
                (StepKind::Fallback { primary_step: primary_edge.to.clone(), fallback_step: fallback_edge.to.clone(), error_patterns }, prompt, node.config.clone(), next, self.extract_gates(&node.config))
            }
            CanvasNodeKind::Gate => {
                let edges = self.edge_map.get(&node.id).unwrap_or(&vec![]);
                let next = edges.iter().map(|e| e.to.clone()).collect();
                let gate_type = self.parse_gate_type(&node.config)?;
                let prompt = format!("Enforce gate: {:?}", gate_type);
                (StepKind::Gate { gate_type: gate_type.clone(), config: node.config.clone() }, prompt, node.config.clone(), next, vec![StepGate { gate_id: Uuid::new_v4().to_string(), gate_type, required: true }])
            }
            CanvasNodeKind::End => {
                let prompt = "Workflow complete".to_string();
                (StepKind::End, prompt, node.config.clone(), vec![], self.extract_gates(&node.config))
            }
        };

        Ok(StepPlan {
            id: step_id,
            kind,
            label: node.label.clone(),
            prompt,
            config,
            next,
            gates,
            metadata: StepMetadata {
                description: node.label.clone(),
                estimated_duration_ms: node.config.get("duration_ms").and_then(|v| v.as_u64()),
                retry_policy: node.config.get("retry_policy").and_then(|v| serde_json::from_value(v.clone()).ok()),
                tags: node.config.get("tags").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default(),
            },
        })
    }

    fn extract_gates(&self, config: &serde_json::Value) -> Vec<StepGate> {
        config.get("gates").and_then(|v| v.as_array()).map(|arr| {
            arr.iter().filter_map(|g| {
                let gate_type = self.parse_gate_type(g).ok()?;
                Some(StepGate {
                    gate_id: Uuid::new_v4().to_string(),
                    gate_type,
                    required: g.get("required").and_then(|v| v.as_bool()).unwrap_or(true),
                })
            }).collect()
        }).unwrap_or_default()
    }

    fn parse_gate_type(&self, config: &serde_json::Value) -> Result<GateType, CompileError> {
        let gate_type = config.get("gate_type").and_then(|v| v.as_str()).ok_or_else(|| CompileError::MissingField("gate_type".to_string()))?;
        match gate_type {
            "scope" => {
                let authority = config.get("authority").and_then(|v| v.as_str()).unwrap_or("default").to_string();
                let host_pattern = config.get("host_pattern").and_then(|v| v.as_str()).unwrap_or("").to_string();
                Ok(GateType::Scope { authority, host_pattern })
            }
            "completion" => {
                let gate_name = config.get("gate_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let requirements: Vec<String> = config.get("requirements").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
                Ok(GateType::Completion { gate_name, requirements })
            }
            "predicate" => {
                let expression = config.get("expression").and_then(|v| v.as_str()).unwrap_or("").to_string();
                Ok(GateType::Predicate { expression })
            }
            _ => Err(CompileError::InvalidStepKind(format!("unknown gate_type: {gate_type}"))),
        }
    }

    fn build_plan_metadata(&self, canvas: &Canvas) -> PlanMetadata {
        let has_branching = canvas.nodes.iter().any(|n| n.kind == CanvasNodeKind::Branch);
        let has_loops = canvas.nodes.iter().any(|n| n.kind == CanvasNodeKind::Loop);
        let has_parallelism = canvas.nodes.iter().any(|n| n.kind == CanvasNodeKind::Parallel);
        let has_gates = canvas.nodes.iter().any(|n| n.kind == CanvasNodeKind::Gate) || canvas.nodes.iter().any(|n| !self.extract_gates(&n.config).is_empty());

        let required_scopes: Vec<String> = canvas.nodes.iter()
            .flat_map(|n| self.extract_gates(&n.config))
            .filter_map(|g| if let GateType::Scope { authority, .. } = g.gate_type { Some(authority) } else { None })
            .collect();

        PlanMetadata {
            description: canvas.metadata.description.clone(),
            estimated_steps: canvas.nodes.len(),
            has_branching,
            has_loops,
            has_parallelism,
            has_gates,
            required_scopes,
        }
    }

    fn generate_prompt_text(&self, plan: &CompiledPlan) -> String {
        let mut prompt = String::new();
        prompt.push_str(&format!("=== WORKFLOW PLAN: {} ===\n", plan.name));
        prompt.push_str(&format!("Version: {}\n", plan.version));
        prompt.push_str(&format!("Plan ID: {}\n\n", plan.plan_id));
        prompt.push_str("You are executing this workflow plan step by step.\n");
        prompt.push_str("Each step has a prompt, optional gates, and next steps.\n");
        prompt.push_str("Execute gates before proceeding. Report outcome of each step.\n\n");

        for step in &plan.steps {
            prompt.push_str(&format!("--- STEP {}: {} ---\n", step.id, step.label));
            prompt.push_str(&format!("Kind: {:?}\n", step.kind));
            prompt.push_str(&format!("Prompt: {}\n", step.prompt));
            if !step.gates.is_empty() {
                prompt.push_str("Gates:\n");
                for gate in &step.gates {
                    prompt.push_str(&format!("  - {:?} (required: {})\n", gate.gate_type, gate.required));
                }
            }
            if !step.next.is_empty() {
                prompt.push_str(&format!("Next: {}\n", step.next.join(", ")));
            }
            prompt.push_str("\n");
        }

        prompt.push_str("Begin at the entry step. Execute sequentially, respecting branches, loops, parallelism, and fallbacks.\n");
        prompt.push_str("When a step completes, proceed to its next step(s).\n");
        prompt.push_str("For parallel steps, fan out and wait for all to complete at the join step.\n");
        prompt.push_str("For loops, iterate until collection exhausted or max iterations reached.\n");
        prompt.push_str("For fallbacks, catch errors matching patterns and execute fallback step.\n");
        prompt.push_str("All gates are fail-closed: if a gate fails, do not proceed.\n");

        prompt
    }

    fn generate_system_prompt(&self, plan: &CompiledPlan) -> String {
        let mut sys = String::new();
        sys.push_str("You are a workflow execution engine. Your task is to execute the provided plan step by step.\n");
        sys.push_str("You have access to tools. Use them to accomplish each step's prompt.\n");
        sys.push_str("Before each step, check if there are gates. Gates are fail-closed — if a gate fails, STOP and report failure.\n");
        sys.push_str("For branching: evaluate the condition, then follow the true or false path.\n");
        sys.push_str("For loops: iterate over the collection, executing the body step for each item.\n");
        sys.push_str("For parallel: spawn all branches concurrently, then wait at the join step.\n");
        sys.push_str("For merge: wait for all incoming branches to complete.\n");
        sys.push_str("For fallback: try the primary step; if it fails with a matching error, execute the fallback.\n");
        sys.push_str("Report each step's outcome clearly. When the workflow ends, summarize results.\n");
        if plan.metadata.has_gates {
            sys.push_str("\nIMPORTANT: This workflow has embedded gates (scope/completion/predicate). Enforce them strictly.\n");
        }
        sys
    }

    fn generate_tools_schema(&self, plan: &CompiledPlan) -> Vec<ToolSchema> {
        let mut tools = Vec::new();

        // Built-in workflow tools
        tools.push(ToolSchema {
            name: "workflow_next".to_string(),
            description: "Proceed to the next step in the workflow".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "step_id": { "type": "string" },
                    "outcome": { "type": "string" }
                },
                "required": ["step_id", "outcome"]
            }),
        });

        tools.push(ToolSchema {
            name: "workflow_gate_check".to_string(),
            description: "Check a gate (scope/completion/predicate) before proceeding".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "gate_id": { "type": "string" },
                    "gate_type": { "type": "string", "enum": ["scope", "completion", "predicate"] },
                    "config": { "type": "object" }
                },
                "required": ["gate_id", "gate_type"]
            }),
        });

        tools.push(ToolSchema {
            name: "workflow_branch".to_string(),
            description: "Evaluate a branch condition and return true/false".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "condition": { "type": "string" },
                    "context": { "type": "object" }
                },
                "required": ["condition"]
            }),
        });

        tools.push(ToolSchema {
            name: "workflow_loop_iterate".to_string(),
            description: "Get next item from loop collection".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "collection": { "type": "string" },
                    "index": { "type": "integer" }
                },
                "required": ["collection", "index"]
            }),
        });

        tools.push(ToolSchema {
            name: "workflow_parallel_spawn".to_string(),
            description: "Spawn parallel branches".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "branches": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["branches"]
            }),
        });

        tools.push(ToolSchema {
            name: "workflow_fallback".to_string(),
            description: "Execute fallback step after primary failure".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "error": { "type": "string" },
                    "fallback_step": { "type": "string" }
                },
                "required": ["error", "fallback_step"]
            }),
        });

        // Add any custom tools from step configs
        for step in &plan.steps {
            if let Some(custom_tools) = step.config.get("tools").and_then(|v| v.as_array()) {
                for tool in custom_tools {
                    if let Some(name) = tool.get("name").and_then(|v| v.as_str()) {
                        if !tools.iter().any(|t| t.name == name) {
                            tools.push(ToolSchema {
                                name: name.to_string(),
                                description: tool.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                input_schema: tool.get("input_schema").cloned().unwrap_or(serde_json::json!({"type": "object"})),
                            });
                        }
                    }
                }
            }
        }

        tools
    }
}

impl Default for CanvasCompiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_linear_canvas() {
        let canvas = Canvas {
            nodes: vec![
                CanvasNode { id: "start".into(), kind: CanvasNodeKind::Start, label: "Start".into(), config: serde_json::json!({}), position: (0.0, 0.0) },
                CanvasNode { id: "step1".into(), kind: CanvasNodeKind::Step, label: "Step 1".into(), config: serde_json::json!({"prompt": "Do thing 1"}), position: (100.0, 0.0) },
                CanvasNode { id: "end".into(), kind: CanvasNodeKind::End, label: "End".into(), config: serde_json::json!({}), position: (200.0, 0.0) },
            ],
            edges: vec![
                CanvasEdge { from: "start".into(), to: "step1".into(), label: None, condition: None },
                CanvasEdge { from: "step1".into(), to: "end".into(), label: None, condition: None },
            ],
            metadata: CanvasMetadata { name: "Linear".into(), version: "1.0".into(), description: "Linear workflow".into(), author: None, tags: vec![] },
        };

        let mut compiler = CanvasCompiler::new();
        let prompt = compiler.compile(canvas).unwrap();

        assert_eq!(prompt.plan.steps.len(), 3);
        assert_eq!(prompt.plan.steps[0].kind, StepKind::Atomic);
        assert_eq!(prompt.plan.steps[1].kind, StepKind::Atomic);
        assert_eq!(prompt.plan.steps[2].kind, StepKind::End);
    }

    #[test]
    fn test_compile_branch_canvas() {
        let canvas = Canvas {
            nodes: vec![
                CanvasNode { id: "start".into(), kind: CanvasNodeKind::Start, label: "Start".into(), config: serde_json::json!({}), position: (0.0, 0.0) },
                CanvasNode { id: "branch".into(), kind: CanvasNodeKind::Branch, label: "Branch".into(), config: serde_json::json!({"condition": "x > 5"}), position: (100.0, 0.0) },
                CanvasNode { id: "true_step".into(), kind: CanvasNodeKind::Step, label: "True".into(), config: serde_json::json!({"prompt": "True path"}), position: (200.0, -100.0) },
                CanvasNode { id: "false_step".into(), kind: CanvasNodeKind::Step, label: "False".into(), config: serde_json::json!({"prompt": "False path"}), position: (200.0, 100.0) },
                CanvasNode { id: "end".into(), kind: CanvasNodeKind::End, label: "End".into(), config: serde_json::json!({}), position: (300.0, 0.0) },
            ],
            edges: vec![
                CanvasEdge { from: "start".into(), to: "branch".into(), label: None, condition: None },
                CanvasEdge { from: "branch".into(), to: "true_step".into(), label: Some("true".into()), condition: None },
                CanvasEdge { from: "branch".into(), to: "false_step".into(), label: Some("false".into()), condition: None },
                CanvasEdge { from: "true_step".into(), to: "end".into(), label: None, condition: None },
                CanvasEdge { from: "false_step".into(), to: "end".into(), label: None, condition: None },
            ],
            metadata: CanvasMetadata { name: "Branch".into(), version: "1.0".into(), description: "Branching workflow".into(), author: None, tags: vec![] },
        };

        let mut compiler = CanvasCompiler::new();
        let prompt = compiler.compile(canvas).unwrap();

        assert!(prompt.plan.metadata.has_branching);
        let branch_step = prompt.plan.steps.iter().find(|s| matches!(s.kind, StepKind::Branch { .. })).unwrap();
        if let StepKind::Branch { condition, true_step, false_step } = &branch_step.kind {
            assert_eq!(condition, "x > 5");
            assert_eq!(true_step, "true_step");
            assert_eq!(false_step, "false_step");
        } else {
            panic!("expected branch step");
        }
    }

    #[test]
    fn test_compile_loop_canvas() {
        let canvas = Canvas {
            nodes: vec![
                CanvasNode { id: "start".into(), kind: CanvasNodeKind::Start, label: "Start".into(), config: serde_json::json!({}), position: (0.0, 0.0) },
                CanvasNode { id: "loop".into(), kind: CanvasNodeKind::Loop, label: "Loop".into(), config: serde_json::json!({"collection": "items", "item_var": "item", "max_iterations": 10}), position: (100.0, 0.0) },
                CanvasNode { id: "body".into(), kind: CanvasNodeKind::Step, label: "Body".into(), config: serde_json::json!({"prompt": "Process item"}), position: (200.0, 0.0) },
                CanvasNode { id: "end".into(), kind: CanvasNodeKind::End, label: "End".into(), config: serde_json::json!({}), position: (300.0, 0.0) },
            ],
            edges: vec![
                CanvasEdge { from: "start".into(), to: "loop".into(), label: None, condition: None },
                CanvasEdge { from: "loop".into(), to: "body".into(), label: None, condition: None },
                CanvasEdge { from: "body".into(), to: "loop".into(), label: None, condition: None }, // loop back
                CanvasEdge { from: "loop".into(), to: "end".into(), label: None, condition: None }, // exit
            ],
            metadata: CanvasMetadata { name: "Loop".into(), version: "1.0".into(), description: "Loop workflow".into(), author: None, tags: vec![] },
        };

        let mut compiler = CanvasCompiler::new();
        let prompt = compiler.compile(canvas).unwrap();

        assert!(prompt.plan.metadata.has_loops);
        let loop_step = prompt.plan.steps.iter().find(|s| matches!(s.kind, StepKind::Loop { .. })).unwrap();
        if let StepKind::Loop { collection, item_var, body_step, max_iterations } = &loop_step.kind {
            assert_eq!(collection, "items");
            assert_eq!(item_var, "item");
            assert_eq!(body_step, "body");
            assert_eq!(*max_iterations, Some(10));
        } else {
            panic!("expected loop step");
        }
    }

    #[test]
    fn test_compile_parallel_canvas() {
        let canvas = Canvas {
            nodes: vec![
                CanvasNode { id: "start".into(), kind: CanvasNodeKind::Start, label: "Start".into(), config: serde_json::json!({}), position: (0.0, 0.0) },
                CanvasNode { id: "parallel".into(), kind: CanvasNodeKind::Parallel, label: "Parallel".into(), config: serde_json::json!({"join_step": "join"}), position: (100.0, 0.0) },
                CanvasNode { id: "branch1".into(), kind: CanvasNodeKind::Step, label: "Branch 1".into(), config: serde_json::json!({"prompt": "Task 1"}), position: (200.0, -100.0) },
                CanvasNode { id: "branch2".into(), kind: CanvasNodeKind::Step, label: "Branch 2".into(), config: serde_json::json!({"prompt": "Task 2"}), position: (200.0, 100.0) },
                CanvasNode { id: "join".into(), kind: CanvasNodeKind::Merge, label: "Join".into(), config: serde_json::json!({}), position: (300.0, 0.0) },
                CanvasNode { id: "end".into(), kind: CanvasNodeKind::End, label: "End".into(), config: serde_json::json!({}), position: (400.0, 0.0) },
            ],
            edges: vec![
                CanvasEdge { from: "start".into(), to: "parallel".into(), label: None, condition: None },
                CanvasEdge { from: "parallel".into(), to: "branch1".into(), label: None, condition: None },
                CanvasEdge { from: "parallel".into(), to: "branch2".into(), label: None, condition: None },
                CanvasEdge { from: "branch1".into(), to: "join".into(), label: None, condition: None },
                CanvasEdge { from: "branch2".into(), to: "join".into(), label: None, condition: None },
                CanvasEdge { from: "join".into(), to: "end".into(), label: None, condition: None },
            ],
            metadata: CanvasMetadata { name: "Parallel".into(), version: "1.0".into(), description: "Parallel workflow".into(), author: None, tags: vec![] },
        };

        let mut compiler = CanvasCompiler::new();
        let prompt = compiler.compile(canvas).unwrap();

        assert!(prompt.plan.metadata.has_parallelism);
        let parallel_step = prompt.plan.steps.iter().find(|s| matches!(s.kind, StepKind::Parallel { .. })).unwrap();
        if let StepKind::Parallel { branches, join_step } = &parallel_step.kind {
            assert_eq!(branches.len(), 2);
            assert!(branches.contains(&"branch1".to_string()));
            assert!(branches.contains(&"branch2".to_string()));
            assert_eq!(join_step, "join");
        } else {
            panic!("expected parallel step");
        }
    }

    #[test]
    fn test_compile_fallback_canvas() {
        let canvas = Canvas {
            nodes: vec![
                CanvasNode { id: "start".into(), kind: CanvasNodeKind::Start, label: "Start".into(), config: serde_json::json!({}), position: (0.0, 0.0) },
                CanvasNode { id: "fallback".into(), kind: CanvasNodeKind::Fallback, label: "Fallback".into(), config: serde_json::json!({"error_patterns": ["timeout", "connection"]}), position: (100.0, 0.0) },
                CanvasNode { id: "primary".into(), kind: CanvasNodeKind::Step, label: "Primary".into(), config: serde_json::json!({"prompt": "Primary task"}), position: (200.0, -100.0) },
                CanvasNode { id: "fallback_step".into(), kind: CanvasNodeKind::Step, label: "Fallback Step".into(), config: serde_json::json!({"prompt": "Fallback task"}), position: (200.0, 100.0) },
                CanvasNode { id: "end".into(), kind: CanvasNodeKind::End, label: "End".into(), config: serde_json::json!({}), position: (300.0, 0.0) },
            ],
            edges: vec![
                CanvasEdge { from: "start".into(), to: "fallback".into(), label: None, condition: None },
                CanvasEdge { from: "fallback".into(), to: "primary".into(), label: Some("primary".into()), condition: None },
                CanvasEdge { from: "fallback".into(), to: "fallback_step".into(), label: Some("fallback".into()), condition: None },
                CanvasEdge { from: "primary".into(), to: "end".into(), label: None, condition: None },
                CanvasEdge { from: "fallback_step".into(), to: "end".into(), label: None, condition: None },
            ],
            metadata: CanvasMetadata { name: "Fallback".into(), version: "1.0".into(), description: "Fallback workflow".into(), author: None, tags: vec![] },
        };

        let mut compiler = CanvasCompiler::new();
        let prompt = compiler.compile(canvas).unwrap();

        let fallback_step = prompt.plan.steps.iter().find(|s| matches!(s.kind, StepKind::Fallback { .. })).unwrap();
        if let StepKind::Fallback { primary_step, fallback_step: fb_step, error_patterns } = &fallback_step.kind {
            assert_eq!(primary_step, "primary");
            assert_eq!(fb_step, "fallback_step");
            assert_eq!(error_patterns, &["timeout", "connection"]);
        } else {
            panic!("expected fallback step");
        }
    }

    #[test]
    fn test_compile_with_gates() {
        let canvas = Canvas {
            nodes: vec![
                CanvasNode { id: "start".into(), kind: CanvasNodeKind::Start, label: "Start".into(), config: serde_json::json!({}), position: (0.0, 0.0) },
                CanvasNode { id: "step1".into(), kind: CanvasNodeKind::Step, label: "Step 1".into(), config: serde_json::json!({
                    "prompt": "Do thing",
                    "gates": [
                        {"gate_type": "scope", "authority": "prod", "host_pattern": "*.example.com", "required": true}
                    ]
                }), position: (100.0, 0.0) },
                CanvasNode { id: "end".into(), kind: CanvasNodeKind::End, label: "End".into(), config: serde_json::json!({}), position: (200.0, 0.0) },
            ],
            edges: vec![
                CanvasEdge { from: "start".into(), to: "step1".into(), label: None, condition: None },
                CanvasEdge { from: "step1".into(), to: "end".into(), label: None, condition: None },
            ],
            metadata: CanvasMetadata { name: "Gated".into(), version: "1.0".into(), description: "Gated workflow".into(), author: None, tags: vec![] },
        };

        let mut compiler = CanvasCompiler::new();
        let prompt = compiler.compile(canvas).unwrap();

        assert!(prompt.plan.metadata.has_gates);
        assert!(prompt.plan.metadata.required_scopes.contains(&"prod".to_string()));
        let step1 = prompt.plan.steps.iter().find(|s| s.id == "step_2").unwrap(); // step_1 is start, step_2 is step1
        assert_eq!(step1.gates.len(), 1);
        assert!(matches!(step1.gates[0].gate_type, GateType::Scope { authority, .. } if authority == "prod"));
    }
}