//! mch-gate — machine-checkable completion gates.
//!
//! Mined from H-mmer/pentest-agents (`tools/autopilot_gate.py`): an agent may
//! not report "done" until a deterministic, code-level gate passes — artifact
//! existence, forbidden-substitute detection, and required-evidence checks.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GateError {
    #[error("gate `{gate}` failed: {reason}")]
    Failed { gate: String, reason: String },
    #[error("invalid gate spec: {0}")]
    Invalid(String),
}

/// A single machine-checkable requirement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Requirement {
    /// A file must exist (artifact check, e.g. `evidence/host/surface/discovery.txt`).
    ArtifactExists { name: String, path: PathBuf },
    /// A file must exist and contain at least one of these substrings.
    ArtifactContains { name: String, path: PathBuf, any_of: Vec<String> },
    /// A named dimension must have been marked covered (surface probes A–I).
    DimensionCovered { name: String, dimension: String },
    /// A named substitute must NOT appear as the claimed reason for completion
    /// (forbidden substitutes: vuln-scanner output cannot count as exhaustion).
    ForbiddenSubstitute { name: String, claim: String, substitutes: Vec<String> },
}

impl Requirement {
    pub fn name(&self) -> &str {
        match self {
            Requirement::ArtifactExists { name, .. }
            | Requirement::ArtifactContains { name, .. }
            | Requirement::DimensionCovered { name, .. }
            | Requirement::ForbiddenSubstitute { name, .. } => name,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionGate {
    pub name: String,
    pub requirements: Vec<Requirement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateOutcome {
    Passed,
    Failed(BTreeSet<String>),
}

impl fmt::Display for GateOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GateOutcome::Passed => write!(f, "PASSED"),
            GateOutcome::Failed(failures) => write!(f, "FAILED: {}", {
                let v: Vec<&str> = failures.iter().map(|s| s.as_str()).collect();
                v.join("; ")
            }),
        }
    }
}

/// Context a run accumulates: which dimensions were probed, what claims were made.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunContext {
    #[serde(default)]
    pub covered_dimensions: BTreeSet<String>,
    #[serde(default)]
    pub completion_claims: Vec<String>,
}

impl RunContext {
    pub fn cover(&mut self, dimension: &str) {
        self.covered_dimensions.insert(dimension.to_string());
    }
    pub fn claim(&mut self, claim: &str) {
        self.completion_claims.push(claim.to_string());
    }
}

impl CompletionGate {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), requirements: Vec::new() }
    }

    pub fn require(&mut self, req: Requirement) -> &mut Self {
        self.requirements.push(req);
        self
    }

    /// Deterministic evaluation. No LLM, no honor system.
    pub fn evaluate(&self, root: &Path, ctx: &RunContext) -> Result<GateOutcome, GateError> {
        if self.requirements.is_empty() {
            return Err(GateError::Invalid(format!("gate `{}` has no requirements", self.name)));
        }
        let mut failures = BTreeSet::new();
        for req in &self.requirements {
            match self.check(req, root, ctx) {
                Ok(()) => {}
                Err(reason) => {
                    failures.insert(format!("{}: {}", req.name(), reason));
                }
            }
        }
        if failures.is_empty() {
            Ok(GateOutcome::Passed)
        } else {
            Ok(GateOutcome::Failed(failures))
        }
    }

    fn check(&self, req: &Requirement, root: &Path, ctx: &RunContext) -> Result<(), String> {
        match req {
            Requirement::ArtifactExists { path, .. } => {
                let p = root.join(path);
                if p.is_file() {
                    Ok(())
                } else {
                    Err(format!("artifact missing: {}", p.display()))
                }
            }
            Requirement::ArtifactContains { path, any_of, .. } => {
                let p = root.join(path);
                let bytes = std::fs::read(&p)
                    .map_err(|_| format!("artifact missing: {}", p.display()))?;
                let text = String::from_utf8_lossy(&bytes);
                if any_of.iter().any(|needle| text.contains(needle.as_str())) {
                    Ok(())
                } else {
                    Err(format!(
                        "artifact {} lacks any of {:?}",
                        p.display(),
                        any_of
                    ))
                }
            }
            Requirement::DimensionCovered { dimension, .. } => {
                if ctx.covered_dimensions.contains(dimension) {
                    Ok(())
                } else {
                    Err(format!("surface dimension `{dimension}` not covered"))
                }
            }
            Requirement::ForbiddenSubstitute { claim, substitutes, .. } => {
                let offending: Vec<&String> = ctx
                    .completion_claims
                    .iter()
                    .flat_map(|c| substitutes.iter().map(move |s| (c, s)))
                    .filter(|(c, s)| c.to_ascii_lowercase().contains(&s.to_ascii_lowercase()))
                    .map(|(_, s)| s)
                    .collect();
                if offending.is_empty() {
                    Ok(())
                } else {
                    Err(format!(
                        "claim `{claim}` relies on forbidden substitute(s): {}",
                        {
                            let v: Vec<&str> = offending.iter().map(|s| s.as_str()).collect();
                            v.join(", ")
                        }
                    ))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mch-gate-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn artifact_gate_passes_when_file_exists() {
        let dir = tmpdir();
        std::fs::write(dir.join("discovery.txt"), "paths enumerated").unwrap();
        let mut gate = CompletionGate::new("recon-complete");
        gate.require(Requirement::ArtifactExists {
            name: "discovery".to_string(),
            path: PathBuf::from("discovery.txt"),
        });
        assert_eq!(gate.evaluate(&dir, &RunContext::default()).unwrap(), GateOutcome::Passed);
    }

    #[test]
    fn artifact_gate_fails_when_missing() {
        let dir = tmpdir();
        let mut gate = CompletionGate::new("recon-complete");
        gate.require(Requirement::ArtifactExists {
            name: "discovery".to_string(),
            path: PathBuf::from("nonexistent.txt"),
        });
        match gate.evaluate(&dir, &RunContext::default()).unwrap() {
            GateOutcome::Failed(f) => assert!(f.iter().any(|s| s.contains("nonexistent"))),
            GateOutcome::Passed => panic!("should fail"),
        }
    }

    #[test]
    fn dimension_gate() {
        let mut ctx = RunContext::default();
        ctx.cover("method-confusion");
        let mut gate = CompletionGate::new("surface");
        gate.require(Requirement::DimensionCovered { name: "d".to_string(), dimension: "method-confusion".into() });
        assert_eq!(gate.evaluate(&tmpdir(), &ctx).unwrap(), GateOutcome::Passed);

        let mut gate2 = CompletionGate::new("surface2");
        gate2.require(Requirement::DimensionCovered { name: "d".to_string(), dimension: "h2-desync".into() });
        assert!(matches!(gate2.evaluate(&tmpdir(), &ctx).unwrap(), GateOutcome::Failed(_)));
    }

    #[test]
    fn forbidden_substitute_gate() {
        let mut ctx = RunContext::default();
        ctx.claim("exhausted rce via vuln-scanner output");
        let mut gate = CompletionGate::new("no-shortcuts");
        gate.require(Requirement::ForbiddenSubstitute {
            name: "no-scanner-shortcut".to_string(),
            claim: "rce exhaustion".into(),
            substitutes: vec!["vuln-scanner".into(), "sast".into()],
        });
        match gate.evaluate(&tmpdir(), &ctx).unwrap() {
            GateOutcome::Failed(f) => assert!(f.iter().any(|s| s.contains("vuln-scanner"))),
            GateOutcome::Passed => panic!("should fail"),
        }
    }

    #[test]
    fn empty_gate_is_invalid() {
        let gate = CompletionGate::new("empty");
        assert!(gate.evaluate(&tmpdir(), &RunContext::default()).is_err());
    }
}
