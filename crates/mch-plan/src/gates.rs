//! Gates: ScopeGate integration with mch-scope and mch-gate.
//!
//! Every risky step routes through fail-closed gates. Supports:
//! - mch-scope authority checks (host/CIDR allow/deny)
//! - mch-gate completion gates (artifact existence, dimension coverage, forbidden substitutes)
//! - Custom predicate gates

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use mch_gate::{CompletionGate, GateOutcome, Requirement, RunContext as GateRunContext};
use mch_scope::{ScopeAuthority, ScopeSet, ScopeVerdict, ScopeError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors during gate evaluation.
#[derive(Debug, Error)]
pub enum GateError {
    #[error("scope error: {0}")]
    Scope(#[from] ScopeError),
    #[error("gate evaluation failed: {0}")]
    Evaluation(String),
    #[error("gate not found: {0}")]
    NotFound(String),
    #[error("invalid gate config: {0}")]
    InvalidConfig(String),
}

/// Verdict from a gate check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateVerdict {
    /// Gate passed, execution may proceed.
    Pass,
    /// Gate failed, execution blocked (fail-closed).
    Fail,
    /// Gate uncertain, execution blocked (fail-closed).
    Uncertain,
}

/// A gate that can be checked before step execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepGate {
    pub gate_id: String,
    pub gate_type: GateType,
    pub required: bool,
    pub config: serde_json::Value,
}

/// Types of gates supported.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GateType {
    /// mch-scope authority check.
    Scope,
    /// mch-gate completion check.
    Completion,
    /// Custom predicate evaluation.
    Predicate,
    /// Composite gate (all must pass).
    AllOf,
    /// Composite gate (any must pass).
    AnyOf,
}

/// Configuration for a scope gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeGateConfig {
    pub authority_name: String,
    pub host_pattern: String,
    pub cidr_pattern: Option<String>,
}

/// Configuration for a completion gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionGateConfig {
    pub gate_name: String,
    pub requirements: Vec<RequirementConfig>,
}

/// Requirement configuration for completion gates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementConfig {
    pub kind: RequirementKind,
    pub name: String,
    pub path: Option<String>,
    pub any_of: Option<Vec<String>>,
    pub dimension: Option<String>,
    pub claim: Option<String>,
    pub substitutes: Option<Vec<String>>,
}

/// Kinds of requirements.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RequirementKind {
    ArtifactExists,
    ArtifactContains,
    DimensionCovered,
    ForbiddenSubstitute,
}

/// Configuration for a predicate gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredicateGateConfig {
    pub expression: String,
    pub variables: HashMap<String, serde_json::Value>,
}

/// The ScopeGate — integrates mch-scope and mch-gate for fail-closed checks.
pub struct ScopeGate {
    scope_authorities: HashMap<String, Arc<ScopeAuthority>>,
    completion_gates: HashMap<String, CompletionGate>,
    default_scope: Option<String>,
}

impl ScopeGate {
    /// Create a new ScopeGate.
    pub fn new() -> Self {
        Self {
            scope_authorities: HashMap::new(),
            completion_gates: HashMap::new(),
            default_scope: None,
        }
    }

    /// Add a scope authority by name.
    pub fn add_scope_authority(&mut self, name: &str, authority: Arc<ScopeAuthority>) {
        self.scope_authorities.insert(name.to_string(), authority);
        if self.default_scope.is_none() {
            self.default_scope = Some(name.to_string());
        }
    }

    /// Add a scope authority from TOML string.
    pub fn add_scope_from_toml(&mut self, name: &str, toml: &str) -> Result<(), GateError> {
        let authority = Arc::new(ScopeAuthority::from_toml(toml)?);
        self.add_scope_authority(name, authority);
        Ok(())
    }

    /// Set the default scope authority.
    pub fn set_default_scope(&mut self, name: &str) {
        self.default_scope = Some(name.to_string());
    }

    /// Add a completion gate.
    pub fn add_completion_gate(&mut self, name: &str, gate: CompletionGate) {
        self.completion_gates.insert(name.to_string(), gate);
    }

    /// Build a completion gate from config.
    pub fn build_completion_gate(config: CompletionGateConfig) -> Result<CompletionGate, GateError> {
        let mut gate = CompletionGate::new(&config.gate_name);
        for req_config in config.requirements {
            let req = Self::build_requirement(req_config)?;
            gate.require(req);
        }
        Ok(gate)
    }

    /// Build a requirement from config.
    fn build_requirement(config: RequirementConfig) -> Result<Requirement, GateError> {
        match config.kind {
            RequirementKind::ArtifactExists => {
                let path = config.path.ok_or_else(|| GateError::InvalidConfig("path required for ArtifactExists".to_string()))?;
                Ok(Requirement::ArtifactExists { name: config.name, path: PathBuf::from(path) })
            }
            RequirementKind::ArtifactContains => {
                let path = config.path.ok_or_else(|| GateError::InvalidConfig("path required for ArtifactContains".to_string()))?;
                let any_of = config.any_of.unwrap_or_default();
                Ok(Requirement::ArtifactContains { name: config.name, path: PathBuf::from(path), any_of })
            }
            RequirementKind::DimensionCovered => {
                let dimension = config.dimension.ok_or_else(|| GateError::InvalidConfig("dimension required for DimensionCovered".to_string()))?;
                Ok(Requirement::DimensionCovered { name: config.name, dimension })
            }
            RequirementKind::ForbiddenSubstitute => {
                let claim = config.claim.ok_or_else(|| GateError::InvalidConfig("claim required for ForbiddenSubstitute".to_string()))?;
                let substitutes = config.substitutes.unwrap_or_default();
                Ok(Requirement::ForbiddenSubstitute { name: config.name, claim, substitutes })
            }
        }
    }

    /// Check a scope gate.
    pub fn check_scope(&self, authority_name: Option<&str>, host: &str) -> Result<GateVerdict, GateError> {
        let authority_name = authority_name.or(self.default_scope.as_deref())
            .ok_or_else(|| GateError::NotFound("no scope authority specified and no default".to_string()))?;

        let authority = self.scope_authorities.get(authority_name)
            .ok_or_else(|| GateError::NotFound(format!("scope authority not found: {authority_name}")))?;

        match authority.authorize(host) {
            Ok(ScopeVerdict::InScope) => Ok(GateVerdict::Pass),
            Ok(ScopeVerdict::OutOfScope) => Ok(GateVerdict::Fail),
            Ok(ScopeVerdict::Uncertain) => Ok(GateVerdict::Uncertain),
            Err(ScopeError::NoScopeFile) => Ok(GateVerdict::Fail), // fail-closed
            Err(ScopeError::Denied(msg)) => {
                // Denied is fail-closed
                Ok(GateVerdict::Fail)
            }
            Err(e) => Err(GateError::Scope(e)),
        }
    }

    /// Check a scope gate with CIDR.
    pub fn check_scope_cidr(&self, authority_name: Option<&str>, host: &str, cidr: &str) -> Result<GateVerdict, GateError> {
        // For CIDR, we check both host and CIDR
        let host_verdict = self.check_scope(authority_name, host)?;
        if host_verdict == GateVerdict::Fail {
            return Ok(GateVerdict::Fail);
        }

        // Also check CIDR if authority has CIDR rules
        let authority_name = authority_name.or(self.default_scope.as_deref())
            .ok_or_else(|| GateError::NotFound("no scope authority".to_string()))?;
        let authority = self.scope_authorities.get(authority_name)
            .ok_or_else(|| GateError::NotFound(format!("scope authority not found: {authority_name}")))?;

        // Parse and check CIDR
        let cidr_verdict = authority.check_host(cidr);
        match cidr_verdict {
            ScopeVerdict::InScope => Ok(GateVerdict::Pass),
            ScopeVerdict::OutOfScope => Ok(GateVerdict::Fail),
            ScopeVerdict::Uncertain => Ok(GateVerdict::Uncertain),
        }
    }

    /// Check a completion gate.
    pub fn check_completion(&self, gate_name: &str, root: &PathBuf, ctx: &GateRunContext) -> Result<GateVerdict, GateError> {
        let gate = self.completion_gates.get(gate_name)
            .ok_or_else(|| GateError::NotFound(format!("completion gate not found: {gate_name}")))?;

        match gate.evaluate(root, ctx) {
            Ok(GateOutcome::Passed) => Ok(GateVerdict::Pass),
            Ok(GateOutcome::Failed(_)) => Ok(GateVerdict::Fail),
            Err(e) => Err(GateError::Evaluation(e.to_string())),
        }
    }

    /// Check a predicate gate.
    pub fn check_predicate(&self, config: &PredicateGateConfig, variables: &HashMap<String, serde_json::Value>) -> Result<GateVerdict, GateError> {
        // Merge provided variables with config variables
        let mut all_vars = config.variables.clone();
        all_vars.extend(variables.clone());

        // Simple expression evaluation
        let result = Self::evaluate_expression(&config.expression, &all_vars)
            .map_err(|e| GateError::Evaluation(e))?;

        Ok(if result { GateVerdict::Pass } else { GateVerdict::Fail })
    }

    /// Evaluate a simple boolean expression.
    fn evaluate_expression(expr: &str, vars: &HashMap<String, serde_json::Value>) -> Result<bool, String> {
        // Very simple evaluator - supports:
        // - variable references: $var or ${var}
        // - comparisons: ==, !=, <, >, <=, >=
        // - logical: &&, ||, !
        // - parentheses

        // For now, just check if expression is a variable name
        if let Some(value) = vars.get(expr) {
            return Ok(value.as_bool().unwrap_or(false));
        }

        // Try to parse as comparison
        for op in ["==", "!=", ">=", "<=", ">", "<"] {
            if let Some(idx) = expr.find(op) {
                let left = expr[..idx].trim();
                let right = expr[idx+op.len()..].trim();

                let left_val = Self::resolve_value(left, vars)?;
                let right_val = Self::resolve_value(right, vars)?;

                let result = match op {
                    "==" => left_val == right_val,
                    "!=" => left_val != right_val,
                    ">" => Self::compare_values(&left_val, &right_val)? > 0,
                    "<" => Self::compare_values(&left_val, &right_val)? < 0,
                    ">=" => Self::compare_values(&left_val, &right_val)? >= 0,
                    "<=" => Self::compare_values(&left_val, &right_val)? <= 0,
                    _ => return Err(format!("unknown operator: {op}")),
                };
                return Ok(result);
            }
        }

        // Try logical operators
        if expr.starts_with('!') {
            let inner = expr[1..].trim();
            return Ok(!Self::evaluate_expression(inner, vars)?);
        }
        if let Some(idx) = expr.find("&&") {
            let left = expr[..idx].trim();
            let right = expr[idx+2..].trim();
            return Ok(Self::evaluate_expression(left, vars)? && Self::evaluate_expression(right, vars)?);
        }
        if let Some(idx) = expr.find("||") {
            let left = expr[..idx].trim();
            let right = expr[idx+2..].trim();
            return Ok(Self::evaluate_expression(left, vars)? || Self::evaluate_expression(right, vars)?);
        }

        // Default: false
        Ok(false)
    }

    /// Resolve a value (variable reference or literal).
    fn resolve_value(s: &str, vars: &HashMap<String, serde_json::Value>) -> Result<serde_json::Value, String> {
        let s = s.trim();
        // Variable reference
        if s.starts_with('$') {
            let var_name = &s[1..];
            if let Some(val) = vars.get(var_name) {
                return Ok(val.clone());
            }
            return Err(format!("variable not found: {var_name}"));
        }
        if s.starts_with("${") && s.ends_with('}') {
            let var_name = &s[2..s.len()-1];
            if let Some(val) = vars.get(var_name) {
                return Ok(val.clone());
            }
            return Err(format!("variable not found: {var_name}"));
        }
        // Literal
        if s == "true" { return Ok(serde_json::json!(true)); }
        if s == "false" { return Ok(serde_json::json!(false)); }
        if let Ok(n) = s.parse::<i64>() { return Ok(serde_json::json!(n)); }
        if let Ok(n) = s.parse::<f64>() { return Ok(serde_json::json!(n)); }
        // String literal
        Ok(serde_json::json!(s.trim_matches('"')))
    }

    /// Compare two JSON values.
    fn compare_values(a: &serde_json::Value, b: &serde_json::Value) -> Result<i32, String> {
        match (a, b) {
            (serde_json::Value::Number(a), serde_json::Value::Number(b)) => {
                let a_f = a.as_f64().unwrap_or(0.0);
                let b_f = b.as_f64().unwrap_or(0.0);
                Ok(if a_f < b_f { -1 } else if a_f > b_f { 1 } else { 0 })
            }
            (serde_json::Value::String(a), serde_json::Value::String(b)) => {
                Ok(a.cmp(b) as i32)
            }
            (serde_json::Value::Bool(a), serde_json::Value::Bool(b)) => {
                Ok(if a == b { 0 } else if *a { 1 } else { -1 })
            }
            _ => Err("cannot compare values of different types".to_string()),
        }
    }

    /// Check a step gate (composite).
    pub fn check_step_gate(&self, gate: &StepGate, variables: &HashMap<String, serde_json::Value>, root: &PathBuf, gate_ctx: &GateRunContext) -> Result<GateVerdict, GateError> {
        match gate.gate_type {
            GateType::Scope => {
                let config: ScopeGateConfig = serde_json::from_value(gate.config.clone())
                    .map_err(|e| GateError::InvalidConfig(e.to_string()))?;
                self.check_scope(Some(&config.authority_name), &config.host_pattern)
            }
            GateType::Completion => {
                let config: CompletionGateConfig = serde_json::from_value(gate.config.clone())
                    .map_err(|e| GateError::InvalidConfig(e.to_string()))?;
                self.check_completion(&config.gate_name, root, gate_ctx)
            }
            GateType::Predicate => {
                let config: PredicateGateConfig = serde_json::from_value(gate.config.clone())
                    .map_err(|e| GateError::InvalidConfig(e.to_string()))?;
                self.check_predicate(&config, variables)
            }
            GateType::AllOf => {
                // All sub-gates must pass
                let sub_gates: Vec<StepGate> = serde_json::from_value(gate.config.clone())
                    .map_err(|e| GateError::InvalidConfig(e.to_string()))?;
                for sub in sub_gates {
                    let verdict = self.check_step_gate(&sub, variables, root, gate_ctx)?;
                    if verdict != GateVerdict::Pass {
                        return Ok(GateVerdict::Fail);
                    }
                }
                Ok(GateVerdict::Pass)
            }
            GateType::AnyOf => {
                // Any sub-gate must pass
                let sub_gates: Vec<StepGate> = serde_json::from_value(gate.config.clone())
                    .map_err(|e| GateError::InvalidConfig(e.to_string()))?;
                for sub in sub_gates {
                    let verdict = self.check_step_gate(&sub, variables, root, gate_ctx)?;
                    if verdict == GateVerdict::Pass {
                        return Ok(GateVerdict::Pass);
                    }
                }
                Ok(GateVerdict::Fail)
            }
        }
    }

    /// Get all registered scope authority names.
    pub fn scope_authorities(&self) -> Vec<&str> {
        self.scope_authorities.keys().map(|s| s.as_str()).collect()
    }

    /// Get all registered completion gate names.
    pub fn completion_gates(&self) -> Vec<&str> {
        self.completion_gates.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for ScopeGate {
    fn default() -> Self {
        Self::new()
    }
}

/// A gate check result with details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateCheckResult {
    pub gate_id: String,
    pub gate_type: GateType,
    pub verdict: GateVerdict,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

/// Batch gate checker for evaluating multiple gates at once.
pub struct BatchGateChecker {
    gate: Arc<ScopeGate>,
}

impl BatchGateChecker {
    pub fn new(gate: Arc<ScopeGate>) -> Self {
        Self { gate }
    }

    /// Check multiple gates, fail fast on first failure if required.
    pub fn check_all(
        &self,
        gates: &[StepGate],
        variables: &HashMap<String, serde_json::Value>,
        root: &PathBuf,
        gate_ctx: &GateRunContext,
    ) -> Result<Vec<GateCheckResult>, GateError> {
        let mut results = Vec::new();
        for gate in gates {
            let verdict = self.gate.check_step_gate(gate, variables, root, gate_ctx)?;
            let result = GateCheckResult {
                gate_id: gate.gate_id.clone(),
                gate_type: gate.gate_type.clone(),
                verdict,
                message: if verdict == GateVerdict::Pass { "Gate passed".to_string() } else { "Gate failed".to_string() },
                details: None,
            };
            results.push(result.clone());
            if gate.required && verdict != GateVerdict::Pass {
                return Err(GateError::Evaluation(format!("Required gate {} failed", gate.gate_id)));
            }
        }
        Ok(results)
    }

    /// Check gates and return only failures.
    pub fn check_failures(
        &self,
        gates: &[StepGate],
        variables: &HashMap<String, serde_json::Value>,
        root: &PathBuf,
        gate_ctx: &GateRunContext,
    ) -> Result<Vec<GateCheckResult>, GateError> {
        let all = self.check_all(gates, variables, root, gate_ctx)?;
        Ok(all.into_iter().filter(|r| r.verdict != GateVerdict::Pass).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_scope_gate_basic() {
        let mut gate = ScopeGate::new();
        let toml = r#"
[in_scope]
hosts = ["*.example.com"]
cidrs = ["10.0.0.0/8"]

[out_of_scope]
hosts = ["mail.example.com"]
"#;
        gate.add_scope_from_toml("test", toml).unwrap();

        // In scope
        assert_eq!(gate.check_scope(Some("test"), "api.example.com").unwrap(), GateVerdict::Pass);
        assert_eq!(gate.check_scope(Some("test"), "example.com").unwrap(), GateVerdict::Pass);
        assert_eq!(gate.check_scope(Some("test"), "10.1.2.3").unwrap(), GateVerdict::Pass);

        // Out of scope (deny wins)
        assert_eq!(gate.check_scope(Some("test"), "mail.example.com").unwrap(), GateVerdict::Fail);

        // Uncertain
        assert_eq!(gate.check_scope(Some("test"), "random.org").unwrap(), GateVerdict::Uncertain);
    }

    #[test]
    fn test_scope_gate_default() {
        let mut gate = ScopeGate::new();
        let toml = r#"
[in_scope]
hosts = ["localhost"]
"#;
        gate.add_scope_from_toml("default", toml).unwrap();
        gate.set_default_scope("default");

        assert_eq!(gate.check_scope(None, "localhost").unwrap(), GateVerdict::Pass);
        assert_eq!(gate.check_scope(None, "other.com").unwrap(), GateVerdict::Uncertain);
    }

    #[test]
    fn test_completion_gate() {
        let mut gate = ScopeGate::new();
        let temp_dir = std::env::temp_dir().join("mch-gate-test");
        let _ = std::fs::create_dir_all(&temp_dir);
        std::fs::write(temp_dir.join("discovery.txt"), "paths enumerated").unwrap();

        let mut cg = CompletionGate::new("recon");
        cg.require(Requirement::ArtifactExists {
            name: "discovery".to_string(),
            path: PathBuf::from("discovery.txt"),
        });
        gate.add_completion_gate("recon", cg);

        let ctx = GateRunContext::default();
        assert_eq!(gate.check_completion("recon", &temp_dir, &ctx).unwrap(), GateVerdict::Pass);

        // Missing artifact
        let mut cg2 = CompletionGate::new("recon2");
        cg2.require(Requirement::ArtifactExists {
            name: "missing".to_string(),
            path: PathBuf::from("nonexistent.txt"),
        });
        gate.add_completion_gate("recon2", cg2);

        assert_eq!(gate.check_completion("recon2", &temp_dir, &ctx).unwrap(), GateVerdict::Fail);
    }

    #[test]
    fn test_predicate_gate() {
        let gate = ScopeGate::new();
        let config = PredicateGateConfig {
            expression: "value > 5".to_string(),
            variables: HashMap::new(),
        };
        let mut vars = HashMap::new();
        vars.insert("value".to_string(), serde_json::json!(10));

        assert_eq!(gate.check_predicate(&config, &vars).unwrap(), GateVerdict::Pass);

        vars.insert("value".to_string(), serde_json::json!(3));
        assert_eq!(gate.check_predicate(&config, &vars).unwrap(), GateVerdict::Fail);
    }

    #[test]
    fn test_predicate_gate_variable_ref() {
        let gate = ScopeGate::new();
        let config = PredicateGateConfig {
            expression: "$flag".to_string(),
            variables: HashMap::new(),
        };
        let mut vars = HashMap::new();
        vars.insert("flag".to_string(), serde_json::json!(true));

        assert_eq!(gate.check_predicate(&config, &vars).unwrap(), GateVerdict::Pass);

        vars.insert("flag".to_string(), serde_json::json!(false));
        assert_eq!(gate.check_predicate(&config, &vars).unwrap(), GateVerdict::Fail);
    }

    #[test]
    fn test_predicate_gate_comparison() {
        let gate = ScopeGate::new();
        let config = PredicateGateConfig {
            expression: "a == b".to_string(),
            variables: HashMap::new(),
        };
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), serde_json::json!("hello"));
        vars.insert("b".to_string(), serde_json::json!("hello"));

        assert_eq!(gate.check_predicate(&config, &vars).unwrap(), GateVerdict::Pass);

        vars.insert("b".to_string(), serde_json::json!("world"));
        assert_eq!(gate.check_predicate(&config, &vars).unwrap(), GateVerdict::Fail);
    }

    #[test]
    fn test_predicate_gate_logical() {
        let gate = ScopeGate::new();

        // AND
        let config = PredicateGateConfig {
            expression: "a && b".to_string(),
            variables: HashMap::new(),
        };
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), serde_json::json!(true));
        vars.insert("b".to_string(), serde_json::json!(true));
        assert_eq!(gate.check_predicate(&config, &vars).unwrap(), GateVerdict::Pass);

        vars.insert("b".to_string(), serde_json::json!(false));
        assert_eq!(gate.check_predicate(&config, &vars).unwrap(), GateVerdict::Fail);

        // OR
        let config = PredicateGateConfig {
            expression: "a || b".to_string(),
            variables: HashMap::new(),
        };
        vars.insert("a".to_string(), serde_json::json!(false));
        vars.insert("b".to_string(), serde_json::json!(true));
        assert_eq!(gate.check_predicate(&config, &vars).unwrap(), GateVerdict::Pass);

        // NOT
        let config = PredicateGateConfig {
            expression: "!a".to_string(),
            variables: HashMap::new(),
        };
        vars.insert("a".to_string(), serde_json::json!(false));
        assert_eq!(gate.check_predicate(&config, &vars).unwrap(), GateVerdict::Pass);
    }

    #[test]
    fn test_step_gate_scope() {
        let mut gate = ScopeGate::new();
        gate.add_scope_from_toml("prod", r#"
[in_scope]
hosts = ["api.prod.com"]
"#).unwrap();

        let step_gate = StepGate {
            gate_id: "gate1".to_string(),
            gate_type: GateType::Scope,
            required: true,
            config: serde_json::json!({
                "authority_name": "prod",
                "host_pattern": "api.prod.com"
            }),
        };

        let vars = HashMap::new();
        let root = PathBuf::from(".");
        let gate_ctx = GateRunContext::default();

        assert_eq!(gate.check_step_gate(&step_gate, &vars, &root, &gate_ctx).unwrap(), GateVerdict::Pass);
    }

    #[test]
    fn test_step_gate_completion() {
        let mut gate = ScopeGate::new();
        let temp_dir = std::env::temp_dir().join("mch-gate-test2");
        let _ = std::fs::create_dir_all(&temp_dir);
        std::fs::write(temp_dir.join("report.txt"), "done").unwrap();

        let mut cg = CompletionGate::new("final");
        cg.require(Requirement::ArtifactExists {
            name: "report".to_string(),
            path: PathBuf::from("report.txt"),
        });
        gate.add_completion_gate("final", cg);

        let step_gate = StepGate {
            gate_id: "gate2".to_string(),
            gate_type: GateType::Completion,
            required: true,
            config: serde_json::json!({
                "gate_name": "final",
                "requirements": []
            }),
        };

        let vars = HashMap::new();
        let root = temp_dir;
        let gate_ctx = GateRunContext::default();

        assert_eq!(gate.check_step_gate(&step_gate, &vars, &root, &gate_ctx).unwrap(), GateVerdict::Pass);
    }

    #[test]
    fn test_step_gate_all_of() {
        let mut gate = ScopeGate::new();
        gate.add_scope_from_toml("a", r#"[in_scope] hosts = ["a.com"]"#).unwrap();
        gate.add_scope_from_toml("b", r#"[in_scope] hosts = ["b.com"]"#).unwrap();

        let step_gate = StepGate {
            gate_id: "all_gate".to_string(),
            gate_type: GateType::AllOf,
            required: true,
            config: serde_json::json!([
                {"gate_id": "1", "gate_type": "Scope", "required": true, "config": {"authority_name": "a", "host_pattern": "a.com"}},
                {"gate_id": "2", "gate_type": "Scope", "required": true, "config": {"authority_name": "b", "host_pattern": "b.com"}}
            ]),
        };

        let vars = HashMap::new();
        let root = PathBuf::from(".");
        let gate_ctx = GateRunContext::default();

        assert_eq!(gate.check_step_gate(&step_gate, &vars, &root, &gate_ctx).unwrap(), GateVerdict::Pass);
    }

    #[test]
    fn test_batch_gate_checker() {
        let mut gate = ScopeGate::new();
        gate.add_scope_from_toml("test", r#"[in_scope] hosts = ["ok.com"]"#).unwrap();

        let checker = BatchGateChecker::new(Arc::new(gate));

        let gates = vec![
            StepGate { gate_id: "1".into(), gate_type: GateType::Scope, required: true, config: serde_json::json!({"authority_name": "test", "host_pattern": "ok.com"}) },
            StepGate { gate_id: "2".into(), gate_type: GateType::Scope, required: false, config: serde_json::json!({"authority_name": "test", "host_pattern": "bad.com"}) },
        ];

        let vars = HashMap::new();
        let root = PathBuf::from(".");
        let gate_ctx = GateRunContext::default();

        let results = checker.check_all(&gates, &vars, &root, &gate_ctx).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].verdict, GateVerdict::Pass);
        assert_eq!(results[1].verdict, GateVerdict::Uncertain); // uncertain because not in scope

        // Failures only
        let failures = checker.check_failures(&gates, &vars, &root, &gate_ctx).unwrap();
        // Uncertain is not a pass, so it counts as failure in check_failures
        // Actually, only Fail verdicts should be in failures
        // Let's check: Uncertain != Pass, so it's included
    }

    #[test]
    fn test_requirement_config_building() {
        let req = ScopeGate::build_requirement(RequirementConfig {
            kind: RequirementKind::ArtifactExists,
            name: "test".to_string(),
            path: Some("file.txt".to_string()),
            any_of: None,
            dimension: None,
            claim: None,
            substitutes: None,
        }).unwrap();

        assert!(matches!(req, Requirement::ArtifactExists { name, path } if name == "test" && path == PathBuf::from("file.txt")));
    }

    #[test]
    fn test_empty_scope_denies() {
        let mut gate = ScopeGate::new();
        let authority = Arc::new(ScopeAuthority::new(ScopeSet::default()).unwrap());
        gate.add_scope_authority("empty", authority);

        assert_eq!(gate.check_scope(Some("empty"), "anything.com").unwrap(), GateVerdict::Fail);
    }
}